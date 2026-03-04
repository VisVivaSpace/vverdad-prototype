//! VVERDAD - Data Processing Engine for Aerospace Vehicle Design
//!
//! Architecture follows:
//! - ECS (Entity Component System) via Bevy for data management
//! - DOP (Data-Oriented Programming) principles
//! - FP (Functional Programming) patterns for transformations

pub mod analysis;
pub mod config;
pub mod error;
pub mod events;
pub mod init;
pub mod node;
pub mod source;
pub mod time;
pub mod units;
pub mod value;

use std::fs;
use std::path::{Path, PathBuf};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;

use crate::analysis::discovery::{
    DiscoveredAnalyses, discover_analyses_system, handle_manifest_errors_system,
};
use crate::analysis::renderer::render_analyses_system;
use crate::analysis::runner::{
    DockerClientResource, ExecutionError, execute_analyses_system, handle_execution_errors_system,
};
use crate::analysis::validation::{
    OutputValidationError, handle_output_validation_errors_system, validate_outputs_system,
};
use crate::config::{InputType, OutputType, VVConfig};
use crate::events::*;
use crate::node::*;
use crate::source::{
    DirectorySink, DirectorySource, FileSource, OutputSink, ZipSource, copy_project_to_archive,
    copy_project_to_dir, flush_sink,
};

// =============================================================================
// System Sets (ECS Pattern: Explicit Ordering)
// =============================================================================

/// Schedule label for loading data and discovering analyses (runs once)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadSchedule;

/// Schedule label for the reactive loop: render templates, scan output, run analyses
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReactSchedule;

/// Schedule label for output validation (runs once after reactive loop)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidateSchedule;

/// System sets for controlling execution order
/// ECS Pattern: Group related systems, explicit dependencies
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProcessingStage {
    /// Load data from filesystem into ECS
    Load,
    /// Handle any load errors
    HandleLoadErrors,
    /// Discover analysis bundles in project tree
    DiscoverAnalyses,
    /// Handle any manifest errors
    HandleManifestErrors,
    /// Render satisfied templates
    RenderTemplates,
    /// Load new data from _output/
    LoadOutputData,
    /// Render analysis bundles (copy static files, render templates)
    RenderAnalyses,
    /// Execute analyses in Docker containers
    ExecuteAnalyses,
    /// Handle any execution errors
    HandleExecutionErrors,
    /// Validate that analyses produced expected outputs
    ValidateOutputs,
    /// Handle any output validation errors
    HandleOutputValidationErrors,
}

// =============================================================================
// App Entry Point
// =============================================================================

// =============================================================================
// FileSource Resource (wraps dyn FileSource for ECS)
// =============================================================================

/// Resource wrapping the FileSource enum for ECS access
#[derive(Resource)]
pub struct FileSourceResource(pub FileSource);

/// Resource wrapping the OutputSink enum for ECS access
#[derive(Resource)]
pub struct OutputSinkResource(pub OutputSink);

// =============================================================================
// App Entry Point
// =============================================================================

pub struct App {
    pub world: World,
    pub load_schedule: Schedule,
    pub react_schedule: Schedule,
    pub validate_schedule: Schedule,
    pub has_errors: bool,
}

/// Creates a new App with the given project path and default in-place output
pub fn create_app(project_path: PathBuf) -> Result<App, error::VVError> {
    create_app_with_output(project_path, OutputType::InPlace)
}

