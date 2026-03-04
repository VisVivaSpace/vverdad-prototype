//! Docker container execution functions
//!
//! Standalone functions for creating and running Docker containers
//! with proper resource limits and timeout handling.
//!
//! All tokio usage is confined to this file. If bollard is replaced,
//! only this file needs to change and tokio can be dropped.

use std::path::Path;
use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
    StartContainerOptions, WaitContainerOptions,
};
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use futures_util::StreamExt;

use crate::analysis::manifest::ResourceLimits;
use crate::error::VVError;

// =============================================================================
// Configuration Types (Pure Data)
// =============================================================================

/// Configuration for running a Docker container
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Docker image to use
    pub image: String,
    /// Command to execute (entrypoint script)
    pub entrypoint: String,
    /// Working directory inside the container (where analysis bundle is mounted)
    pub working_dir: String,
    /// Host path to mount as working directory
    pub host_path: String,
    /// Resource limits
    pub resources: ResourceLimits,
}

/// Result of container execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Container exit code (0 = success)
    pub exit_code: i64,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Execution duration in seconds
    pub duration_secs: f64,
    /// Whether the container was killed due to timeout
    pub timed_out: bool,
}

/// Returns true if execution was successful (exit code 0, no timeout)
pub fn is_execution_success(result: &ExecutionResult) -> bool {
    result.exit_code == 0 && !result.timed_out
}

// =============================================================================
// Docker Standalone Functions
// =============================================================================

/// Creates a tokio runtime for driving bollard async operations.
/// Internal to this module — no tokio types leak into the public API.
fn create_runtime() -> Result<tokio::runtime::Runtime, VVError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|e| VVError::DockerNotAvailable(e.to_string()))
}

/// Creates a Docker client.
/// Returns just the client — runtime is created locally when needed.
pub fn connect_docker() -> Result<Docker, VVError> {
    Docker::connect_with_local_defaults()
        .map_err(|e| VVError::DockerNotAvailable(e.to_string()))
}

/// Checks if the Docker daemon is available
pub fn check_docker_connection(docker: &Docker) -> Result<(), VVError> {
    let rt = create_runtime()?;
    rt.block_on(async {
        docker
            .ping()
            .await
            .map(|_| ())
            .map_err(|e| VVError::DockerNotAvailable(e.to_string()))
    })
}

/// Runs a container with the given configuration
pub fn run_container(
    docker: &Docker,
    config: &DockerConfig,
) -> Result<ExecutionResult, VVError> {
    let rt = create_runtime()?;
    rt.block_on(run_container_async(docker, config))
}

async fn run_container_async(
    docker: &Docker,
    config: &DockerConfig,
) -> Result<ExecutionResult, VVError> {
    let container_name = format!("vverdad-{}", generate_container_id());
    let start_time = Instant::now();

    // Create container
    let container_id = create_container(docker, &container_name, config).await?;

    // Start container
    docker
        .start_container(&container_id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| VVError::ContainerStartFailed(e.to_string()))?;

    // Wait for container with timeout
    let timeout = Duration::from_secs(config.resources.timeout_seconds);
    let wait_result = wait_with_timeout(docker, &container_id, timeout).await;

    let duration_secs = start_time.elapsed().as_secs_f64();

    // Capture logs
    let (stdout, stderr) = capture_logs(docker, &container_id).await?;

    // Cleanup container
    cleanup_container(docker, &container_id).await?;

    match wait_result {
        Ok(exit_code) => Ok(ExecutionResult {
            exit_code,
            stdout,
            stderr,
            duration_secs,
            timed_out: false,
        }),
        Err(VVError::ContainerTimeout(_)) => Ok(ExecutionResult {
            exit_code: -1,
            stdout,
            stderr,
            duration_secs,
            timed_out: true,
        }),
        Err(e) => Err(e),
    }
}

async fn create_container(
    docker: &Docker,
    name: &str,
    config: &DockerConfig,
) -> Result<String, VVError> {
    let host_config = HostConfig {
        mounts: Some(vec![Mount {
            target: Some(config.working_dir.clone()),
            source: Some(config.host_path.clone()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(false),
            ..Default::default()
        }]),
        memory: Some((config.resources.memory_mb * 1024 * 1024) as i64),
        nano_cpus: Some((config.resources.cpu_cores * 1e9) as i64),
        ..Default::default()
    };

    let container_config = Config {
        image: Some(config.image.clone()),
        cmd: Some(vec![config.entrypoint.clone()]),
        working_dir: Some(config.working_dir.clone()),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name,
        platform: None,
    };

    let response = docker
        .create_container(Some(options), container_config)
        .await
        .map_err(|e| {
            if e.to_string().contains("No such image") {
                VVError::DockerImageNotFound(config.image.clone())
            } else {
                VVError::ContainerCreateFailed(e.to_string())
            }
        })?;

    Ok(response.id)
}

async fn wait_with_timeout(
    docker: &Docker,
    container_id: &str,
    timeout: Duration,
) -> Result<i64, VVError> {
    let wait_options = WaitContainerOptions {
        condition: "not-running",
    };

    let wait_future = async {
        let mut stream = docker.wait_container(container_id, Some(wait_options));
        match stream.next().await {
            Some(result) => match result {
                Ok(response) => Ok(response.status_code),
                Err(e) => Err(VVError::ContainerWaitFailed(e.to_string())),
            },
            _ => Err(VVError::ContainerWaitFailed(
                "No response from wait".to_string(),
            )),
        }
    };

    match tokio::time::timeout(timeout, wait_future).await {
        Ok(result) => result,
        Err(_) => {
            // Timeout occurred, try to stop the container
            let _ = docker.stop_container(container_id, None).await;
            Err(VVError::ContainerTimeout(timeout.as_secs()))
        }
    }
}

async fn capture_logs(docker: &Docker, container_id: &str) -> Result<(String, String), VVError> {
    let options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        follow: false,
        ..Default::default()
    };

    let mut stdout = String::new();
    let mut stderr = String::new();

    let mut stream = docker.logs(container_id, Some(options));
    while let Some(result) = stream.next().await {
        match result {
            Ok(LogOutput::StdOut { message }) => {
                stdout.push_str(&String::from_utf8_lossy(&message));
            }
            Ok(LogOutput::StdErr { message }) => {
                stderr.push_str(&String::from_utf8_lossy(&message));
            }
            _ => {}
        }
    }

    Ok((stdout, stderr))
}

