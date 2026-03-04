//! Analysis execution runner
//!
//! Orchestrates Docker container execution for rendered analysis bundles.
//! No tokio types appear here — all async details are confined to docker.rs.

use std::path::Path;

use bevy_ecs::prelude::*;
use bollard::Docker;

use crate::analysis::docker::{
    ExecutionResult, check_docker_connection, connect_docker, create_docker_config,
    is_execution_success, run_container,
};
use crate::analysis::manifest::Manifest;
use crate::analysis::renderer::RenderedAnalysis;
use crate::config::VVConfig;
use crate::error::VVError;
use crate::events::ProcessingStatus;
use crate::node::NodeInfo;

// =============================================================================
// Resources (Pure Data)
// =============================================================================

/// Resource holding the Docker client.
/// Fields are directly accessible — no wrapper methods.
/// No tokio runtime stored — docker.rs manages its own runtime internally.
#[derive(Resource)]
pub struct DockerClientResource {
    pub docker: Option<Docker>,
    pub init_error: Option<String>,
}

impl Default for DockerClientResource {
    fn default() -> Self {
        match connect_docker() {
            Ok(docker) => Self {
                docker: Some(docker),
                init_error: None,
            },
            Err(e) => Self {
                docker: None,
                init_error: Some(e.to_string()),
            },
        }
    }
}

// =============================================================================
// Components
// =============================================================================

/// Component marking an analysis that has been executed
#[derive(Component)]
pub struct ExecutedAnalysis {
    pub result: ExecutionResult,
}

// =============================================================================
// Events
// =============================================================================

/// Event emitted when analysis execution fails
#[derive(bevy_ecs::message::Message)]
pub struct ExecutionError {
    pub analysis_id: String,
    pub error: VVError,
}

// =============================================================================
// Systems
// =============================================================================

/// System: Execute rendered analysis bundles in Docker containers
pub fn execute_analyses_system(
    mut commands: Commands,
    config: Res<VVConfig>,
    docker_res: Res<DockerClientResource>,
    analyses: Query<
        (
            Entity,
            &NodeInfo,
            &crate::analysis::AnalysisBundle,
            &RenderedAnalysis,
        ),
        Without<ExecutedAnalysis>,
    >,
    mut errors: MessageWriter<ExecutionError>,
) {
    // Skip if Docker is not available
    let docker = match &docker_res.docker {
        Some(d) => d,
        None => {
            let err_msg = docker_res
                .init_error
                .as_deref()
                .unwrap_or("Unknown error")
                .to_string();
            for (_entity, _node_info, bundle, _rendered) in analyses.iter() {
                errors.write(ExecutionError {
                    analysis_id: bundle.manifest.id.clone(),
                    error: VVError::DockerNotAvailable(err_msg.clone()),
                });
            }
            return;
        }
    };

    // Check Docker connection
    if let Err(e) = check_docker_connection(docker) {
        let err_msg = e.to_string();
        for (_entity, _node_info, bundle, _rendered) in analyses.iter() {
            errors.write(ExecutionError {
                analysis_id: bundle.manifest.id.clone(),
                error: VVError::DockerNotAvailable(err_msg.clone()),
            });
        }
        return;
    }

    for (entity, _node_info, bundle, rendered) in analyses.iter() {
        let result = execute_analysis(
            docker,
            &bundle.manifest,
            &rendered.output_path,
            &config,
        );

        match result {
            Ok(exec_result) => {
                // Check if execution succeeded
                if is_execution_success(&exec_result) {
                    commands.entity(entity).insert(ExecutedAnalysis {
                        result: exec_result,
                    });
                } else {
                    // Execution completed but failed
                    let error = if exec_result.timed_out {
                        VVError::ContainerTimeout(bundle.manifest.resources.timeout_seconds)
                    } else {
                        VVError::ContainerFailed {
                            exit_code: exec_result.exit_code,
                            stderr: exec_result.stderr.clone(),
                        }
                    };

                    errors.write(ExecutionError {
                        analysis_id: bundle.manifest.id.clone(),
                        error,
                    });

                    // Still attach the result for debugging
                    commands.entity(entity).insert(ExecutedAnalysis {
                        result: exec_result,
                    });
                }
            }
            Err(error) => {
                errors.write(ExecutionError {
                    analysis_id: bundle.manifest.id.clone(),
                    error,
                });
            }
        }
    }
}

/// System: Handle execution errors
/// Note: Docker unavailability is treated as a warning, not a fatal error
pub fn handle_execution_errors_system(
    mut events: MessageReader<ExecutionError>,
    mut status: ResMut<ProcessingStatus>,
) {
    for error in events.read() {
        // Docker unavailability is a warning - execution is optional
        if matches!(error.error, VVError::DockerNotAvailable(_)) {
            eprintln!(
                "Warning: Skipping Docker execution for '{}'\n{}",
                error.analysis_id,
                crate::error::format_diagnostic(&error.error)
            );
        } else {
            // Other execution errors are fatal
            eprintln!(
                "Error executing analysis '{}'\n{}",
                error.analysis_id,
                crate::error::format_diagnostic(&error.error)
            );
            crate::events::mark_error(&mut status);
        }
    }
}

// =============================================================================
// Pure Functions
// =============================================================================

/// Executes a single analysis bundle
fn execute_analysis(
    docker: &Docker,
    manifest: &Manifest,
    output_path: &Path,
    _config: &VVConfig,
) -> Result<ExecutionResult, VVError> {
    let docker_config = create_docker_config(
        &manifest.image,
        &manifest.entrypoint,
        output_path,
        &manifest.resources,
    );

    run_container(docker, &docker_config)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_client_resource_default() {
        // This test checks that the resource can be created
        // It may fail to connect to Docker, but should not panic
        let resource = DockerClientResource::default();
        // Either docker is Some or init_error is Some
        assert!(resource.docker.is_some() || resource.init_error.is_some());
    }
}
