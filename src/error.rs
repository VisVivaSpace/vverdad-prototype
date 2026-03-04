// thiserror + miette derive macros generate Display/Diagnostic impls that trigger
// false-positive unused_assignments warnings on named struct variant fields.
// Module-level allow is safe here — this file is pure error type definitions.
#![allow(unused_assignments)]

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};
use std::path::PathBuf;
use thiserror::Error;

/// Formats a VVError with miette's rich diagnostic output (colors, codes, help text)
pub fn format_diagnostic(error: &VVError) -> String {
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor());
    let mut output = String::new();
    if handler
        .render_report(&mut output, error as &dyn Diagnostic)
        .is_ok()
    {
        output
    } else {
        // Fallback to Display if rendering fails
        format!("{}", error)
    }
}

#[derive(Error, Debug, Diagnostic)]
pub enum VVError {
    #[error("file type not supported: {0}")]
    #[diagnostic(
        code(vverdad::unsupported_file_type),
        help(
            "Supported formats: JSON, YAML, TOML, RON, CSV, XLSX, MessagePack, Pickle, CBOR, BSON"
        )
    )]
    UnsupportedFileType(PathBuf),

    #[error("file doesn't have a valid extension: {0}")]
    #[diagnostic(
        code(vverdad::no_valid_extension),
        help("Data files must have an extension (.json, .yaml, .toml, etc.)")
    )]
    NoValidExtension(PathBuf),

    #[error("file doesn't have any valid data: {0}")]
    #[diagnostic(
        code(vverdad::empty_data_file),
        help("Ensure the file contains valid data in the expected format")
    )]
    EmptyDataFile(PathBuf),

    #[error("Directory expected, found this instead: {0}")]
    #[diagnostic(
        code(vverdad::not_directory),
        help("The input path must be a directory or a .vv archive file")
    )]
    NotDirectory(PathBuf),

    #[error("Can't parse this directory: {0}")]
    #[diagnostic(
        code(vverdad::cant_parse_directory),
        help("Check for permission issues or corrupted files")
    )]
    CantParseDirectory(PathBuf),

    #[error("File not found: {0}")]
    #[diagnostic(
        code(vverdad::file_not_found),
        help("Check that the path exists and is accessible")
    )]
    FileNotFound(PathBuf),

    #[error("Not a file: {0}")]
    #[diagnostic(
        code(vverdad::not_a_file),
        help("Expected a file but found a directory")
    )]
    NotAFile(PathBuf),

    #[error("Invalid .vv archive: {0}")]
    #[diagnostic(
        code(vverdad::invalid_archive),
        help("Ensure the .vv file is a valid zip archive")
    )]
    InvalidVvArchive(PathBuf),

    #[error("UTF-8 encoding error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serializer error: {0}")]
    SerializerError(#[from] serde_value::SerializerError),

    #[error("JSON error: {0}")]
    #[diagnostic(
        code(vverdad::json_error),
        help("Check for missing commas, unquoted strings, or trailing commas")
    )]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    #[diagnostic(
        code(vverdad::yaml_error),
        help("Check for indentation errors or invalid YAML syntax")
    )]
    Yaml(#[from] serde_yaml::Error),

    #[error("TOML error: {0}")]
    #[diagnostic(
        code(vverdad::toml_error),
        help("Check for missing quotes, invalid keys, or syntax errors")
    )]
    Toml(#[from] toml::de::Error),

    #[error("RON error: {0}")]
    #[diagnostic(
        code(vverdad::ron_error),
        help("Check RON syntax - similar to Rust struct literals")
    )]
    Ron(#[from] ron::error::SpannedError),

    #[error("MessagePack decode error: {0}")]
    MsgpackDecode(#[from] rmp_serde::decode::Error),

    #[error("Pickle error: {0}")]
    Pickle(#[from] serde_pickle::Error),

    #[error("CBOR error: {0}")]
    Cbor(#[from] ciborium::de::Error<std::io::Error>),

    #[error("BSON error: {0}")]
    Bson(#[from] bson::de::Error),

    #[error("CSV error: {0}")]
    #[diagnostic(
        code(vverdad::csv_error),
        help("Check for unescaped quotes or inconsistent column counts")
    )]
    Csv(#[from] csv::Error),

    #[error("Zip archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Archive already finalized: {0}")]
    #[diagnostic(
        code(vverdad::archive_finished),
        help("Cannot write to an archive that has already been finalized")
    )]
    ArchiveFinished(PathBuf),

    #[error("Path traversal attempt: {0}")]
    #[diagnostic(
        code(vverdad::path_traversal),
        help("File paths in manifests must not escape their containing directory")
    )]
    PathTraversal(PathBuf),

    #[error("XML parsing error: {0}")]
    #[diagnostic(
        code(vverdad::xml_error),
        help("Check for malformed XML tags or encoding issues")
    )]
    Xml(#[from] quick_xml::Error),

    #[error("XLSX format error: {0}")]
    XlsxFormat(String),

    #[error("MINIJINJA error: {0}")]
    Minijinja(#[from] minijinja::Error),

    #[error("Template requires keys not provided by data: {0:?}")]
    #[diagnostic(
        code(vverdad::unmet_dependencies),
        help("Ensure all variables used in templates are defined in data files")
    )]
    UnmetDependencies(Vec<String>),

    #[error("Circular dependency detected: {}", cycle.join(" → "))]
    #[diagnostic(
        code(vverdad::circular_dependency),
        help("Break the cycle by removing or restructuring template dependencies")
    )]
    CircularDependency { cycle: Vec<String> },

    // ==========================================================================
    // Analysis Bundle Errors
    // ==========================================================================
    #[error("Missing manifest.ron in analysis bundle: {0}")]
    #[diagnostic(
        code(vverdad::missing_manifest),
        help("Every .analysis directory must contain a manifest.ron file")
    )]
    MissingManifest(PathBuf),

    #[error("Invalid manifest in {path}: {message}")]
    #[diagnostic(
        code(vverdad::invalid_manifest),
        help("Check manifest.ron syntax and required fields (id, version, image, entrypoint)")
    )]
    InvalidManifest { path: PathBuf, message: String },

    #[error("Missing static file '{file}' in analysis bundle {bundle}")]
    #[diagnostic(
        code(vverdad::missing_static_file),
        help("Ensure all files listed in static_files exist in the .analysis directory")
    )]
    MissingStaticFile { bundle: PathBuf, file: String },

    #[error("Missing template file '{file}' in analysis bundle {bundle}")]
    #[diagnostic(
        code(vverdad::missing_template_file),
        help("Ensure all files listed in templates exist in the .analysis directory")
    )]
    MissingTemplateFile { bundle: PathBuf, file: String },

    // ==========================================================================
    // Docker Execution Errors
    // ==========================================================================
    #[error("Docker daemon not available: {0}")]
    #[diagnostic(
        code(vverdad::docker_not_available),
        help("Ensure Docker is installed and running. Try 'docker info' to check.")
    )]
    DockerNotAvailable(String),

    #[error("Docker image not found: {0}")]
    #[diagnostic(
        code(vverdad::docker_image_not_found),
        help("Pull the image with 'docker pull <image>' or check the image name in manifest.ron")
    )]
    DockerImageNotFound(String),

    #[error("Failed to create container: {0}")]
    #[diagnostic(code(vverdad::container_create_failed))]
    ContainerCreateFailed(String),

    #[error("Failed to start container: {0}")]
    #[diagnostic(code(vverdad::container_start_failed))]
    ContainerStartFailed(String),

    #[error("Failed to wait for container: {0}")]
    #[diagnostic(code(vverdad::container_wait_failed))]
    ContainerWaitFailed(String),

    #[error("Container execution failed with exit code {exit_code}")]
    #[diagnostic(
        code(vverdad::container_failed),
        help("Check the analysis script for errors. stderr: {stderr}")
    )]
    ContainerFailed { exit_code: i64, stderr: String },

    #[error("Container execution timed out after {0} seconds")]
    #[diagnostic(
        code(vverdad::container_timeout),
        help("Increase timeout_seconds in manifest.ron or optimize the analysis script")
    )]
    ContainerTimeout(u64),

    #[error("Failed to cleanup container: {0}")]
    #[diagnostic(code(vverdad::container_cleanup_failed))]
    ContainerCleanupFailed(String),

    // ==========================================================================
    // Output Validation Errors
    // ==========================================================================
    #[error("Missing expected output '{expected_file}' in {analysis_output_dir}")]
    #[diagnostic(
        code(vverdad::missing_output),
        help("Check that the analysis script produces all declared outputs")
    )]
    MissingOutput {
        analysis_output_dir: PathBuf,
        expected_file: String,
    },

    // ==========================================================================
    // CLI Argument Validation Errors
    // ==========================================================================
    #[error("No project input specified")]
    #[diagnostic(
        code(vverdad::missing_input),
        help("Usage: vv <INPUT> [-d <DIR>] [-f <FILE>] [-y]")
    )]
    MissingProjectInput,

    #[error("Cannot use both -d (--output-dir) and -f (--output-file)")]
    #[diagnostic(
        code(vverdad::conflicting_output),
        help("Choose one output destination")
    )]
    ConflictingOutputFlags,

    #[error("{kind} not found: {path}")]
    #[diagnostic(code(vverdad::input_not_found), help("Check the path and try again"))]
    InputNotFound { kind: String, path: PathBuf },

    // ==========================================================================
    // Init Errors
    // ==========================================================================
    #[error("Init target directory does not exist: {0}")]
    #[diagnostic(
        code(vverdad::init_dir_not_found),
        help("Create the directory first or specify an existing directory")
    )]
    InitDirectoryNotFound(PathBuf),

    #[error("File already exists and --force not specified: {0}")]
    #[diagnostic(
        code(vverdad::init_file_exists),
        help("Use --force to overwrite existing files")
    )]
    InitFileExists(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_diagnostic_includes_code_and_help() {
        let error = VVError::UnsupportedFileType(PathBuf::from("/test/file.xyz"));
        let output = format_diagnostic(&error);

        assert!(
            output.contains("vverdad::unsupported_file_type"),
            "Should include error code, got: {}",
            output
        );
        assert!(
            output.contains("file type not supported"),
            "Should include error message, got: {}",
            output
        );
        assert!(
            output.contains("Supported formats:"),
            "Should include help text, got: {}",
            output
        );
    }

    #[test]
    fn test_format_diagnostic_json_error() {
        let json_err: serde_json::Error = serde_json::from_str::<i32>("invalid").unwrap_err();
        let error = VVError::Json(json_err);
        let output = format_diagnostic(&error);

        assert!(
            output.contains("vverdad::json_error"),
            "Should include JSON error code, got: {}",
            output
        );
        assert!(
            output.contains("Check for missing commas"),
            "Should include JSON help text, got: {}",
            output
        );
    }
}
