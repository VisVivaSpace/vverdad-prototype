//! Discovery system for analysis bundles
//!
//! Scans the project directory tree for .analysis directories,
//! loads their manifests, and creates ECS entities.

use std::path::{Path, PathBuf};

use bevy_ecs::prelude::*;

use crate::analysis::manifest::{Manifest, is_analysis_bundle, load_manifest, manifest_path};
use crate::error::VVError;
use crate::events::ManifestError;
use crate::node::{Directory, NodeInfo};

// =============================================================================
// Components (Pure Data)
// =============================================================================

/// Component marking an entity as an analysis bundle
/// ECS Pattern: Required component ensures NodeInfo is always present
#[derive(Component)]
#[require(NodeInfo)]
pub struct AnalysisBundle {
    pub manifest: Manifest,
}

// =============================================================================
// Resources
// =============================================================================

/// Resource tracking all discovered analyses
#[derive(Resource, Default)]
pub struct DiscoveredAnalyses {
    pub analyses: Vec<Entity>,
}

// =============================================================================
// Systems
// =============================================================================

/// System: Discover analysis bundles in the loaded project tree
/// Reads NodeInfo components, checks for .analysis directories
pub fn discover_analyses_system(
    mut commands: Commands,
    nodes: Query<(Entity, &NodeInfo), With<Directory>>,
    mut errors: MessageWriter<ManifestError>,
    mut discovered: ResMut<DiscoveredAnalyses>,
) {
    for (entity, node_info) in nodes.iter() {
        if is_analysis_bundle(&node_info.path) {
            match load_analysis_bundle(&node_info.path) {
                Ok(manifest) => {
                    commands.entity(entity).insert(AnalysisBundle { manifest });
                    discovered.analyses.push(entity);
                }
                Err(error) => {
                    errors.write(ManifestError {
                        path: node_info.path.clone(),
                        error,
                    });
                }
            }
        }
    }
}

/// System: Handle manifest errors
pub fn handle_manifest_errors_system(
    mut events: MessageReader<ManifestError>,
    mut status: ResMut<crate::events::ProcessingStatus>,
) {
    for error in events.read() {
        eprintln!(
            "Error loading manifest in '{}'\n{}",
            error.path.display(),
            crate::error::format_diagnostic(&error.error)
        );
        crate::events::mark_error(&mut status);
    }
}

// =============================================================================
// Pure Functions
// =============================================================================

/// Loads an analysis bundle from a directory path
/// Returns the parsed manifest or an error
fn load_analysis_bundle(bundle_path: &Path) -> Result<Manifest, VVError> {
    let manifest_file = manifest_path(bundle_path);

    if !manifest_file.exists() {
        return Err(VVError::MissingManifest(bundle_path.to_path_buf()));
    }

    load_manifest(&manifest_file)
}

/// Collects all analysis bundle paths from a directory tree (recursive)
/// Pure function - transforms directory entries to paths
pub fn find_analysis_bundles(root: &std::path::Path) -> Vec<PathBuf> {
    let mut bundles = Vec::new();
    find_bundles_recursive(root, &mut bundles);
    bundles
}

fn find_bundles_recursive(dir: &std::path::Path, bundles: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if is_analysis_bundle(&path) {
                bundles.push(path);
            } else {
                find_bundles_recursive(&path, bundles);
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_manifest() -> &'static str {
        r#"Analysis(
    id: "test_analysis",
    version: "1.0.0",
    image: "python:3.11",
    entrypoint: "script.py",
)"#
    }

    #[test]
    fn test_discovered_analyses_resource() {
        let mut discovered = DiscoveredAnalyses::default();
        assert!(discovered.analyses.is_empty());
        assert_eq!(discovered.analyses.len(), 0);

        // Add a fake entity
        discovered.analyses.push(Entity::from_bits(1));
        assert!(!discovered.analyses.is_empty());
        assert_eq!(discovered.analyses.len(), 1);
    }

    #[test]
    fn test_find_analysis_bundles() {
        let temp = TempDir::new().unwrap();

        // Create nested structure with analysis bundle
        let analysis_dir = temp.path().join("subsystem/thermal.analysis");
        fs::create_dir_all(&analysis_dir).unwrap();
        fs::write(analysis_dir.join("manifest.ron"), create_test_manifest()).unwrap();

        // Create regular directory (not an analysis)
        let regular_dir = temp.path().join("subsystem/data");
        fs::create_dir_all(&regular_dir).unwrap();

        let bundles = find_analysis_bundles(temp.path());
        assert_eq!(bundles.len(), 1);
        assert!(bundles[0].ends_with("thermal.analysis"));
    }

    #[test]
    fn test_load_analysis_bundle_success() {
        let temp = TempDir::new().unwrap();
        let analysis_dir = temp.path().join("test.analysis");
        fs::create_dir_all(&analysis_dir).unwrap();
        fs::write(analysis_dir.join("manifest.ron"), create_test_manifest()).unwrap();

        let manifest = load_analysis_bundle(&analysis_dir).expect("Should load successfully");
        assert_eq!(manifest.id, "test_analysis");
    }

    #[test]
    fn test_load_analysis_bundle_missing_manifest() {
        let temp = TempDir::new().unwrap();
        let analysis_dir = temp.path().join("test.analysis");
        fs::create_dir_all(&analysis_dir).unwrap();
        // No manifest.ron file

        let result = load_analysis_bundle(&analysis_dir);
        assert!(matches!(result, Err(VVError::MissingManifest(_))));
    }

    #[test]
    fn test_load_analysis_bundle_invalid_manifest() {
        let temp = TempDir::new().unwrap();
        let analysis_dir = temp.path().join("test.analysis");
        fs::create_dir_all(&analysis_dir).unwrap();
        fs::write(analysis_dir.join("manifest.ron"), "invalid ron {{{").unwrap();

        let result = load_analysis_bundle(&analysis_dir);
        assert!(matches!(result, Err(VVError::InvalidManifest { .. })));
    }
}