/// Creates a new App with the given project path and output type
pub fn create_app_with_output(
    project_path: PathBuf,
    output_type: OutputType,
) -> Result<App, error::VVError> {
    let mut world = World::new();

    // Detect input type and create appropriate FileSource
    let input_type = config::detect_input_type(&project_path)
        .ok_or_else(|| error::VVError::NotDirectory(project_path.clone()))?;

    let file_source = match &input_type {
        InputType::Directory(dir) => FileSource::Directory(DirectorySource::new(dir.clone())),
        InputType::ZipArchive(archive) => FileSource::Zip(ZipSource::new(archive.clone())?),
    };

    // Handle project copying based on output type
    let output_sink = match &output_type {
        OutputType::InPlace => {
            // Write directly to input directory's _output/
            let output_dir = match &input_type {
                InputType::Directory(dir) => dir.join("_output"),
                InputType::ZipArchive(archive) => archive
                    .parent()
                    .unwrap_or(archive.as_path())
                    .join("_output"),
            };
            OutputSink::Directory(DirectorySink::new(output_dir))
        }
        OutputType::Directory(dest_dir) => {
            // Copy project to destination directory
            copy_project_to_dir(&file_source, dest_dir)?;
            let output_dir = dest_dir.join("_output");
            OutputSink::Directory(DirectorySink::new(output_dir))
        }
        OutputType::Archive(archive_path) => {
            // Copy project to new archive
            let sink = copy_project_to_archive(&file_source, archive_path)?;
            OutputSink::Zip(sink)
        }
    };

    // Insert config resource with detected input and output types
    world.insert_resource(config::config_with_input_and_output(
        input_type,
        output_type,
    ));
    world.insert_resource(FileSourceResource(file_source));
    world.insert_resource(OutputSinkResource(output_sink));
    world.insert_resource(ProcessingStatus::default());

    // Initialize message resources
    world.init_resource::<Messages<LoadError>>();
    world.init_resource::<Messages<ManifestError>>();
    world.init_resource::<Messages<ValidationError>>();
    world.init_resource::<Messages<ExecutionError>>();
    world.init_resource::<Messages<OutputValidationError>>();

    // Initialize analysis discovery resource
    world.init_resource::<DiscoveredAnalyses>();

    // Initialize Docker client resource (may fail if Docker not available)
    world.init_resource::<DockerClientResource>();

    // Load schedule: runs once to load source data and discover analyses
    let mut load_schedule = Schedule::new(LoadSchedule);
    load_schedule.configure_sets(
        (
            ProcessingStage::Load,
            ProcessingStage::HandleLoadErrors,
            ProcessingStage::DiscoverAnalyses,
            ProcessingStage::HandleManifestErrors,
        )
            .chain(),
    );
    load_schedule.add_systems((
        load_data_system.in_set(ProcessingStage::Load),
        handle_load_errors_system.in_set(ProcessingStage::HandleLoadErrors),
        discover_analyses_system
            .in_set(ProcessingStage::DiscoverAnalyses)
            .run_if(should_continue),
        handle_manifest_errors_system.in_set(ProcessingStage::HandleManifestErrors),
    ));

    // React schedule: render templates, scan output, render+execute analyses
    let mut react_schedule = Schedule::new(ReactSchedule);
    react_schedule.configure_sets(
        (
            ProcessingStage::RenderTemplates,
            ProcessingStage::LoadOutputData,
            ProcessingStage::RenderAnalyses,
            ProcessingStage::ExecuteAnalyses,
            ProcessingStage::HandleExecutionErrors,
        )
            .chain(),
    );
    react_schedule.add_systems((
        render_templates_system.in_set(ProcessingStage::RenderTemplates),
        load_output_data_system.in_set(ProcessingStage::LoadOutputData),
        render_analyses_system.in_set(ProcessingStage::RenderAnalyses),
        execute_analyses_system
            .in_set(ProcessingStage::ExecuteAnalyses)
            .run_if(should_continue),
        handle_execution_errors_system.in_set(ProcessingStage::HandleExecutionErrors),
    ));

    // Validate schedule: runs once after the loop
    let mut validate_schedule = Schedule::new(ValidateSchedule);
    validate_schedule.configure_sets(
        (
            ProcessingStage::ValidateOutputs,
            ProcessingStage::HandleOutputValidationErrors,
        )
            .chain(),
    );
    validate_schedule.add_systems((
        validate_outputs_system
            .in_set(ProcessingStage::ValidateOutputs)
            .run_if(should_continue),
        handle_output_validation_errors_system
            .in_set(ProcessingStage::HandleOutputValidationErrors),
    ));

    Ok(App {
        world,
        load_schedule,
        react_schedule,
        validate_schedule,
        has_errors: false,
    })
}

