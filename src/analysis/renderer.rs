//! Analysis bundle rendering system
//!
//! Copies static files and renders templates from .analysis bundles
//! to prepare them for Docker execution.

use std::fs;
use std::path::{Path, PathBuf};

use bevy_ecs::prelude::*;

use crate::analysis::discovery::AnalysisBundle;
use crate::analysis::manifest::{Manifest, TemplateSpec};
use crate::error::VVError;
use crate::node::NodeInfo;

// =============================================================================
// Components (Pure Data)
// =============================================================================

/// Component marking an analysis that has been rendered and is ready for execution
#[derive(Component)]
pub struct RenderedAnalysis {
    /// Path to the rendered analysis directory in output/
    pub output_path: PathBuf,
    /// Path to the entrypoint script (relative to output_path)
    pub entrypoint: PathBuf,
}

// =============================================================================
// Systems
// =============================================================================

/// Exclusive system: Render all discovered analysis bundles using EcsContext.
pub fn render_analyses_system(world: &mut World) {
    // Early exit if prerequisites missing
    let has_prereqs = world.get_resource::<crate::node::DataRoot>().is_some()
        && world
            .get_resource::<crate::events::ProcessingStatus>()
            .is_some_and(|s| !s.has_errors);
    if !has_prereqs {
        return;
    }

    // Create query state (requires &mut World)
    let mut analyses = world.query_filtered::<
        (Entity, &NodeInfo, &AnalysisBundle),
        Without<RenderedAnalysis>,
    >();

    // Read phase: collect analysis render tasks
    let tasks: Vec<(Entity, String, Result<RenderedAnalysis, VVError>)> = {
        let root = world.resource::<crate::node::DataRoot>().0;
        let config = world.resource::<crate::config::VVConfig>();
        let output_path = crate::config::output_path(config);
        let input_path = crate::config::input_path(config);

        let ctx = crate::node::EcsContext {
            entity: bevy_entity_ptr::BoundEntity::new(root, world),
        };

        analyses
            .iter(world)
            .map(|(entity, node_info, bundle)| {
                let result = render_analysis_bundle(
                    &node_info.path,
                    &bundle.manifest,
                    output_path.as_path(),
                    input_path.as_path(),
                    &ctx,
                );
                (entity, bundle.manifest.id.clone(), result)
            })
            .collect()
    };

    // Write phase: insert RenderedAnalysis components or report errors
    for (entity, analysis_id, result) in tasks {
        match result {
            Ok(rendered) => {
                world.entity_mut(entity).insert(rendered);
            }
            Err(error) => {
                eprintln!(
                    "Error rendering analysis '{}'\n{}",
                    analysis_id,
                    crate::error::format_diagnostic(&error)
                );
                world
                    .resource_mut::<crate::events::ProcessingStatus>()
                    .has_errors = true;
            }
        }
    }
}

// =============================================================================
// Path Validation
// =============================================================================

/// Validates that a resolved path stays within the base directory.
///
/// Joins `requested` to `base`, normalizes both paths (resolving symlinks
/// where possible), and checks that the result starts with `base`.
fn validate_path_within(base: &Path, requested: &str) -> Result<PathBuf, VVError> {
    let joined = base.join(requested);

    // Canonicalize base (must exist)
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());

    // Canonicalize joined path; if it doesn't exist yet, canonicalize
    // the longest existing ancestor and append the remaining components.
    let canonical_joined = joined
        .canonicalize()
        .unwrap_or_else(|_| canonicalize_best_effort(&joined));

    if !canonical_joined.starts_with(&canonical_base) {
        return Err(VVError::PathTraversal(joined));
    }

    Ok(joined)
}

/// Canonicalize as much of the path as possible.
///
/// Walks from the full path upward until an existing ancestor is found,
/// canonicalizes that ancestor, then re-appends the non-existent tail.
/// This correctly resolves symlinks in existing path prefixes (e.g.,
/// macOS `/var/folders` → `/private/var/folders`).
fn canonicalize_best_effort(path: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut current = path.to_path_buf();

    loop {
        if let Ok(canonical) = current.canonicalize() {
            let mut result = canonical;
            for component in tail.into_iter().rev() {
                result.push(component);
            }
            return result;
        }
        if let Some(file_name) = current.file_name() {
            tail.push(file_name.to_os_string());
            current.pop();
        } else {
            break;
        }
    }

    // Nothing could be canonicalized; normalize lexically as fallback
    normalize_lexical(path)
}

