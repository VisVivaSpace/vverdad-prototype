//! Output validation system for executed analyses
//!
//! Verifies that analyses produce their expected outputs
//! as declared in the manifest.

use std::path::{Path, PathBuf};

use bevy_ecs::prelude::*;

use crate::analysis::AnalysisBundle;
use crate::analysis::manifest::Manifest;
use crate::analysis::runner::ExecutedAnalysis;
use crate::config::VVConfig;
use crate::error::VVError;
use crate::events::ProcessingStatus;
use crate::node::NodeInfo;

// =============================================================================
// Components
// =============================================================================

/// Component marking an analysis whose outputs have been validated
#[derive(Component)]
pub struct ValidatedAnalysis {
    /// List of verified output files
    pub outputs_verified: Vec<PathBuf>,
}

// =============================================================================
// Events
// =============================================================================

/// Event emitted when output validation fails
#[derive(bevy_ecs::message::Message)]
pub struct OutputValidationError {
    pub analysis_id: String,
    pub missing_outputs: Vec<String>,
    pub execution_logs: Option<String>,
}

// =============================================================================
// Systems
// =============================================================================

/// System: Validate outputs from executed analyses
pub fn validate_outputs_system(
    mut commands: Commands,
    config: Res<VVConfig>,
    analyses: Query<
        (
            Entity,
            &NodeInfo,
            &AnalysisBundle,
            &crate::analysis::renderer::RenderedAnalysis,
            &ExecutedAnalysis,
        ),
        Without<ValidatedAnalysis>,
    >,
    mut errors: MessageWriter<OutputValidationError>,
) {
    for (entity, node_info, bundle, rendered, executed) in analyses.iter() {
        // Skip validation if execution didn't succeed
        if !crate::analysis::docker::is_execution_success(&executed.result) {
            continue;
        }

        // Calculate the output directory (parent of the .analysis bundle output)
        let output_dir = calculate_output_dir(&node_info.path, &config);

        let result =
            validate_analysis_outputs(&bundle.manifest, &output_dir, &rendered.output_path);

        match result {
            Ok(verified) => {
                commands.entity(entity).insert(ValidatedAnalysis {
                    outputs_verified: verified,
                });
            }
            Err((missing, _)) => {
                errors.write(OutputValidationError {
                    analysis_id: bundle.manifest.id.clone(),
                    missing_outputs: missing,
                    execution_logs: Some(format!(
                        "stdout:\n{}\nstderr:\n{}",
                        executed.result.stdout, executed.result.stderr
                    )),
                });
            }
        }
    }
}

/// System: Handle output validation errors
pub fn handle_output_validation_errors_system(
    mut events: MessageReader<OutputValidationError>,
    mut status: ResMut<ProcessingStatus>,
) {
    for error in events.read() {
        // Create a VVError for the first missing output for pretty display
        if let Some(first_missing) = error.missing_outputs.first() {
            let err = crate::error::VVError::MissingOutput {
                analysis_output_dir: std::path::PathBuf::from(&error.analysis_id),
                expected_file: first_missing.clone(),
            };
            eprintln!(
                "Error: Analysis '{}' missing expected outputs\n{}",
                error.analysis_id,
                crate::error::format_diagnostic(&err)
            );
            if error.missing_outputs.len() > 1 {
                eprintln!("Additional missing: {:?}", &error.missing_outputs[1..]);
            }
        }
        if let Some(logs) = &error.execution_logs {
            eprintln!("Execution logs:\n{}", logs);
        }
        crate::events::mark_error(&mut status);
    }
}

// =============================================================================
// Pure Functions
// =============================================================================

/// Calculates the output directory for analysis outputs
/// (parent directory of the rendered analysis bundle)
fn calculate_output_dir(bundle_source_path: &Path, config: &VVConfig) -> PathBuf {
    // Get the relative path of the bundle from input root
    let input_root = crate::config::input_path(config);
    let relative = bundle_source_path
        .strip_prefix(&input_root)
        .unwrap_or(bundle_source_path);

    // The output directory is the parent of the bundle in output/
    let output_bundle = crate::config::output_path(config).join(relative);
    output_bundle
        .parent()
        .unwrap_or(&output_bundle)
        .to_path_buf()
}

