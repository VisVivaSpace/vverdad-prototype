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
    #[error("unsupported file type: {0}")]
    #[diagnostic(
        code(vverdad::unsupported_file_type),
        help("supported data formats: JSON, YAML, TOML, RON, CSV, XLSX, MessagePack, Pickle, CBOR, BSON\nsupported template extensions: .j2, .jinja, .jinja2, .tmpl")
    )]
    UnsupportedFileType(PathBuf),

    #[error("file has no extension: {0}")]
    #[diagnostic(
        code(vverdad::no_valid_extension),
        help("data files need an extension so vv knows how to parse them (e.g. .json, .yaml, .toml)")
    )]
    NoValidExtension(PathBuf),

    #[error("file parsed successfully but contains no data: {0}")]
    #[diagnostic(
        code(vverdad::empty_data_file),
        help("the file is valid but empty — add data or remove it from the project")
    )]
    EmptyDataFile(PathBuf),

    #[error("expected a directory, got: {0}")]
    #[diagnostic(
        code(vverdad::not_directory),
        help("provide a project directory or a .vv archive file\n  vv ./my-project\n  vv project.vv")
    )]
    NotDirectory(PathBuf),

    #[error("cannot read directory: {0}")]
    #[diagnostic(
        code(vverdad::cant_parse_directory),
        help("check file permissions and ensure the directory is not corrupted")
    )]
    CantParseDirectory(PathBuf),

    #[error("file not found: {0}")]
    #[diagnostic(
        code(vverdad::file_not_found),
        help("check that the path exists and you have read permission")
    )]
    FileNotFound(PathBuf),

    #[error("expected a file, got a directory: {0}")]
    #[diagnostic(
        code(vverdad::not_a_file),
        help("this path is a directory — did you mean to pass it as the project input?")
    )]
    NotAFile(PathBuf),

    #[error("invalid .vv archive: {0}")]
    #[diagnostic(
        code(vverdad::invalid_archive),
        help(".vv archives are zip files — verify the file is not corrupted\n  unzip -t project.vv")
    )]
    InvalidVvArchive(PathBuf),

    #[error("UTF-8 encoding error: {0}")]
    #[diagnostic(
        code(vverdad::utf8_error),
        help("text data files must be UTF-8 encoded — check for binary content or non-UTF-8 encoding")
    )]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("I/O error: {0}")]
    #[diagnostic(
        code(vverdad::io_error),
        help("check file permissions and available disk space")
    )]
    Io(#[from] std::io::Error),

    #[error("serializer error: {0}")]
    #[diagnostic(
        code(vverdad::serializer_error),
        help("internal data conversion failed — this may indicate an unsupported data structure")
    )]
    SerializerError(#[from] serde_value::SerializerError),

    #[error("JSON parse error: {0}")]
    #[diagnostic(
        code(vverdad::json_error),
        help("common issues: missing commas, unquoted keys, trailing commas (not valid JSON)")
    )]
    Json(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    #[diagnostic(
        code(vverdad::yaml_error),
        help("common issues: inconsistent indentation, tabs instead of spaces, unquoted special characters")
    )]
    Yaml(#[from] serde_yaml::Error),

    #[error("TOML parse error: {0}")]
    #[diagnostic(
        code(vverdad::toml_error),
        help("common issues: missing quotes around strings, bare keys with special characters")
    )]
    Toml(#[from] toml::de::Error),

    #[error("RON parse error: {0}")]
    #[diagnostic(
        code(vverdad::ron_error),
        help("RON uses Rust-like syntax — check for missing parentheses or commas\n  see https://github.com/ron-rs/ron")
    )]
    Ron(#[from] ron::error::SpannedError),

    #[error("MessagePack decode error: {0}")]
    #[diagnostic(
        code(vverdad::msgpack_error),
        help("the .msgpack/.mp file may be corrupted or encoded with an incompatible schema")
    )]
    MsgpackDecode(#[from] rmp_serde::decode::Error),

    #[error("Pickle decode error: {0}")]
    #[diagnostic(
        code(vverdad::pickle_error),
        help("the .pickle/.pkl file may use an unsupported protocol version or Python-specific types")
    )]
    Pickle(#[from] serde_pickle::Error),

    #[error("CBOR decode error: {0}")]
    #[diagnostic(
        code(vverdad::cbor_error),
        help("the .cbor file may be corrupted or use unsupported CBOR tags")
    )]
    Cbor(#[from] ciborium::de::Error<std::io::Error>),

    #[error("BSON decode error: {0}")]
    #[diagnostic(
        code(vverdad::bson_error),
        help("the .bson file may be corrupted or use unsupported BSON types")
    )]
    Bson(#[from] bson::de::Error),

    #[error("CSV parse error: {0}")]
    #[diagnostic(
        code(vverdad::csv_error),
        help("common issues: unescaped quotes, inconsistent column counts, mixed line endings")
    )]
    Csv(#[from] csv::Error),

    #[error("zip archive error: {0}")]
    #[diagnostic(
        code(vverdad::zip_error),
        help("the archive may be corrupted — verify with: unzip -t <file>.vv")
    )]
    Zip(#[from] zip::result::ZipError),

    #[error("archive already finalized: {0}")]
    #[diagnostic(
        code(vverdad::archive_finished),
        help("cannot write to an archive after it has been closed — this is an internal error")
    )]
    ArchiveFinished(PathBuf),

    #[error("path traversal blocked: {0}")]
    #[diagnostic(
        code(vverdad::path_traversal),
        help("file paths must stay within their containing directory — remove any '..' segments")
    )]
    PathTraversal(PathBuf),

    #[error("XML parse error: {0}")]
    #[diagnostic(
        code(vverdad::xml_error),
        help("common issues: unclosed tags, invalid characters, missing XML declaration")
    )]
    Xml(#[from] quick_xml::Error),

    #[error("XLSX format error: {0}")]
    #[diagnostic(
        code(vverdad::xlsx_error),
        help("the .xlsx file may be corrupted or use unsupported features — try re-exporting from Excel")
    )]
    XlsxFormat(String),

    #[error("template error: {0}")]
    #[diagnostic(
        code(vverdad::template_error),
        help("check template syntax — see docs/template-guide.md for Jinja2 filter reference")
    )]
    Minijinja(#[from] minijinja::Error),

    #[error("template requires data not yet available: {0:?}")]
    #[diagnostic(
        code(vverdad::unmet_dependencies),
        help("these variables are used in a template but not defined in any data file\ncheck the project directory structure — variable paths match folder/file names")
    )]
    UnmetDependencies(Vec<String>),

    #[error("circular dependency: {}", cycle.join(" \u{2192} "))]
    #[diagnostic(
        code(vverdad::circular_dependency),
        help("templates form a cycle where each depends on output from another\nbreak the cycle by restructuring which templates produce which data")
    )]
    CircularDependency { cycle: Vec<String> },

    // ==========================================================================
    // Analysis Bundle Errors
    // ==========================================================================
    #[error("missing manifest.ron in: {0}")]
    #[diagnostic(
        code(vverdad::missing_manifest),
        help("every .analysis directory needs a manifest.ron defining: id, version, image, entrypoint")
    )]
    MissingManifest(PathBuf),

    #[error("invalid manifest in {path}: {message}")]
    #[diagnostic(
        code(vverdad::invalid_manifest),
        help("required manifest.ron fields: id (string), version (string), image (string), entrypoint (string)\nsee RON syntax: https://github.com/ron-rs/ron")
    )]
    InvalidManifest { path: PathBuf, message: String },

    #[error("static file '{file}' not found in {bundle}")]
    #[diagnostic(
        code(vverdad::missing_static_file),
        help("the manifest lists this file in static_files but it doesn't exist in the .analysis directory")
    )]
    MissingStaticFile { bundle: PathBuf, file: String },

    #[error("template file '{file}' not found in {bundle}")]
    #[diagnostic(
        code(vverdad::missing_template_file),
        help("the manifest lists this file in templates but it doesn't exist in the .analysis directory")
    )]
    MissingTemplateFile { bundle: PathBuf, file: String },

    // ==========================================================================
    // Docker Execution Errors
    // ==========================================================================
    #[error("Docker is not available: {0}")]
    #[diagnostic(
        code(vverdad::docker_not_available),
        help("analysis bundles require Docker — check that the daemon is running:\n  docker info\nDocker is optional; template rendering works without it.")
    )]
    DockerNotAvailable(String),

    #[error("Docker image not found: {0}")]
    #[diagnostic(
        code(vverdad::docker_image_not_found),
        help("pull the image first, or check the image name in manifest.ron:\n  docker pull <image>")
    )]
    DockerImageNotFound(String),

    #[error("failed to create Docker container: {0}")]
    #[diagnostic(
        code(vverdad::container_create_failed),
        help("check Docker daemon status and available resources (disk space, memory)")
    )]
    ContainerCreateFailed(String),

    #[error("failed to start Docker container: {0}")]
    #[diagnostic(
        code(vverdad::container_start_failed),
        help("the container was created but could not start — check the image entrypoint and Docker logs")
    )]
    ContainerStartFailed(String),

    #[error("lost connection while waiting for container: {0}")]
    #[diagnostic(
        code(vverdad::container_wait_failed),
        help("the Docker daemon may have restarted or the connection was interrupted")
    )]
    ContainerWaitFailed(String),

    #[error("analysis failed with exit code {exit_code}")]
    #[diagnostic(
        code(vverdad::container_failed),
        help("the analysis script returned a non-zero exit code\nstderr: {stderr}")
    )]
    ContainerFailed { exit_code: i64, stderr: String },

    #[error("analysis timed out after {0} seconds")]
    #[diagnostic(
        code(vverdad::container_timeout),
        help("increase timeout_seconds in manifest.ron, or optimize the analysis script")
    )]
    ContainerTimeout(u64),

    #[error("failed to clean up container: {0}")]
    #[diagnostic(
        code(vverdad::container_cleanup_failed),
        help("a container may still be running — check with: docker ps -a")
    )]
    ContainerCleanupFailed(String),

    // ==========================================================================
    // Output Validation Errors
    // ==========================================================================
    #[error("expected output '{expected_file}' not produced in {analysis_output_dir}")]
    #[diagnostic(
        code(vverdad::missing_output),
        help("the analysis manifest declares this output but the script did not create it\ncheck that the script writes to the correct path inside the container")
    )]
    MissingOutput {
        analysis_output_dir: PathBuf,
        expected_file: String,
    },

    // ==========================================================================
    // CLI Argument Validation Errors
    // ==========================================================================
    #[error("no project input specified")]
    #[diagnostic(
        code(vverdad::missing_input),
        help("provide a project directory or .vv archive:\n  vv ./my-project\n  vv project.vv\nrun 'vv --help' for full usage")
    )]
    MissingProjectInput,

    #[error("cannot use both -d and -f")]
    #[diagnostic(
        code(vverdad::conflicting_output),
        help("choose one output mode:\n  -d <DIR>   write to a directory\n  -f <FILE>  write to a .vv archive")
    )]
    ConflictingOutputFlags,

    #[error("{kind} not found: {path}")]
    #[diagnostic(
        code(vverdad::input_not_found),
        help("check the path and ensure it exists — use an absolute path if unsure")
    )]
    InputNotFound { kind: String, path: PathBuf },

    // ==========================================================================
    // Init Errors
    // ==========================================================================
    #[error("directory does not exist: {0}")]
    #[diagnostic(
        code(vverdad::init_dir_not_found),
        help("create the directory first, or omit it to use the current directory:\n  mkdir my-project && vv init my-project\n  vv init")
    )]
    InitDirectoryNotFound(PathBuf),

    #[error("file already exists: {0}")]
    #[diagnostic(
        code(vverdad::init_file_exists),
        help("use --force to overwrite:\n  vv init --force")
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
            output.contains("unsupported file type"),
            "Should include error message, got: {}",
            output
        );
        assert!(
            output.contains("supported data formats"),
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
            output.contains("missing commas"),
            "Should include JSON help text, got: {}",
            output
        );
    }
}