/// Runs the processing pipeline with reactive rendering.
///
/// 1. Load source data and discover analyses (once)
/// 2. Clean _output/
/// 3. Initialize UnmetNeeds on all templates
/// 4. Reactive loop: run react_schedule until no work remains
/// 5. Report templates with unmet needs
/// 6. Validate outputs
/// 7. Flush output sink
pub fn run_app(app: &mut App) {
    // Phase 1: Load source data and discover analyses
    app.load_schedule.run(&mut app.world);
    if check_errors(app) {
        flush_output(app);
        return;
    }

    // Phase 2: Clean output directory (stateless — each run starts fresh)
    let output_dir = {
        let config = app.world.resource::<VVConfig>();
        config::output_path(config)
    };
    clean_output_dir(&output_dir);

    // Phase 3: Check for circular dependencies
    if let Some(data_root) = app.world.get_resource::<DataRoot>() {
        let root = data_root.0;
        let graph = build_dependency_graph(&mut app.world, root);
        let cycles = detect_cycles(&graph);
        if !cycles.is_empty() {
            for cycle in &cycles {
                let err = error::VVError::CircularDependency {
                    cycle: cycle.clone(),
                };
                eprintln!("{}", error::format_diagnostic(&err));
            }
            app.has_errors = true;
            flush_output(app);
            return;
        }
    }

    // Phase 4: Initialize UnmetNeeds + SeenOutputFiles
    app.world.init_resource::<SeenOutputFiles>();
    initialize_unmet_needs(&mut app.world);

    // Phase 4: Reactive loop — run until no work remains
    loop {
        app.react_schedule.run(&mut app.world);
        if check_errors(app) {
            break;
        }
        if !is_work_remaining(&mut app.world) {
            break;
        }
    }

    // Phase 5: Report templates with unmet needs
    report_unmet_needs(&mut app.world);

    // Phase 6: Validate outputs
    app.validate_schedule.run(&mut app.world);
    check_errors(app);

    // Phase 7: Flush output sink
    flush_output(app);
}

/// Checks ProcessingStatus for errors and updates app.has_errors. Returns true if errors found.
fn check_errors(app: &mut App) -> bool {
    let has = app.world.resource::<ProcessingStatus>().has_errors;
    app.has_errors |= has;
    has
}

/// Flushes the output sink (important for ZipSink finalization).
fn flush_output(app: &mut App) {
    if let Some(mut output_sink) = app.world.get_resource_mut::<OutputSinkResource>() {
        let _ = flush_sink(&mut output_sink.0);
    }
}

/// Reports templates whose needs were never fully met.
fn report_unmet_needs(world: &mut World) {
    let mut query =
        world.query_filtered::<(&Template, &UnmetNeeds), bevy_ecs::query::Without<Rendered>>();
    for (t, needs) in query.iter(world) {
        if !needs.remaining.is_empty() {
            let remaining: Vec<_> = needs.remaining.iter().collect();
            eprintln!(
                "Warning: template '{}' was not rendered — unmet requirements: {:?}",
                t.template_name, remaining
            );
        }
    }
}

/// Returns true if there are templates ready to render (UnmetNeeds empty, not yet Rendered).
fn is_work_remaining(world: &mut World) -> bool {
    let mut query =
        world.query_filtered::<&UnmetNeeds, bevy_ecs::query::Without<Rendered>>();
    query.iter(world).any(|needs| needs.remaining.is_empty())
}

// =============================================================================
// Run Conditions
// =============================================================================

/// Condition: Only run if no errors have occurred
fn should_continue(status: Res<ProcessingStatus>) -> bool {
    !status.has_errors
}

// =============================================================================
// Systems (ECS Pattern: Functions Over Components)
// =============================================================================

/// System: Load project data into ECS world
/// Side-effect: Reads via FileSource
fn load_data_system(
    mut commands: Commands,
    config: Res<VVConfig>,
    file_source: Res<FileSourceResource>,
    mut errors: MessageWriter<LoadError>,
) {
    let input_path = config::input_path(&config);
    let source = &file_source.0;

    let mut template_env = new_template_environment();

    match load_directory(
        &mut commands,
        &mut template_env,
        source,
        input_path.clone(),
        "",
        &config,
    ) {
        Ok(root_entity) => {
            commands.insert_resource(DataRoot(root_entity));
            commands.insert_resource(template_env);
        }
        Err(e) => {
            errors.write(LoadError {
                path: input_path,
                error: e,
            });
        }
    }
}

/// System: Handle load errors
fn handle_load_errors_system(
    mut events: MessageReader<LoadError>,
    mut status: ResMut<ProcessingStatus>,
) {
    for error in events.read() {
        eprintln!(
            "Error loading '{}'\n{}",
            error.path.display(),
            error::format_diagnostic(&error.error)
        );
        mark_error(&mut status);
    }
}