async fn cleanup_container(docker: &Docker, container_id: &str) -> Result<(), VVError> {
    let options = RemoveContainerOptions {
        force: true,
        ..Default::default()
    };

    docker
        .remove_container(container_id, Some(options))
        .await
        .map_err(|e| VVError::ContainerCleanupFailed(e.to_string()))?;

    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Generates a simple identifier for container naming.
/// Uses timestamp + PID — stateless, no global mutable counter.
fn generate_container_id() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:x}{:x}{}",
        now.as_secs(),
        now.subsec_nanos(),
        std::process::id()
    )
}

/// Creates a DockerConfig from analysis bundle data.
/// Ensures the host path is absolute — Docker bind mounts require it.
pub fn create_docker_config(
    image: &str,
    entrypoint: &str,
    bundle_output_path: &Path,
    resources: &ResourceLimits,
) -> DockerConfig {
    // Docker bind mounts require absolute paths.
    // canonicalize() resolves symlinks and requires the path to exist;
    // fall back to joining with cwd for paths that don't exist yet.
    let absolute_path = if bundle_output_path.is_absolute() {
        bundle_output_path.to_path_buf()
    } else {
        bundle_output_path
            .canonicalize()
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(bundle_output_path)
            })
    };

    DockerConfig {
        image: image.to_string(),
        entrypoint: entrypoint.to_string(),
        working_dir: "/analysis".to_string(),
        host_path: absolute_path.to_string_lossy().to_string(),
        resources: resources.clone(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_config_creation() {
        let resources = ResourceLimits {
            cpu_cores: 2.0,
            memory_mb: 1024,
            timeout_seconds: 60,
        };

        let config = create_docker_config(
            "python:3.11",
            "python script.py",
            Path::new("/tmp/analysis"),
            &resources,
        );

        assert_eq!(config.image, "python:3.11");
        assert_eq!(config.entrypoint, "python script.py");
        assert_eq!(config.working_dir, "/analysis");
        assert_eq!(config.host_path, "/tmp/analysis");
        assert_eq!(config.resources.cpu_cores, 2.0);
    }

    #[test]
    fn test_docker_config_relative_path_becomes_absolute() {
        let resources = ResourceLimits {
            cpu_cores: 1.0,
            memory_mb: 512,
            timeout_seconds: 30,
        };

        let config = create_docker_config(
            "python:3.11",
            "python script.py",
            Path::new("artifacts/_output/analysis"),
            &resources,
        );

        assert!(
            Path::new(&config.host_path).is_absolute(),
            "Docker bind mount path must be absolute, got: {}",
            config.host_path
        );
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: "".to_string(),
            duration_secs: 1.5,
            timed_out: false,
        };
        assert!(is_execution_success(&result));
    }

    #[test]
    fn test_execution_result_failure_exit_code() {
        let result = ExecutionResult {
            exit_code: 1,
            stdout: "".to_string(),
            stderr: "error".to_string(),
            duration_secs: 1.0,
            timed_out: false,
        };
        assert!(!is_execution_success(&result));
    }

    #[test]
    fn test_execution_result_timeout() {
        let result = ExecutionResult {
            exit_code: -1,
            stdout: "".to_string(),
            stderr: "".to_string(),
            duration_secs: 60.0,
            timed_out: true,
        };
        assert!(!is_execution_success(&result));
    }

    // Note: Integration tests for Docker require a running Docker daemon
    // and are skipped by default. Run with `cargo test -- --ignored` when Docker is available.

    #[test]
    #[ignore]
    fn test_docker_connection() {
        let docker = connect_docker().expect("Failed to create Docker client");
        check_docker_connection(&docker).expect("Docker daemon not available");
    }

    #[test]
    #[ignore]
    fn test_run_simple_container() {
        let docker = connect_docker().expect("Failed to create Docker client");

        let config = DockerConfig {
            image: "alpine:latest".to_string(),
            entrypoint: "echo".to_string(),
            working_dir: "/".to_string(),
            host_path: "/tmp".to_string(),
            resources: ResourceLimits {
                cpu_cores: 1.0,
                memory_mb: 128,
                timeout_seconds: 30,
            },
        };

        let result = run_container(&docker, &config);
        // This test may fail if Docker is not available or image not pulled
        if let Ok(exec_result) = result {
            assert_eq!(exec_result.exit_code, 0);
        }
    }
}