/// Lexically normalize a path by resolving `.` and `..` components
/// without touching the filesystem.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if components.last().is_some_and(|c| {
                    !matches!(c, std::path::Component::ParentDir)
                        && !matches!(c, std::path::Component::RootDir)
                }) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            std::path::Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

// =============================================================================
// Pure Functions
// =============================================================================

/// Renders an analysis bundle to the output directory
///
/// - Creates the output directory structure
/// - Copies static files
/// - Renders templates with project data
pub fn render_analysis_bundle<C: serde::Serialize>(
    bundle_path: &Path,
    manifest: &Manifest,
    output_root: &Path,
    input_root: &Path,
    context: &C,
) -> Result<RenderedAnalysis, VVError> {
    // Calculate output path (mirror the input structure)
    let relative_path = bundle_path.strip_prefix(input_root).unwrap_or(bundle_path);
    let output_path = output_root.join(relative_path);

    // Create output directory
    fs::create_dir_all(&output_path)?;

    // Copy static files
    copy_static_files(bundle_path, &output_path, &manifest.static_files)?;

    // Render templates
    render_analysis_templates(bundle_path, &output_path, &manifest.templates, context)?;

    // Copy manifest to output (for reference)
    let manifest_src = bundle_path.join("manifest.ron");
    let manifest_dst = output_path.join("manifest.ron");
    if manifest_src.exists() {
        fs::copy(&manifest_src, &manifest_dst)?;
    }

    Ok(RenderedAnalysis {
        output_path,
        entrypoint: PathBuf::from(&manifest.entrypoint),
    })
}

/// Copies static files from bundle to output directory
pub fn copy_static_files(
    bundle_path: &Path,
    output_path: &Path,
    static_files: &[String],
) -> Result<(), VVError> {
    for filename in static_files {
        // Validate paths stay within their respective directories
        let src = validate_path_within(bundle_path, filename)?;
        let dst = validate_path_within(output_path, filename)?;

        if !src.exists() {
            return Err(VVError::MissingStaticFile {
                bundle: bundle_path.to_path_buf(),
                file: filename.clone(),
            });
        }

        // Create parent directories if needed
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&src, &dst)?;
    }
    Ok(())
}