/// System: Validate that all template requirements are satisfied
/// Pure validation over ECS queries
///
/// NOTE: Currently disabled — users can structure projects however they want.
/// Retained for potential future opt-in strict mode.
#[allow(dead_code)]
fn validate_dependencies_system(
    data_root: Option<Res<DataRoot>>,
    directories: Query<&Directory>,
    provides_query: Query<&Provides>,
    requires_query: Query<&Requires>,
    mut errors: MessageWriter<ValidationError>,
) {
    let Some(data_root) = data_root else {
        return; // No data loaded, skip validation
    };

    let root = data_root.0;

    // FP Pattern: Collect data using pure functions
    let provides = collect_all_provides(root, &directories, &provides_query);
    let requires = collect_all_requires(root, &directories, &requires_query);

    // FP Pattern: Pure validation function
    if let Err(error::VVError::UnmetDependencies(unmet)) =
        validate_dependencies(&provides, &requires)
    {
        errors.write(ValidationError { unmet });
    }
}

/// System: Handle validation errors
///
/// NOTE: Currently disabled — see `validate_dependencies_system`.
#[allow(dead_code)]
fn handle_validation_errors_system(
    mut events: MessageReader<ValidationError>,
    mut status: ResMut<ProcessingStatus>,
) {
    for error in events.read() {
        let err = error::VVError::UnmetDependencies(error.unmet.clone());
        eprintln!("{}", error::format_diagnostic(&err));
        mark_error(&mut status);
    }
}

