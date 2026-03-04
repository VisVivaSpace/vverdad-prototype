//! Configuration module
//!
//! ECS Pattern: Centralized configuration as a Resource.
//! Eliminates magic strings and provides a single source of truth.

use std::path::{Path, PathBuf};

use bevy_ecs::prelude::*;

use crate::error::VVError;

// =============================================================================
// Input Type
// =============================================================================

/// Specifies the type of input source
#[derive(Clone, Debug, PartialEq)]
pub enum InputType {
    /// A directory on the filesystem
    Directory(PathBuf),
    /// A .vv zip archive
    ZipArchive(PathBuf),
}

// =============================================================================
// Output Type
// =============================================================================

/// Specifies where output should be written
#[derive(Clone, Debug, PartialEq)]
pub enum OutputType {
    /// Write _output/ inside the input (current default behavior)
    InPlace,
    /// Copy project to directory, write _output/ inside
    Directory(PathBuf),
    /// Create .vv archive with project + _output/
    Archive(PathBuf),
}

/// Detects the input type from a path
pub fn detect_input_type(path: &Path) -> Option<InputType> {
    if path.extension().map(|e| e == "vv").unwrap_or(false) {
        if path.is_file() {
            Some(InputType::ZipArchive(path.to_path_buf()))
        } else {
            None
        }
    } else if path.is_dir() {
        Some(InputType::Directory(path.to_path_buf()))
    } else {
        None
    }
}

/// Returns the path associated with an input type
pub fn input_type_path(input_type: &InputType) -> &PathBuf {
    match input_type {
        InputType::Directory(p) | InputType::ZipArchive(p) => p,
    }
}

// =============================================================================
// Configuration Resource
// =============================================================================

/// Centralized configuration for the VVERDAD engine
#[derive(Resource, Clone)]
pub struct VVConfig {
    /// Input type (directory or zip archive)
    pub input_type: InputType,
    /// Output type (in-place, directory, or archive)
    pub output_type: OutputType,
    /// Subdirectory for output files (relative to project_dir)
    pub output_subdir: String,
    /// Supported template file extensions
    pub template_extensions: Vec<String>,
    /// Supported data file extensions
    pub data_extensions: Vec<String>,
}

/// Creates a new config with the given project directory (legacy, assumes directory)
pub fn new_config(project_dir: PathBuf) -> VVConfig {
    VVConfig {
        input_type: InputType::Directory(project_dir),
        output_type: OutputType::InPlace,
        ..Default::default()
    }
}

/// Creates a new config with the given input type
pub fn config_with_input_type(input_type: InputType) -> VVConfig {
    VVConfig {
        input_type,
        output_type: OutputType::InPlace,
        ..Default::default()
    }
}

/// Creates a new config with the given input and output types
pub fn config_with_input_and_output(input_type: InputType, output_type: OutputType) -> VVConfig {
    VVConfig {
        input_type,
        output_type,
        ..Default::default()
    }
}

/// Returns the input directory path
pub fn input_path(config: &VVConfig) -> PathBuf {
    input_type_path(&config.input_type).clone()
}

/// Returns the output directory path
pub fn output_path(config: &VVConfig) -> PathBuf {
    match &config.output_type {
        OutputType::InPlace => match &config.input_type {
            InputType::Directory(dir) => dir.join(&config.output_subdir),
            InputType::ZipArchive(archive) => archive
                .parent()
                .unwrap_or(archive.as_path())
                .join(&config.output_subdir),
        },
        OutputType::Directory(dir) => dir.join(&config.output_subdir),
        OutputType::Archive(_) => PathBuf::from(&config.output_subdir),
    }
}

/// Returns the root path for copying project files
pub fn copy_root_path(config: &VVConfig) -> Option<PathBuf> {
    match &config.output_type {
        OutputType::InPlace => None,
        OutputType::Directory(dir) => Some(dir.clone()),
        OutputType::Archive(archive) => Some(archive.clone()),
    }
}

/// Returns whether the input is a zip archive
pub fn is_zip_archive(config: &VVConfig) -> bool {
    matches!(config.input_type, InputType::ZipArchive(_))
}

/// Checks if a file extension is a supported template type
pub fn is_template_extension(config: &VVConfig, ext: &str) -> bool {
    config.template_extensions.iter().any(|e| e == ext)
}

/// Checks if a file extension is a supported data type
pub fn is_data_extension(config: &VVConfig, ext: &str) -> bool {
    config.data_extensions.iter().any(|e| e == ext)
}

impl Default for VVConfig {
    fn default() -> Self {
        Self {
            input_type: InputType::Directory(PathBuf::new()),
            output_type: OutputType::InPlace,
            output_subdir: "_output".to_string(),
            template_extensions: vec![
                "j2".to_string(),
                "jinja".to_string(),
                "jinja2".to_string(),
                "tmpl".to_string(),
            ],
            data_extensions: vec![
                // Text formats
                "json".to_string(),
                "yaml".to_string(),
                "yml".to_string(),
                "toml".to_string(),
                "ron".to_string(),
                // Binary formats
                "msgpack".to_string(),
                "mp".to_string(),
                "pickle".to_string(),
                "pkl".to_string(),
                "cbor".to_string(),
                "bson".to_string(),
                // Tabular formats
                "csv".to_string(),
                "xlsx".to_string(),
                // Markdown
                "md".to_string(),
            ],
        }
    }
}

// =============================================================================
// CLI Argument Validation
// =============================================================================