/// Validates that all expected outputs exist
/// Returns Ok with list of verified files, or Err with list of missing files
pub fn validate_analysis_outputs(
    manifest: &Manifest,
    output_dir: &Path,
    _bundle_output_path: &Path,
) -> Result<Vec<PathBuf>, (Vec<String>, Vec<PathBuf>)> {
    let mut verified = Vec::new();
    let mut missing = Vec::new();

    for output in &manifest.outputs {
        let output_path = output_dir.join(&output.key);
        if output_path.exists() {
            verified.push(output_path);
        } else {
            missing.push(output.key.clone());
        }
    }

    if missing.is_empty() {
        Ok(verified)
    } else {
        Err((missing, verified))
    }
}

/// Validates a single output file exists
pub fn validate_output_file(output_dir: &Path, filename: &str) -> Result<PathBuf, VVError> {
    let path = output_dir.join(filename);
    if path.exists() {
        Ok(path)
    } else {
        Err(VVError::MissingOutput {
            analysis_output_dir: output_dir.to_path_buf(),
            expected_file: filename.to_string(),
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::manifest::parse_manifest;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_manifest_with_outputs(outputs: Vec<&str>) -> Manifest {
        let outputs_ron: String = outputs
            .iter()
            .map(|o| format!("Output(key: \"{}\"),", o))
            .collect();

        parse_manifest(&format!(
            r#"
Analysis(
    id: "test_analysis",
    version: "1.0.0",
    image: "python:3.11",
    entrypoint: "script.py",
    outputs: [
        {}
    ],
)
"#,
            outputs_ron
        ))
        .unwrap()
    }

    #[test]
    fn test_validate_all_outputs_present() {
        let temp = TempDir::new().unwrap();
        let output_dir = temp.path();

        // Create expected outputs
        fs::write(output_dir.join("result.json"), "{}").unwrap();
        fs::write(output_dir.join("report.pdf"), "pdf content").unwrap();

        let manifest = create_test_manifest_with_outputs(vec!["result.json", "report.pdf"]);

        let result = validate_analysis_outputs(&manifest, output_dir, output_dir);
        assert!(result.is_ok());

        let verified = result.unwrap();
        assert_eq!(verified.len(), 2);
    }

    #[test]
    fn test_validate_missing_output() {
        let temp = TempDir::new().unwrap();
        let output_dir = temp.path();

        // Only create one of the expected outputs
        fs::write(output_dir.join("result.json"), "{}").unwrap();
        // report.pdf is NOT created

        let manifest = create_test_manifest_with_outputs(vec!["result.json", "report.pdf"]);

        let result = validate_analysis_outputs(&manifest, output_dir, output_dir);
        assert!(result.is_err());

        let (missing, _verified) = result.unwrap_err();
        assert_eq!(missing.len(), 1);
        assert!(missing.contains(&"report.pdf".to_string()));
    }

    #[test]
    fn test_validate_no_outputs_expected() {
        let temp = TempDir::new().unwrap();
        let output_dir = temp.path();

        let manifest = create_test_manifest_with_outputs(vec![]);

        let result = validate_analysis_outputs(&manifest, output_dir, output_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_validate_output_file() {
        let temp = TempDir::new().unwrap();
        let output_dir = temp.path();

        // Create the file
        fs::write(output_dir.join("data.json"), "{}").unwrap();

        let result = validate_output_file(output_dir, "data.json");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("data.json"));
    }

    #[test]
    fn test_validate_output_file_missing() {
        let temp = TempDir::new().unwrap();
        let output_dir = temp.path();

        let result = validate_output_file(output_dir, "missing.json");
        assert!(matches!(result, Err(VVError::MissingOutput { .. })));
    }
}
