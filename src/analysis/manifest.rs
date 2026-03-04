//! Manifest types for analysis bundles
//!
//! RON-deserializable types that define analysis configuration,
//! inputs, outputs, and execution requirements.

use std::path::Path;

use serde::Deserialize;

use crate::error::VVError;

// =============================================================================
// Manifest Types (Pure Data - DOP Pattern)
// =============================================================================

/// Root manifest structure for analysis bundles
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Analysis")]
pub struct Manifest {
    /// Unique identifier for this analysis
    pub id: String,
    /// Semantic version string
    pub version: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Docker image to run analysis in
    pub image: String,
    /// Script to execute (after templating)
    pub entrypoint: String,
    /// Input data requirements
    #[serde(default)]
    pub inputs: Vec<InputSpec>,
    /// Expected output files
    #[serde(default)]
    pub outputs: Vec<OutputSpec>,
    /// Templates to render
    #[serde(default)]
    pub templates: Vec<TemplateSpec>,
    /// Static files to copy as-is
    #[serde(default)]
    pub static_files: Vec<String>,
    /// Resource limits for container execution
    #[serde(default)]
    pub resources: ResourceLimits,
}

/// Specification for an input dependency
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Input")]
pub struct InputSpec {
    /// Key path in project data (e.g., "materials.thermal_conductivity")
    pub key: String,
    /// Whether this input must exist
    #[serde(default = "default_true")]
    pub required: bool,
}

/// Specification for an output file
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Output")]
pub struct OutputSpec {
    /// Filename or relative path for the output
    pub key: String,
}

/// Specification for a template file
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Template")]
pub struct TemplateSpec {
    /// Source template file (e.g., "script.py.j2")
    pub source: String,
    /// Destination filename after rendering (e.g., "script.py")
    pub destination: String,
}

/// Resource limits for container execution
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Resources")]
pub struct ResourceLimits {
    /// CPU cores (fractional allowed)
    #[serde(default = "default_cpu_cores")]
    pub cpu_cores: f64,
    /// Memory limit in megabytes
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u64,
    /// Maximum execution time in seconds
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

// =============================================================================
// Default Values
// =============================================================================

fn default_true() -> bool {
    true
}

fn default_cpu_cores() -> f64 {
    1.0
}

fn default_memory_mb() -> u64 {
    512
}

fn default_timeout_seconds() -> u64 {
    300
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_cores: default_cpu_cores(),
            memory_mb: default_memory_mb(),
            timeout_seconds: default_timeout_seconds(),
        }
    }
}

// =============================================================================
// Pure Functions
// =============================================================================

/// Loads and parses a manifest from a RON file
/// Pure function with side effect (file read)
pub fn load_manifest(path: &Path) -> Result<Manifest, VVError> {
    let content = std::fs::read_to_string(path)?;
    parse_manifest(&content).map_err(|e| VVError::InvalidManifest {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Parses manifest from RON string
/// Pure function - no side effects
pub fn parse_manifest(content: &str) -> Result<Manifest, ron::error::SpannedError> {
    ron::from_str(content)
}

/// Checks if a path represents an analysis bundle (ends with .analysis)
/// Pure predicate function - checks extension only, not filesystem
pub fn is_analysis_bundle(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext == "analysis")
        .unwrap_or(false)
}

/// Returns the path to manifest.ron within an analysis bundle
/// Pure function
pub fn manifest_path(bundle_path: &Path) -> std::path::PathBuf {
    bundle_path.join("manifest.ron")
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_MANIFEST: &str = r#"
Analysis(
    id: "test_analysis",
    version: "1.0.0",
    image: "python:3.11",
    entrypoint: "script.py",
)
"#;

    const FULL_MANIFEST: &str = r#"
Analysis(
    id: "thermal_analysis",
    version: "1.0.0",
    description: Some("Steady-state thermal analysis"),
    image: "ghcr.io/org/vverdad-python:3.11",
    entrypoint: "script.py",
    inputs: [
        Input(key: "geometry.stl", required: true),
        Input(key: "materials.thermal_conductivity", required: true),
        Input(key: "parameters.ambient_temp", required: false),
    ],
    outputs: [
        Output(key: "temperature_field.bin"),
        Output(key: "max_temp.json"),
    ],
    templates: [
        Template(source: "script.py.j2", destination: "script.py"),
        Template(source: "input.json.j2", destination: "input.json"),
    ],
    static_files: [
        "materials_db.json",
    ],
    resources: Resources(
        cpu_cores: 2.0,
        memory_mb: 4096,
        timeout_seconds: 3600,
    ),
)
"#;

    #[test]
    fn test_parse_minimal_manifest() {
        let manifest = parse_manifest(MINIMAL_MANIFEST).expect("Failed to parse minimal manifest");
        assert_eq!(manifest.id, "test_analysis");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.image, "python:3.11");
        assert_eq!(manifest.entrypoint, "script.py");
        assert!(manifest.description.is_none());
        assert!(manifest.inputs.is_empty());
        assert!(manifest.outputs.is_empty());
        assert!(manifest.templates.is_empty());
        assert!(manifest.static_files.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let manifest = parse_manifest(FULL_MANIFEST).expect("Failed to parse full manifest");
        assert_eq!(manifest.id, "thermal_analysis");
        assert_eq!(
            manifest.description,
            Some("Steady-state thermal analysis".to_string())
        );
        assert_eq!(manifest.inputs.len(), 3);
        assert_eq!(manifest.outputs.len(), 2);
        assert_eq!(manifest.templates.len(), 2);
        assert_eq!(manifest.static_files.len(), 1);
        assert_eq!(manifest.resources.cpu_cores, 2.0);
        assert_eq!(manifest.resources.memory_mb, 4096);
        assert_eq!(manifest.resources.timeout_seconds, 3600);
    }

    #[test]
    fn test_input_spec_required_default() {
        let manifest = parse_manifest(FULL_MANIFEST).unwrap();
        // First two inputs should be required (explicitly set)
        assert!(manifest.inputs[0].required);
        assert!(manifest.inputs[1].required);
        // Third input is explicitly not required
        assert!(!manifest.inputs[2].required);
    }

    #[test]
    fn test_default_resource_limits() {
        let manifest = parse_manifest(MINIMAL_MANIFEST).unwrap();
        assert_eq!(manifest.resources.cpu_cores, 1.0);
        assert_eq!(manifest.resources.memory_mb, 512);
        assert_eq!(manifest.resources.timeout_seconds, 300);
    }

    #[test]
    fn test_is_analysis_bundle() {
        assert!(is_analysis_bundle(Path::new("/path/to/thermal.analysis")));
        assert!(!is_analysis_bundle(Path::new("/path/to/thermal")));
        assert!(!is_analysis_bundle(Path::new("/path/to/thermal.json")));
    }

    #[test]
    fn test_invalid_manifest_syntax() {
        let invalid = "Analysis( id: )"; // Missing value
        let result = parse_manifest(invalid);
        assert!(result.is_err());
    }
}