/// Validate CLI arguments and return the resolved project path.
/// Pure function: no I/O, no side effects.
pub fn validate_run_args(
    project: &Option<PathBuf>,
    output_dir: &Option<PathBuf>,
    output_file: &Option<PathBuf>,
) -> Result<PathBuf, VVError> {
    let project_path = project.as_ref().ok_or(VVError::MissingProjectInput)?;

    if output_dir.is_some() && output_file.is_some() {
        return Err(VVError::ConflictingOutputFlags);
    }

    let is_archive = project_path.extension().map(|e| e == "vv").unwrap_or(false);

    if is_archive {
        if !project_path.is_file() {
            return Err(VVError::InputNotFound {
                kind: "Archive".into(),
                path: project_path.clone(),
            });
        }
    } else if !project_path.is_dir() {
        return Err(VVError::InputNotFound {
            kind: "Directory".into(),
            path: project_path.clone(),
        });
    }

    Ok(project_path.clone())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VVConfig::default();

        assert_eq!(config.output_subdir, "_output");
        assert!(is_template_extension(&config, "j2"));
        assert!(is_template_extension(&config, "jinja"));
        assert!(is_data_extension(&config, "json"));
        assert!(is_data_extension(&config, "yaml"));
    }

    #[test]
    fn test_config_paths() {
        let config = new_config(PathBuf::from("/project"));

        assert_eq!(input_path(&config), PathBuf::from("/project"));
        assert_eq!(output_path(&config), PathBuf::from("/project/_output"));
    }

    #[test]
    fn test_config_with_zip_archive() {
        let config =
            config_with_input_type(InputType::ZipArchive(PathBuf::from("/data/project.vv")));

        assert!(is_zip_archive(&config));
        assert_eq!(input_path(&config), PathBuf::from("/data/project.vv"));
    }

    #[test]
    fn test_input_type_detect_directory() {
        let temp = std::env::temp_dir();
        let input_type = detect_input_type(&temp);
        assert!(matches!(input_type, Some(InputType::Directory(_))));
    }

    #[test]
    fn test_extension_checks() {
        let config = VVConfig::default();

        // Template extensions
        assert!(is_template_extension(&config, "j2"));
        assert!(is_template_extension(&config, "jinja2"));
        assert!(!is_template_extension(&config, "json"));

        // Text data extensions
        assert!(is_data_extension(&config, "json"));
        assert!(is_data_extension(&config, "toml"));
        assert!(is_data_extension(&config, "yaml"));
        assert!(is_data_extension(&config, "ron"));

        // Binary data extensions
        assert!(is_data_extension(&config, "msgpack"));
        assert!(is_data_extension(&config, "mp"));
        assert!(is_data_extension(&config, "pickle"));
        assert!(is_data_extension(&config, "pkl"));
        assert!(is_data_extension(&config, "cbor"));
        assert!(is_data_extension(&config, "bson"));

        // Tabular data extensions
        assert!(is_data_extension(&config, "csv"));
        assert!(is_data_extension(&config, "xlsx"));

        // Non-data extensions
        assert!(!is_data_extension(&config, "j2"));
        assert!(!is_data_extension(&config, "txt"));
    }

    #[test]
    fn test_output_type_directory() {
        let config = config_with_input_and_output(
            InputType::Directory(PathBuf::from("/input")),
            OutputType::Directory(PathBuf::from("/output")),
        );

        assert_eq!(output_path(&config), PathBuf::from("/output/_output"));
        assert_eq!(copy_root_path(&config), Some(PathBuf::from("/output")));
    }

    #[test]
    fn test_output_type_archive() {
        let config = config_with_input_and_output(
            InputType::Directory(PathBuf::from("/input")),
            OutputType::Archive(PathBuf::from("/output.vv")),
        );

        assert_eq!(output_path(&config), PathBuf::from("_output"));
        assert_eq!(copy_root_path(&config), Some(PathBuf::from("/output.vv")));
    }

    #[test]
    fn test_output_type_in_place() {
        let config = config_with_input_and_output(
            InputType::Directory(PathBuf::from("/project")),
            OutputType::InPlace,
        );

        assert_eq!(output_path(&config), PathBuf::from("/project/_output"));
        assert_eq!(copy_root_path(&config), None);
    }

    #[test]
    fn test_validate_run_args_missing_input() {
        let result = validate_run_args(&None, &None, &None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, VVError::MissingProjectInput),
            "Expected MissingProjectInput, got: {:?}",
            err
        );
    }

    #[test]
    fn test_validate_run_args_conflicting_flags() {
        let result = validate_run_args(
            &Some(PathBuf::from("/tmp")),
            &Some(PathBuf::from("/out")),
            &Some(PathBuf::from("/out.vv")),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, VVError::ConflictingOutputFlags),
            "Expected ConflictingOutputFlags, got: {:?}",
            err
        );
    }

    #[test]
    fn test_validate_run_args_nonexistent_directory() {
        let result = validate_run_args(
            &Some(PathBuf::from("/nonexistent_path_that_does_not_exist")),
            &None,
            &None,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, VVError::InputNotFound { .. }),
            "Expected InputNotFound, got: {:?}",
            err
        );
    }

    #[test]
    fn test_validate_run_args_nonexistent_archive() {
        let result = validate_run_args(&Some(PathBuf::from("/nonexistent_path.vv")), &None, &None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, VVError::InputNotFound { .. }),
            "Expected InputNotFound, got: {:?}",
            err
        );
    }

    #[test]
    fn test_validate_run_args_valid_directory() {
        let temp = std::env::temp_dir();
        let result = validate_run_args(&Some(temp.clone()), &None, &None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), temp);
    }
}