/// Renders template files from bundle to output directory
pub fn render_analysis_templates<C: serde::Serialize>(
    bundle_path: &Path,
    output_path: &Path,
    templates: &[TemplateSpec],
    context: &C,
) -> Result<(), VVError> {
    let mut env = minijinja::Environment::new();
    crate::units::register_filters(&mut env);
    crate::time::register_filters(&mut env);

    for template_spec in templates {
        // Validate paths stay within their respective directories
        let src = validate_path_within(bundle_path, &template_spec.source)?;
        let dst = validate_path_within(output_path, &template_spec.destination)?;

        if !src.exists() {
            return Err(VVError::MissingTemplateFile {
                bundle: bundle_path.to_path_buf(),
                file: template_spec.source.clone(),
            });
        }

        // Load and render template
        let template_content = fs::read_to_string(&src)?;
        env.add_template_owned(template_spec.source.clone(), template_content)?;

        let template = env.get_template(&template_spec.source)?;
        let rendered = template.render(context)?;

        // Create parent directories if needed
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&dst, rendered)?;
    }
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::manifest::parse_manifest;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_context() -> crate::value::Value {
        use crate::value::Value;
        let mut propulsion = HashMap::new();
        propulsion.insert("thrust".to_string(), Value::String("100 N".into()));
        propulsion.insert("isp".to_string(), Value::String("323 s".into()));

        let mut root = HashMap::new();
        root.insert("propulsion".to_string(), Value::Map(propulsion));
        Value::Map(root)
    }

    fn create_test_manifest() -> Manifest {
        parse_manifest(
            r#"
Analysis(
    id: "test_analysis",
    version: "1.0.0",
    image: "python:3.11",
    entrypoint: "script.py",
    templates: [
        Template(source: "script.py.j2", destination: "script.py"),
    ],
    static_files: [
        "data.json",
    ],
)
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_copy_static_files() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        // Create a static file
        fs::write(bundle_path.join("data.json"), r#"{"key": "value"}"#).unwrap();

        let result = copy_static_files(&bundle_path, &output_path, &["data.json".to_string()]);
        assert!(result.is_ok());
        assert!(output_path.join("data.json").exists());

        let content = fs::read_to_string(output_path.join("data.json")).unwrap();
        assert!(content.contains("key"));
    }

    #[test]
    fn test_copy_static_files_missing_file() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        let result = copy_static_files(&bundle_path, &output_path, &["missing.json".to_string()]);
        assert!(matches!(result, Err(VVError::MissingStaticFile { .. })));
    }

    #[test]
    fn test_render_analysis_templates() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        // Create a template file
        fs::write(
            bundle_path.join("script.py.j2"),
            "thrust = {{ propulsion.thrust }}",
        )
        .unwrap();

        let templates = vec![TemplateSpec {
            source: "script.py.j2".to_string(),
            destination: "script.py".to_string(),
        }];

        let context = create_test_context();
        let result = render_analysis_templates(&bundle_path, &output_path, &templates, &context);
        assert!(result.is_ok());
        assert!(output_path.join("script.py").exists());

        let content = fs::read_to_string(output_path.join("script.py")).unwrap();
        assert!(content.contains("thrust = 100 N"));
    }

    #[test]
    fn test_render_analysis_templates_missing_template() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        let templates = vec![TemplateSpec {
            source: "missing.py.j2".to_string(),
            destination: "missing.py".to_string(),
        }];

        let context = create_test_context();
        let result = render_analysis_templates(&bundle_path, &output_path, &templates, &context);
        assert!(matches!(result, Err(VVError::MissingTemplateFile { .. })));
    }

    #[test]
    fn test_copy_static_files_path_traversal() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        // Attempt to read outside the bundle via ../
        let result = copy_static_files(
            &bundle_path,
            &output_path,
            &["../../../etc/passwd".to_string()],
        );
        assert!(
            matches!(result, Err(VVError::PathTraversal(_))),
            "Should reject path traversal, got: {:?}",
            result
        );
    }

    #[test]
    fn test_render_templates_source_path_traversal() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        let templates = vec![TemplateSpec {
            source: "../../../etc/passwd".to_string(),
            destination: "output.txt".to_string(),
        }];

        let context = create_test_context();
        let result = render_analysis_templates(&bundle_path, &output_path, &templates, &context);
        assert!(
            matches!(result, Err(VVError::PathTraversal(_))),
            "Should reject source path traversal, got: {:?}",
            result
        );
    }

    #[test]
    fn test_render_templates_destination_path_traversal() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("test.analysis");
        let output_path = temp.path().join("output");
        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_path).unwrap();

        // Create a valid template source
        fs::write(bundle_path.join("template.j2"), "hello").unwrap();

        let templates = vec![TemplateSpec {
            source: "template.j2".to_string(),
            destination: "../../../tmp/evil.txt".to_string(),
        }];

        let context = create_test_context();
        let result = render_analysis_templates(&bundle_path, &output_path, &templates, &context);
        assert!(
            matches!(result, Err(VVError::PathTraversal(_))),
            "Should reject destination path traversal, got: {:?}",
            result
        );
    }

    #[test]
    fn test_render_analysis_bundle() {
        let temp = TempDir::new().unwrap();
        let input_root = temp.path().join("project");
        let output_root = temp.path().join("output");
        let bundle_path = input_root.join("subsystem/test.analysis");

        fs::create_dir_all(&bundle_path).unwrap();
        fs::create_dir_all(&output_root).unwrap();

        // Create manifest
        fs::write(
            bundle_path.join("manifest.ron"),
            r#"Analysis(id: "test", version: "1.0.0", image: "python:3.11", entrypoint: "script.py")"#,
        )
        .unwrap();

        // Create static file
        fs::write(bundle_path.join("data.json"), r#"{"key": "value"}"#).unwrap();

        // Create template
        fs::write(
            bundle_path.join("script.py.j2"),
            "thrust = {{ propulsion.thrust }}",
        )
        .unwrap();

        let manifest = create_test_manifest();
        let context = create_test_context();

        let result = render_analysis_bundle(
            &bundle_path,
            &manifest,
            &output_root,
            &input_root,
            &context,
        );

        assert!(result.is_ok());
        let rendered = result.unwrap();

        // Check output directory was created
        assert!(rendered.output_path.exists());
        assert_eq!(rendered.entrypoint, PathBuf::from("script.py"));

        // Check static file was copied
        assert!(rendered.output_path.join("data.json").exists());

        // Check template was rendered
        assert!(rendered.output_path.join("script.py").exists());
        let content = fs::read_to_string(rendered.output_path.join("script.py")).unwrap();
        assert!(content.contains("thrust = 100 N"));
    }
}