/// Exclusive system: Renders templates whose UnmetNeeds are empty.
///
/// Read phase: builds EcsContext from DataRoot, renders ready templates to strings.
/// Write phase: writes rendered strings to OutputSink, inserts Rendered markers.
fn render_templates_system(world: &mut World) {
    // Early exit if prerequisites missing
    let has_prereqs = world.get_resource::<DataRoot>().is_some()
        && world.get_resource::<TemplateEnvironment>().is_some()
        && world.get_resource::<ProcessingStatus>().is_some_and(|s| !s.has_errors);
    if !has_prereqs {
        return;
    }

    // Create query state (requires &mut World)
    let mut ready_templates =
        world.query_filtered::<(Entity, &Template, &UnmetNeeds), Without<Rendered>>();

    // Read phase: render ready templates to strings (immutable world borrow)
    let results: Vec<(Entity, String, PathBuf, Result<String, error::VVError>)> = {
        let root = world.resource::<DataRoot>().0;
        let config = world.resource::<VVConfig>();
        let output_dir = config::output_path(config);
        let tmpl_env = world.resource::<TemplateEnvironment>();
        let env = &tmpl_env.0;

        // Build EcsContext for lazy serialization
        let ctx = EcsContext {
            entity: bevy_entity_ptr::BoundEntity::new(root, world),
        };

        // Query templates with empty UnmetNeeds (ready to render)
        let mut results = Vec::new();
        for (entity, template, needs) in ready_templates.iter(world) {
            if !needs.remaining.is_empty() {
                continue;
            }

            let output_path =
                crate::value::build_output_path(&output_dir, &template.template_name);
            let render_result =
                crate::value::render_template(&template.template_name, &ctx, env);
            results.push((
                entity,
                template.template_name.clone(),
                output_path,
                render_result,
            ));
        }
        results
    };

    // Write phase: write strings to OutputSink, insert Rendered markers
    for (entity, template_name, output_path, result) in results {
        match result {
            Ok(rendered) => {
                let write_result = {
                    let sink = &mut world.resource_mut::<OutputSinkResource>().0;
                    source::write_file(sink, &output_path, rendered.as_bytes())
                };
                match write_result {
                    Ok(()) => {
                        world.entity_mut(entity).insert(Rendered);
                    }
                    Err(e) => {
                        eprintln!(
                            "Error writing '{}'\n{}",
                            template_name,
                            error::format_diagnostic(&e)
                        );
                        world.resource_mut::<ProcessingStatus>().has_errors = true;
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Error rendering '{}'\n{}",
                    template_name,
                    error::format_diagnostic(&e)
                );
                world.resource_mut::<ProcessingStatus>().has_errors = true;
            }
        }
    }
}

/// Exclusive system: Scans _output/ for new data files, loads them,
/// and updates UnmetNeeds on all unrendered templates.
fn load_output_data_system(world: &mut World) {
    let has_prereqs = world.get_resource::<DataRoot>().is_some()
        && world.get_resource::<ProcessingStatus>().is_some_and(|s| !s.has_errors);
    if !has_prereqs {
        return;
    }

    let root = world.resource::<DataRoot>().0;
    let output_dir = {
        let config = world.resource::<VVConfig>();
        config::output_path(config)
    };

    let new_provides = load_output_data_and_provides(world, &output_dir, root);

    if !new_provides.is_empty() {
        update_unmet_needs(world, &new_provides);
    }
}

/// Removes all contents of an output directory so each run starts fresh.
///
/// Silently ignores errors (missing directory, permission issues) since
/// this is best-effort cleanup before writing new output.
fn clean_output_dir(output_dir: &Path) {
    let _ = fs::remove_dir_all(output_dir);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Creates a temporary project directory with data files and templates.
    /// `files` is a list of (relative_path, content) pairs.
    /// Returns (TempDir, PathBuf) — keep TempDir alive for the test's duration.
    fn make_project(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        for (rel_path, content) in files {
            let path = root.join(rel_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }
        (tmp, root)
    }

    #[test]
    fn test_validation_passes_when_all_deps_met() {
        let provides = vec![
            "propulsion.thrust".to_string(),
            "propulsion.isp".to_string(),
            "power.P0".to_string(),
        ];
        let requires = vec!["propulsion".to_string(), "power".to_string()];

        let result = validate_dependencies(&provides, &requires);
        assert!(
            result.is_ok(),
            "Validation should pass when all deps are met"
        );
    }

    #[test]
    fn test_validation_fails_with_missing_deps() {
        let provides = vec!["propulsion.thrust".to_string()];
        let requires = vec!["propulsion".to_string(), "power".to_string()];

        let result = validate_dependencies(&provides, &requires);
        assert!(
            result.is_err(),
            "Validation should fail when deps are missing"
        );

        if let Err(error::VVError::UnmetDependencies(unmet)) = result {
            assert!(
                unmet.contains(&"power".to_string()),
                "power should be in unmet deps"
            );
        } else {
            panic!("Expected UnmetDependencies error");
        }
    }

    #[test]
    fn test_validation_handles_exact_matches() {
        // Test case where a provided key exactly matches a required key
        let provides = vec!["config".to_string()];
        let requires = vec!["config".to_string()];

        let result = validate_dependencies(&provides, &requires);
        assert!(result.is_ok(), "Validation should pass for exact matches");
    }

    // =========================================================================
    // Iterative Rendering Tests (Phase 5)
    // =========================================================================

    #[test]
    fn test_iterative_rendering_template_produces_data() {
        // Template A has no requirements and renders a JSON data file.
        // Template B requires "step_a" which is provided by A's output.
        // Both should render across 2 iterations.
        let (_tmp, root) = make_project(&[
            ("source.json", r#"{"value": 42}"#),
            ("step_a.json.j2", r#"{"computed": {{ source.value }}}"#),
            ("report.md.j2", "Computed: {{ step_a.computed }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // step_a.json should have been rendered (deps satisfied by source)
        let step_a_output = root.join("_output/step_a.json");
        assert!(
            step_a_output.exists(),
            "step_a.json should be rendered to _output/"
        );
        let step_a_content = fs::read_to_string(&step_a_output).unwrap();
        assert!(
            step_a_content.contains("42"),
            "step_a.json should contain computed value"
        );

        // report.md should have been rendered using step_a data from _output/
        let report_output = root.join("_output/report.md");
        assert!(
            report_output.exists(),
            "report.md should be rendered to _output/"
        );
        let report_content = fs::read_to_string(&report_output).unwrap();
        assert!(
            report_content.contains("Computed: 42"),
            "report.md should contain 'Computed: 42', got: {}",
            report_content
        );

        assert!(!app.has_errors, "App should have no errors");
    }

    #[test]
    fn test_iterative_rendering_chain() {
        // Three-template chain: A → B → C across 3 iterations.
        // A: no deps beyond input, produces step_a.json
        // B: needs step_a, produces step_b.json
        // C: needs step_b, produces final report
        let (_tmp, root) = make_project(&[
            ("input.json", r#"{"x": 10}"#),
            ("step_a.json.j2", r#"{"a_val": {{ input.x }}}"#),
            ("step_b.json.j2", r#"{"b_val": {{ step_a.a_val }}}"#),
            ("report.md.j2", "Final: {{ step_b.b_val }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // All three outputs should exist
        assert!(
            root.join("_output/step_a.json").exists(),
            "step_a.json should exist"
        );
        assert!(
            root.join("_output/step_b.json").exists(),
            "step_b.json should exist"
        );

        let report = fs::read_to_string(root.join("_output/report.md")).unwrap();
        assert!(
            report.contains("Final: 10"),
            "report.md should contain 'Final: 10', got: {}",
            report
        );

        assert!(!app.has_errors, "App should have no errors");
    }

    #[test]
    fn test_iterative_rendering_non_data_no_extra_loop() {
        // Template produces a .md file (not a data extension that feeds back).
        // No second iteration should be needed — the loop terminates after one pass
        // since no new data files are produced.
        let (_tmp, root) = make_project(&[
            ("data.json", r#"{"greeting": "hello"}"#),
            ("output.md.j2", "Say: {{ data.greeting }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        let output = root.join("_output/output.md");
        assert!(output.exists(), "output.md should be rendered");

        let content = fs::read_to_string(&output).unwrap();
        assert!(
            content.contains("Say: hello"),
            "output.md should contain 'Say: hello', got: {}",
            content
        );

        // Verify all templates were rendered
        let world = &mut app.world;
        let mut unrendered =
            world.query_filtered::<&Template, bevy_ecs::query::Without<Rendered>>();
        let unrendered_count = unrendered.iter(world).count();
        assert_eq!(unrendered_count, 0, "All templates should be rendered");

        assert!(!app.has_errors, "App should have no errors");
    }

    #[test]
    fn test_unsatisfied_template_warns() {
        // Template requires data that nobody provides.
        // Should complete without panic; template should remain unrendered.
        let (_tmp, root) = make_project(&[
            ("data.json", r#"{"value": 1}"#),
            ("needs_missing.md.j2", "Missing: {{ nonexistent.key }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // The output file should NOT exist (template wasn't rendered)
        let output = root.join("_output/needs_missing.md");
        assert!(
            !output.exists(),
            "needs_missing.md should not be rendered (unsatisfied deps)"
        );

        // The template entity should still lack Rendered marker
        let world = &mut app.world;
        let mut unrendered =
            world.query_filtered::<&Template, bevy_ecs::query::Without<Rendered>>();
        let unrendered_count = unrendered.iter(world).count();
        assert_eq!(unrendered_count, 1, "One template should remain unrendered");

        // Should not have fatal errors (unrendered is a warning, not an error)
        assert!(
            !app.has_errors,
            "Unsatisfied template should warn, not error"
        );
    }

    #[test]
    fn test_output_preserves_subdirectory_structure() {
        // Template renders _output/sub/result.json; that data should be
        // accessible as {{ sub.result.key }} in a subsequent iteration.
        let (_tmp, root) = make_project(&[
            ("source.json", r#"{"value": 99}"#),
            // Template that writes into a subdirectory via namespace separator
            ("sub::result.json.j2", r#"{"key": {{ source.value }}}"#),
            ("report.md.j2", "Key: {{ sub.result.key }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // sub/result.json should exist in _output/
        let result_output = root.join("_output/sub/result.json");
        assert!(
            result_output.exists(),
            "sub/result.json should be rendered to _output/sub/"
        );

        // report.md should have used the sub-directory data
        let report = fs::read_to_string(root.join("_output/report.md")).unwrap();
        assert!(
            report.contains("Key: 99"),
            "report.md should contain 'Key: 99', got: {}",
            report
        );

        assert!(!app.has_errors, "App should have no errors");
    }

    #[test]
    fn test_output_overwrites_source_on_collision() {
        // Source has data.json with x=1. A template overwrites it with x=2.
        // A second template that can only render in iteration 2 (because it
        // also requires "computed", which is produced by another template)
        // should see x=2 from the overwritten output, not x=1 from source.
        //
        // Iteration 1: data.json.j2 → _output/data.json {x:2},
        //              computed.json.j2 → _output/computed.json {ready:true}
        // Output scan: loads both, data entity overwritten with x=2
        // Iteration 2: report.md.j2 renders (computed now available), sees x=2
        let (_tmp, root) = make_project(&[
            ("data.json", r#"{"x": 1}"#),
            ("data.json.j2", r#"{"x": 2}"#),
            ("computed.json.j2", r#"{"ready": true}"#),
            // Requires both "data" and "computed" — "computed" not available until iter 2
            (
                "report.md.j2",
                "x is {{ data.x }}, ready: {{ computed.ready }}",
            ),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // _output/data.json should have x=2
        let output_data = root.join("_output/data.json");
        assert!(output_data.exists(), "data.json should be rendered");

        // report.md should see x=2 (output overwrote source in ECS tree)
        let report = fs::read_to_string(root.join("_output/report.md")).unwrap();
        assert!(
            report.contains("x is 2"),
            "report.md should contain 'x is 2' (output overwrites source), got: {}",
            report
        );

        assert!(!app.has_errors, "App should have no errors");
    }

    #[test]
    fn test_clean_output_removes_stale() {
        // Pre-populate _output/ with stale content, then run.
        // Stale content should be gone after the run.
        let (_tmp, root) = make_project(&[
            ("data.json", r#"{"value": 1}"#),
            ("report.md.j2", "Value: {{ data.value }}"),
        ]);

        // Create stale _output/ content
        let output_dir = root.join("_output");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("stale_file.txt"), "old data").unwrap();
        fs::create_dir_all(output_dir.join("stale_subdir")).unwrap();
        fs::write(
            output_dir.join("stale_subdir/nested.txt"),
            "nested old data",
        )
        .unwrap();
        assert!(output_dir.join("stale_file.txt").exists());

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        // Stale content should be gone
        assert!(
            !output_dir.join("stale_file.txt").exists(),
            "Stale file should be removed by clean_output_dir"
        );
        assert!(
            !output_dir.join("stale_subdir").exists(),
            "Stale subdirectory should be removed by clean_output_dir"
        );

        // New output should exist
        let report = root.join("_output/report.md");
        assert!(report.exists(), "report.md should be rendered");
        let content = fs::read_to_string(&report).unwrap();
        assert!(
            content.contains("Value: 1"),
            "report.md should contain 'Value: 1', got: {}",
            content
        );

        assert!(!app.has_errors, "App should have no errors");
    }

    // =========================================================================
    // Cycle Detection Tests (Phase 4)
    // =========================================================================

    #[test]
    fn test_circular_dependency_detected() {
        // Template A produces a.json, requires "b"
        // Template B produces b.json, requires "a"
        // This is a cycle: A → B → A
        let (_tmp, root) = make_project(&[
            ("a.json.j2", r#"{"val": {{ b.val }} }"#),
            ("b.json.j2", r#"{"val": {{ a.val }} }"#),
        ]);

        let mut app = create_app(root).expect("create_app");
        run_app(&mut app);

        assert!(
            app.has_errors,
            "Circular dependency should be detected as an error"
        );

        // Neither template should be rendered
        let world = &mut app.world;
        let mut rendered = world.query_filtered::<&Template, Without<Rendered>>();
        let unrendered_count = rendered.iter(world).count();
        assert_eq!(
            unrendered_count, 2,
            "Neither template should be rendered when cycle detected"
        );
    }

    #[test]
    fn test_linear_dependency_no_cycle() {
        // A → B → C (linear chain, no cycle)
        // A has no deps, produces a.json
        // B requires "a", produces b.json
        // C requires "b", produces report.md
        let (_tmp, root) = make_project(&[
            ("source.json", r#"{"seed": 1}"#),
            ("a.json.j2", r#"{"val": {{ source.seed }}}"#),
            ("b.json.j2", r#"{"val": {{ a.val }}}"#),
            ("report.md.j2", "Result: {{ b.val }}"),
        ]);

        let mut app = create_app(root.clone()).expect("create_app");
        run_app(&mut app);

        assert!(!app.has_errors, "Linear chain should have no errors");

        let report = fs::read_to_string(root.join("_output/report.md")).unwrap();
        assert!(
            report.contains("Result: 1"),
            "report.md should contain 'Result: 1', got: {}",
            report
        );
    }
}
