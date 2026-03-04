//! Node module - ECS components and pure functions for file system representation
//!
//! Follows Data-Oriented Programming principles:
//! - Components are pure data (no behavior)
//! - Functions are pure transformations over data
//! - Composition over inheritance via marker components

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    config::VVConfig,
    error::VVError,
    source::{self, FileSource},
    value::Value,
};
use bevy_ecs::prelude::{Commands, Component, Entity, Query, Resource, Without, World};
use bevy_entity_ptr::{BoundEntity, EntityHandle};

// =============================================================================
// Resources (ECS singletons)
// =============================================================================

/// Wraps the Minijinja template environment as an ECS Resource
#[derive(Resource)]
pub struct TemplateEnvironment(pub minijinja::Environment<'static>);

impl Default for TemplateEnvironment {
    fn default() -> Self {
        Self(minijinja::Environment::new())
    }
}

/// Creates a new TemplateEnvironment with unit and time filters registered
pub fn new_template_environment() -> TemplateEnvironment {
    let mut env = minijinja::Environment::new();
    crate::units::register_filters(&mut env);
    crate::time::register_filters(&mut env);
    TemplateEnvironment(env)
}

/// Loads a template from the given path using a FileSource
pub fn load_template(
    env: &mut TemplateEnvironment,
    source: &FileSource,
    path: &Path,
    name: &str,
) -> Result<(), VVError> {
    let content = source::read_file(source, path)?;
    let content_str = String::from_utf8(content)?;
    env.0.add_template_owned(name.to_string(), content_str)?;
    Ok(())
}

/// Returns the undeclared variables (requirements) for a named template
pub fn template_requires(
    env: &mut TemplateEnvironment,
    name: &str,
) -> Result<Vec<String>, VVError> {
    let tmpl = env.0.get_template(name)?;
    let requires: Vec<String> = tmpl.undeclared_variables(false).into_iter().collect();
    Ok(requires)
}

/// Root entity for the data tree
#[derive(Resource)]
pub struct DataRoot(pub Entity);


// =============================================================================
// Components (Pure Data - ECS Pattern: Small, Focused Components)
// =============================================================================

/// Core node component with path and name (shared by all node types)
#[derive(Component, Default, Clone)]
pub struct NodeInfo {
    pub path: PathBuf,
    pub name: String,
}

/// Marker component for directory nodes
/// ECS Pattern: Required component ensures NodeInfo is always present
#[derive(Component)]
#[require(NodeInfo)]
pub struct Directory {
    pub children: HashMap<String, EntityHandle>,
}

/// Marker component for data file nodes
/// ECS Pattern: Required component ensures NodeInfo is always present
#[derive(Component)]
#[require(NodeInfo)]
pub struct Datafile {
    pub value: Value,
}

/// Marker component for template nodes
/// ECS Pattern: Required component ensures NodeInfo is always present
#[derive(Component)]
#[require(NodeInfo)]
pub struct Template {
    pub template_name: String,
}

/// Component storing provided keys (for data files)
#[derive(Component)]
pub struct Provides(pub Vec<String>);

/// Component storing required keys (for templates)
#[derive(Component)]
pub struct Requires(pub Vec<String>);

/// Marker component: entity has been rendered this run
#[derive(Component)]
pub struct Rendered;

/// Tracks which requirements are still unmet for a template.
/// When `remaining` becomes empty, the template is ready to render.
#[derive(Component)]
pub struct UnmetNeeds {
    pub remaining: HashSet<String>,
}

/// Tracks which output files have already been loaded across iterations.
#[derive(Resource, Default)]
pub struct SeenOutputFiles(pub HashSet<PathBuf>);

// =============================================================================
// Lazy Serialization (EcsContext — walks ECS tree on demand)
// =============================================================================

/// Lazy serialization context that walks the ECS tree on demand.
/// Wraps a BoundEntity — no data copying, no intermediate tree.
pub struct EcsContext<'w> {
    pub entity: BoundEntity<'w>,
}

impl<'w> serde::Serialize for EcsContext<'w> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Datafile: delegate to Value's Serialize impl
        if let Some(datafile) = self.entity.get::<Datafile>() {
            return datafile.value.serialize(serializer);
        }

        // Directory: serialize children as a map, filtering _-prefixed keys
        if let Some(dir) = self.entity.get::<Directory>() {
            use serde::ser::SerializeMap;
            let filtered: Vec<_> = dir
                .children
                .iter()
                .filter(|(name, _)| !name.starts_with('_'))
                .collect();
            let mut map = serializer.serialize_map(Some(filtered.len()))?;
            for (name, handle) in filtered {
                let child_ctx = EcsContext {
                    entity: BoundEntity::new(handle.entity(), self.entity.world()),
                };
                map.serialize_entry(name, &child_ctx)?;
            }
            map.end()
        } else {
            // Templates and other entities don't contribute data
            serializer.serialize_none()
        }
    }
}

// =============================================================================
// Pure Functions (DOP/FP Pattern: Behavior Separate from Data)
// =============================================================================

/// Determines if a path is a supported template file using config
/// Pure function - no side effects
pub fn is_template(file: &Path, config: &VVConfig) -> bool {
    file.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext_str| crate::config::is_template_extension(config, ext_str))
        .unwrap_or(false)
}

/// Determines if a path is a supported data file using config
/// Pure function - no side effects
pub fn is_data_file(file: &Path, config: &VVConfig) -> bool {
    file.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext_str| crate::config::is_data_extension(config, ext_str))
        .unwrap_or(false)
}

/// Extracts a data-safe name from a path (dots replaced with underscores)
/// Pure function - deterministic transformation
pub fn data_name(path: &Path) -> String {
    let raw = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    };

    raw.and_then(|os_str| os_str.to_str())
        .map(|s| s.replace('.', "_"))
        .unwrap_or_else(|| "?".into())
}

/// Extracts template name from a path
/// Pure function - deterministic transformation
pub fn template_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|os_str| os_str.to_str())
        .map(String::from)
        .unwrap_or_else(|| "?".into())
}

/// Recursively loads a directory into the ECS world
/// Side-effect function (reads via FileSource, modifies Commands)
pub fn load_directory(
    commands: &mut Commands,
    env: &mut TemplateEnvironment,
    source: &FileSource,
    dir: PathBuf,
    prefix: &str,
    config: &VVConfig,
) -> Result<Entity, VVError> {
    let entries = source::read_dir(source, &dir)?;
    let mut children = HashMap::<String, EntityHandle>::default();

    for entry in entries {
        let path = entry.path;
        let name = data_name(&path);

        // Skip annotation sidecar files (loaded alongside their data files)
        if crate::value::is_annotation_sidecar(&path) {
            continue;
        }

        // Skip the _output directory — not source data
        if entry.is_dir && name == config.output_subdir {
            continue;
        }

        let entity_opt = if entry.is_dir {
            let new_prefix = format!("{}.{}", prefix, &name);
            Some(load_directory(
                commands,
                env,
                source,
                path,
                &new_prefix,
                config,
            )?)
        } else if source::is_file(source, &path) {
            load_file_entity(commands, env, source, config, path, &name)?
        } else {
            None
        };

        if let Some(entity) = entity_opt {
            children.insert(name, EntityHandle::new(entity));
        }
    }

    // Spawn directory entity with components
    let name = data_name(&dir);
    let entity = commands
        .spawn((NodeInfo { path: dir, name }, Directory { children }))
        .id();

    Ok(entity)
}

/// Loads a file into the ECS world (data file or template)
/// Returns None for unsupported file types
fn load_file_entity(
    commands: &mut Commands,
    env: &mut TemplateEnvironment,
    source: &FileSource,
    config: &VVConfig,
    path: PathBuf,
    name: &str,
) -> Result<Option<Entity>, VVError> {
    if is_data_file(&path, config) {
        let mut value = crate::value::load_from_source(source, &path)?;

        // For markdown files, check for markdown annotation sidecar
        // and wrap in a Map with _markdown_annotations if found
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let sidecar = {
                let mut p = path.as_os_str().to_owned();
                p.push(".annotations.ron");
                PathBuf::from(p)
            };
            if source::is_file(source, &sidecar) {
                if let Ok(bytes) = source::read_file(source, &sidecar) {
                    if let Some(md_anns) = crate::value::parse_markdown_annotations(&bytes) {
                        // Wrap Markdown in a Map to attach _markdown_annotations
                        let mut wrapper = HashMap::new();
                        wrapper.insert("content".to_string(), value);
                        wrapper.insert("_markdown_annotations".to_string(), md_anns);
                        value = Value::Map(wrapper);
                    }
                }
            }
        }

        let prov = crate::value::provides(&value, name);

        let mut entity_commands = commands.spawn((
            NodeInfo {
                path,
                name: name.to_string(),
            },
            Datafile { value },
        ));

        if let Some(keys) = prov {
            entity_commands.insert(Provides(keys));
        }

        Ok(Some(entity_commands.id()))
    } else if is_template(&path, config) {
        let tmpl_name = template_name(&path);
        load_template(env, source, &path, &tmpl_name)?;
        let requires = template_requires(env, &tmpl_name)?;

        let entity = commands
            .spawn((
                NodeInfo {
                    path,
                    name: tmpl_name.clone(),
                },
                Template {
                    template_name: tmpl_name,
                },
                Requires(requires),
            ))
            .id();

        Ok(Some(entity))
    } else {
        Ok(None)
    }
}

// =============================================================================
// Output Data Loading (Direct World Access — runs outside the schedule)
// =============================================================================

/// Loads a rendered output file into the ECS world as a Datafile entity.
///
/// Returns `true` if the file was a data file and was successfully loaded.
/// Returns `false` if the file is not a data file (e.g., a .md report).
/// Errors are logged to stderr and treated as non-fatal (returns `false`).
///
/// Annotation sidecars are intentionally not loaded for output data files —
/// they are generated artifacts, not user-authored data with annotations.
pub fn load_output_data_file(world: &mut World, path: &Path, parent: Entity) -> bool {
    let is_data = {
        let config = world.resource::<VVConfig>();
        is_data_file(path, config)
    };
    if !is_data {
        return false;
    }

    let value = match crate::value::load_file(path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Warning: failed to load output data file {:?}: {}", path, e);
            return false;
        }
    };

    let name = data_name(path);
    let prov = crate::value::provides(&value, &name);

    let mut entity = world.spawn((
        NodeInfo {
            path: path.to_path_buf(),
            name: name.clone(),
        },
        Datafile { value },
    ));

    if let Some(keys) = prov {
        entity.insert(Provides(keys));
    }

    let child_id = entity.id();

    // Insert into parent directory's children (overwrites existing on collision)
    if let Some(mut dir) = world.get_mut::<Directory>(parent) {
        dir.children.insert(name, EntityHandle::new(child_id));
    }

    true
}

/// Finds or creates intermediate `Directory` entities to mirror a file's
/// subdirectory path relative to `output_dir`.
///
/// For `output_dir=/project/_output` and `file_path=/project/_output/sub/deep/result.json`,
/// ensures `Directory` entities for "sub" and "deep" exist under `root`,
/// returning the entity for "deep".
fn find_or_create_parent_dir(
    world: &mut World,
    root: Entity,
    output_dir: &Path,
    file_path: &Path,
) -> Entity {
    let relative = match file_path.strip_prefix(output_dir) {
        Ok(r) => r,
        Err(_) => return root,
    };

    let parent_path = match relative.parent() {
        Some(p) if p.as_os_str().is_empty() => return root,
        Some(p) => p,
        None => return root,
    };

    let mut current = root;
    let mut accumulated = output_dir.to_path_buf();

    for component in parent_path.components() {
        let name = component.as_os_str().to_string_lossy().to_string();
        accumulated.push(&name);

        // Look up existing child directory
        let existing_child = world
            .get::<Directory>(current)
            .and_then(|dir| dir.children.get(&name).map(|h| h.entity()));

        if let Some(child_entity) = existing_child {
            // Child exists — check if it's a Directory and descend
            if world.get::<Directory>(child_entity).is_some() {
                current = child_entity;
                continue;
            }
        }

        // Create new Directory entity
        let new_entity = world
            .spawn((
                NodeInfo {
                    path: accumulated.clone(),
                    name: name.clone(),
                },
                Directory {
                    children: HashMap::new(),
                },
            ))
            .id();

        // Insert into parent's children
        if let Some(mut dir) = world.get_mut::<Directory>(current) {
            dir.children.insert(name, EntityHandle::new(new_entity));
        }

        current = new_entity;
    }

    current
}

/// Collects all files under `dir` recursively.
///
/// Uses `std::fs::read_dir` directly because `_output/` is always on the
/// real filesystem (never inside a .vv zip archive).
fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, out);
        } else {
            out.push(path);
        }
    }
}

// =============================================================================
// Query Functions (FP Pattern: Pure Functions over ECS Data)
// =============================================================================

/// Collects all provided keys from the entity tree using direct World access.
/// For use in exclusive systems where Query parameters aren't available.
pub fn collect_all_provides_world(root: Entity, world: &World) -> Vec<String> {
    let mut result = Vec::new();
    collect_provides_world_recursive(root, world, "", &mut result);
    result
}

fn collect_provides_world_recursive(
    entity: Entity,
    world: &World,
    prefix: &str,
    out: &mut Vec<String>,
) {
    if let Some(p) = world.get::<Provides>(entity) {
        if prefix.is_empty() {
            out.extend(p.0.iter().cloned());
        } else {
            out.extend(p.0.iter().map(|key| format!("{}.{}", prefix, key)));
        }
    }

    if let Some(dir) = world.get::<Directory>(entity) {
        for (child_name, child) in dir.children.iter() {
            let child_entity = child.entity();
            let is_child_dir = world.get::<Directory>(child_entity).is_some();

            let child_prefix = if is_child_dir {
                if prefix.is_empty() {
                    child_name.clone()
                } else {
                    format!("{}.{}", prefix, child_name)
                }
            } else {
                prefix.to_string()
            };
            collect_provides_world_recursive(child_entity, world, &child_prefix, out);
        }
    }
}

/// Collects all provided keys from the entity tree
/// Pure function over immutable query results
pub fn collect_all_provides(
    root: Entity,
    directories: &Query<&Directory>,
    provides: &Query<&Provides>,
) -> Vec<String> {
    let mut result = Vec::new();
    collect_provides_recursive(root, directories, provides, "", &mut result);
    result
}

fn collect_provides_recursive(
    entity: Entity,
    directories: &Query<&Directory>,
    provides: &Query<&Provides>,
    prefix: &str,
    out: &mut Vec<String>,
) {
    // Collect provides from this entity if it has them
    // The Provides component already includes the file name as prefix (e.g., "radiators.mass_kg")
    // We add the directory path prefix (e.g., "thermal") to get the full path (e.g., "thermal.radiators.mass_kg")
    if let Ok(p) = provides.get(entity) {
        if prefix.is_empty() {
            out.extend(p.0.iter().cloned());
        } else {
            out.extend(p.0.iter().map(|key| format!("{}.{}", prefix, key)));
        }
    }

    // Recurse into children if this is a directory
    // The directory prefix accumulates as we descend, but NOT the child name for data files
    // since their Provides already includes the file name
    if let Ok(dir) = directories.get(entity) {
        for (child_name, child) in dir.children.iter() {
            let child_entity = child.entity();
            // Check if child is a directory (has Directory component) or a data file
            let is_child_dir = directories.get(child_entity).is_ok();

            let child_prefix = if is_child_dir {
                // For directories, add the directory name to the prefix
                if prefix.is_empty() {
                    child_name.clone()
                } else {
                    format!("{}.{}", prefix, child_name)
                }
            } else {
                // For data files, keep the current prefix (don't add file name again)
                prefix.to_string()
            };
            collect_provides_recursive(child_entity, directories, provides, &child_prefix, out);
        }
    }
}

/// Collects all required keys from the entity tree
/// Pure function over immutable query results
pub fn collect_all_requires(
    root: Entity,
    directories: &Query<&Directory>,
    requires: &Query<&Requires>,
) -> Vec<String> {
    let mut result = Vec::new();
    collect_requires_recursive(root, directories, requires, &mut result);
    result
}

fn collect_requires_recursive(
    entity: Entity,
    directories: &Query<&Directory>,
    requires: &Query<&Requires>,
    out: &mut Vec<String>,
) {
    // Collect requires from this entity if it has them
    if let Ok(r) = requires.get(entity) {
        out.extend(r.0.iter().cloned());
    }

    // Recurse into children if this is a directory
    if let Ok(dir) = directories.get(entity) {
        for child in dir.children.values() {
            collect_requires_recursive(child.entity(), directories, requires, out);
        }
    }
}


// =============================================================================
// Reactive Rendering Support
// =============================================================================

/// Initializes `UnmetNeeds` on all unrendered templates.
///
/// For each template with `Requires`, computes which requirements are not yet
/// satisfied by any `Provides` component in the tree. Inserts `UnmetNeeds`
/// with the remaining set. Templates with all requirements met get an empty set.
pub fn initialize_unmet_needs(world: &mut World) {
    let root = match world.get_resource::<DataRoot>() {
        Some(dr) => dr.0,
        None => return,
    };

    let all_provides = collect_all_provides_world(root, world);

    let mut query = world.query_filtered::<(Entity, &Requires), Without<Rendered>>();
    let to_init: Vec<(Entity, HashSet<String>)> = query
        .iter(world)
        .map(|(entity, req)| {
            let remaining: HashSet<String> = req
                .0
                .iter()
                .filter(|r| !is_requirement_satisfied(r, &all_provides))
                .cloned()
                .collect();
            (entity, remaining)
        })
        .collect();

    for (entity, remaining) in to_init {
        let needs = UnmetNeeds { remaining };
        world.entity_mut(entity).insert(needs);
    }
}

/// Updates `UnmetNeeds` on all unrendered templates when new provides become available.
///
/// For each new provided key, removes matching requirements from every template's
/// `UnmetNeeds` set using the same prefix-match logic as `is_requirement_satisfied`.
pub fn update_unmet_needs(world: &mut World, new_provides: &[String]) {
    let mut query = world.query_filtered::<(Entity, &mut UnmetNeeds), Without<Rendered>>();

    // Collect entities to update (can't mutate during iteration in some cases)
    let updates: Vec<(Entity, HashSet<String>)> = query
        .iter(world)
        .map(|(entity, needs)| {
            let remaining: HashSet<String> = needs
                .remaining
                .iter()
                .filter(|req| !is_requirement_satisfied(req, new_provides))
                .cloned()
                .collect();
            (entity, remaining)
        })
        .collect();

    for (entity, remaining) in updates {
        if let Some(mut needs) = world.get_mut::<UnmetNeeds>(entity) {
            needs.remaining = remaining;
        }
    }
}

/// Scans `_output/` for new data files produced by template rendering,
/// loads them into the ECS tree, and returns the newly-provided keys.
///
/// Uses `SeenOutputFiles` to avoid reloading files across iterations.
pub fn load_output_data_and_provides(
    world: &mut World,
    output_dir: &Path,
    root: Entity,
) -> Vec<String> {
    // Collect all files first
    let mut files = Vec::new();
    collect_files_recursive(output_dir, &mut files);

    let mut new_provides = Vec::new();
    for path in files {
        {
            let seen = world.resource::<SeenOutputFiles>();
            if seen.0.contains(&path) {
                continue;
            }
        }

        let parent = find_or_create_parent_dir(world, root, output_dir, &path);
        if load_output_data_file(world, &path, parent) {
            // Collect the newly-provided keys from the just-loaded file
            let name = data_name(&path);
            // Build prefix by walking from output_dir to parent
            let prefix = build_output_prefix(output_dir, &path);
            if let Some(last_child) = world.get::<Directory>(parent)
                .and_then(|dir| dir.children.get(&name))
            {
                let child_entity = last_child.entity();
                if let Some(prov) = world.get::<Provides>(child_entity) {
                    if prefix.is_empty() {
                        new_provides.extend(prov.0.iter().cloned());
                    } else {
                        new_provides.extend(prov.0.iter().map(|k| format!("{}.{}", prefix, k)));
                    }
                }
            }

            world.resource_mut::<SeenOutputFiles>().0.insert(path);
        }
    }

    new_provides
}

/// Builds the dotted prefix for an output file relative to output_dir.
///
/// For `output_dir=/project/_output` and `path=/project/_output/sub/deep/result.json`,
/// returns `"sub.deep"`. For files directly in output_dir, returns `""`.
fn build_output_prefix(output_dir: &Path, file_path: &Path) -> String {
    let relative = match file_path.strip_prefix(output_dir) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    let parent = match relative.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => return String::new(),
    };
    parent
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(".")
}

// =============================================================================
// Dependency Graph + Cycle Detection
// =============================================================================

/// Dependency graph between templates for cycle detection.
/// Edges represent "template A depends on template B" relationships.
pub struct DependencyGraph {
    /// Template entity → list of template entities it depends on
    pub edges: HashMap<Entity, Vec<Entity>>,
    /// Template entity → template name (for error messages)
    pub names: HashMap<Entity, String>,
}

/// Builds a dependency graph between templates.
///
/// For each template, determines which other templates it depends on by:
/// 1. Filtering out requirements already satisfied by source data
/// 2. Matching remaining requirements against other templates' implicit provides
///
/// A template's implicit provides prefix is derived from its output filename
/// (e.g., `step_a.json.j2` → prefix `step_a`).
pub fn build_dependency_graph(world: &mut World, root: Entity) -> DependencyGraph {
    let source_provides = collect_all_provides_world(root, world);

    let mut query = world.query::<(Entity, &Template, &Requires)>();

    // Collect template info in one pass
    let templates: Vec<(Entity, String, Vec<String>)> = query
        .iter(world)
        .map(|(entity, template, requires)| {
            (entity, template.template_name.clone(), requires.0.clone())
        })
        .collect();

    // Compute provides prefix for each template
    let template_provides: Vec<(Entity, String)> = templates
        .iter()
        .map(|(entity, name, _)| (*entity, template_output_prefix(name)))
        .collect();

    // Build edges: A depends on B if B could satisfy one of A's unsatisfied requirements
    let mut edges: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut names: HashMap<Entity, String> = HashMap::new();

    for (entity, name, requires) in &templates {
        names.insert(*entity, name.clone());

        let mut deps = Vec::new();
        for req in requires {
            // Skip requirements already satisfied by source data
            if is_requirement_satisfied(req, &source_provides) {
                continue;
            }
            // Find which template(s) could satisfy this requirement
            for (provider_entity, provider_prefix) in &template_provides {
                if *provider_entity != *entity
                    && could_satisfy(provider_prefix, req)
                    && !deps.contains(provider_entity)
                {
                    deps.push(*provider_entity);
                }
            }
        }
        edges.insert(*entity, deps);
    }

    DependencyGraph { edges, names }
}

/// Detects cycles in the dependency graph.
///
/// Returns a list of cycles, where each cycle is a list of template names
/// forming the cycle (e.g., `["a.json", "b.json"]` means a → b → a).
pub fn detect_cycles(graph: &DependencyGraph) -> Vec<Vec<String>> {
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();
    let mut path = Vec::new();
    let mut cycles = Vec::new();

    for &node in graph.edges.keys() {
        if !visited.contains(&node) {
            dfs_detect_cycle(graph, node, &mut visited, &mut in_stack, &mut path, &mut cycles);
        }
    }

    cycles
}

fn dfs_detect_cycle(
    graph: &DependencyGraph,
    node: Entity,
    visited: &mut HashSet<Entity>,
    in_stack: &mut HashSet<Entity>,
    path: &mut Vec<Entity>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node);
    in_stack.insert(node);
    path.push(node);

    if let Some(deps) = graph.edges.get(&node) {
        for &dep in deps {
            if !visited.contains(&dep) {
                dfs_detect_cycle(graph, dep, visited, in_stack, path, cycles);
            } else if in_stack.contains(&dep) {
                // Found a cycle — extract from path starting at dep
                let cycle_start = path.iter().position(|&e| e == dep).unwrap();
                let cycle: Vec<String> = path[cycle_start..]
                    .iter()
                    .map(|e| {
                        graph
                            .names
                            .get(e)
                            .cloned()
                            .unwrap_or_else(|| format!("{:?}", e))
                    })
                    .collect();
                cycles.push(cycle);
            }
        }
    }

    path.pop();
    in_stack.remove(&node);
}

/// Computes the data key prefix that a template's output would provide.
///
/// `step_a.json` → `"step_a"` (output loads as `step_a.*`)
/// `sub::result.json` → `"sub.result"` (output at `_output/sub/result.json`)
fn template_output_prefix(template_name: &str) -> String {
    let output_name = template_name.replace("::", "/");
    let path = Path::new(&output_name);

    let file_key = data_name(path);
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => {
            let dir_prefix = p
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(".");
            format!("{}.{}", dir_prefix, file_key)
        }
        _ => file_key,
    }
}

/// Checks if a template's provides prefix could satisfy a requirement.
///
/// A template providing prefix `P` could satisfy requirement `R` if:
/// - `R == P` (exact match)
/// - `R` starts with `P.` (requirement is a sub-key of the template's output)
/// - `P` starts with `R.` (template output is under the requirement's namespace)
fn could_satisfy(provides_prefix: &str, requirement: &str) -> bool {
    provides_prefix == requirement
        || requirement.starts_with(&format!("{}.", provides_prefix))
        || provides_prefix.starts_with(&format!("{}.", requirement))
}

// =============================================================================
// Validation (FP Pattern: Pure Function with Clear Contract)
// =============================================================================

/// Validates that all required keys are provided by data files.
///
/// The matching is hierarchical: a required key like "propulsion" is satisfied
/// if there's a provided key starting with "propulsion." (e.g., "propulsion.thrust").
///
/// Pure function - no side effects, deterministic output
pub fn validate_dependencies(provides: &[String], requires: &[String]) -> Result<(), VVError> {
    let unmet: Vec<String> = requires
        .iter()
        .filter(|req| !is_requirement_satisfied(req, provides))
        .cloned()
        .collect();

    if unmet.is_empty() {
        Ok(())
    } else {
        Err(VVError::UnmetDependencies(unmet))
    }
}

/// Checks if a single requirement is satisfied by the provided keys
/// Pure predicate function
pub fn is_requirement_satisfied(requirement: &str, provides: &[String]) -> bool {
    let prefix = format!("{}.", requirement);
    provides
        .iter()
        .any(|p| p.starts_with(&prefix) || p == requirement)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> VVConfig {
        VVConfig::default()
    }

    #[test]
    fn test_is_template_detects_extensions() {
        let config = default_config();

        // Should detect template extensions
        assert!(is_template(Path::new("test.j2"), &config));
        assert!(is_template(Path::new("test.jinja"), &config));
        assert!(is_template(Path::new("test.jinja2"), &config));
        assert!(is_template(Path::new("test.tmpl"), &config));

        // Should reject non-template extensions
        assert!(!is_template(Path::new("test.json"), &config));
        assert!(!is_template(Path::new("test.yaml"), &config));
        assert!(!is_template(Path::new("test.txt"), &config));
        assert!(!is_template(Path::new("noextension"), &config));
    }

    #[test]
    fn test_is_data_file_detects_extensions() {
        let config = default_config();

        // Should detect text data extensions
        assert!(is_data_file(Path::new("test.json"), &config));
        assert!(is_data_file(Path::new("test.yaml"), &config));
        assert!(is_data_file(Path::new("test.yml"), &config));
        assert!(is_data_file(Path::new("test.toml"), &config));
        assert!(is_data_file(Path::new("test.ron"), &config));

        // Should detect binary data extensions
        assert!(is_data_file(Path::new("test.msgpack"), &config));
        assert!(is_data_file(Path::new("test.mp"), &config));
        assert!(is_data_file(Path::new("test.pickle"), &config));
        assert!(is_data_file(Path::new("test.pkl"), &config));
        assert!(is_data_file(Path::new("test.cbor"), &config));

        // Should reject non-data extensions
        assert!(!is_data_file(Path::new("test.j2"), &config));
        assert!(!is_data_file(Path::new("test.txt"), &config));
        assert!(!is_data_file(Path::new("noextension"), &config));
    }

    #[test]
    fn test_data_name_sanitizes_dots() {
        // Dots should be replaced with underscores
        assert_eq!(data_name(Path::new("file.test.json")), "file_test");
        assert_eq!(data_name(Path::new("my.data.yaml")), "my_data");
        assert_eq!(data_name(Path::new("simple.json")), "simple");
    }

    #[test]
    fn test_template_name() {
        // Should extract the stem without extension
        assert_eq!(template_name(Path::new("report.md.j2")), "report.md");
        assert_eq!(template_name(Path::new("output.txt.jinja")), "output.txt");
    }

    #[test]
    fn test_is_requirement_satisfied() {
        let provides = vec![
            "propulsion.thrust".to_string(),
            "propulsion.isp".to_string(),
            "config".to_string(),
        ];

        // Prefix matching
        assert!(is_requirement_satisfied("propulsion", &provides));

        // Exact matching
        assert!(is_requirement_satisfied("config", &provides));

        // Not satisfied
        assert!(!is_requirement_satisfied("power", &provides));
    }

    #[test]
    fn test_is_requirement_satisfied_nested_directories() {
        // Simulate provides from nested directory structure:
        // thermal/radiators.yaml -> thermal.radiators.mass_kg
        let provides = vec![
            "thermal.radiators.mass_kg".to_string(),
            "thermal.radiators.area_m2".to_string(),
            "thermal.heat_pipes.capacity".to_string(),
            "propulsion.main_engine.thrust".to_string(),
        ];

        // Directory prefix matching
        assert!(is_requirement_satisfied("thermal", &provides));
        assert!(is_requirement_satisfied("thermal.radiators", &provides));
        assert!(is_requirement_satisfied("thermal.heat_pipes", &provides));
        assert!(is_requirement_satisfied("propulsion", &provides));
        assert!(is_requirement_satisfied(
            "propulsion.main_engine",
            &provides
        ));

        // Not satisfied
        assert!(!is_requirement_satisfied("power", &provides));
        assert!(!is_requirement_satisfied("thermal.sensors", &provides));
    }

    #[test]
    fn test_template_output_prefix() {
        // Simple template: step_a.json → "step_a"
        assert_eq!(template_output_prefix("step_a.json"), "step_a");
        // Report template: report.md → "report"
        assert_eq!(template_output_prefix("report.md"), "report");
        // Namespace separator: sub::result.json → "sub.result"
        assert_eq!(template_output_prefix("sub::result.json"), "sub.result");
        // Deep namespace: a::b::c.json → "a.b.c"
        assert_eq!(template_output_prefix("a::b::c.json"), "a.b.c");
    }

    #[test]
    fn test_could_satisfy() {
        // Exact match
        assert!(could_satisfy("step_a", "step_a"));
        // Requirement is a sub-key of the provides prefix
        assert!(could_satisfy("step_a", "step_a.computed"));
        // Provides prefix is under the requirement's namespace
        assert!(could_satisfy("sub.deep.result", "sub"));
        // No match
        assert!(!could_satisfy("step_a", "step_b"));
        assert!(!could_satisfy("step_a", "other"));
    }

    #[test]
    fn test_detect_cycles_simple() {
        // A requires B, B requires A → cycle
        let mut world = World::new();

        let a = world.spawn((
            Template { template_name: "a.json".into() },
            Requires(vec!["b".into()]),
        )).id();
        let b = world.spawn((
            Template { template_name: "b.json".into() },
            Requires(vec!["a".into()]),
        )).id();

        let mut edges = HashMap::new();
        edges.insert(a, vec![b]);
        edges.insert(b, vec![a]);

        let mut names = HashMap::new();
        names.insert(a, "a.json".into());
        names.insert(b, "b.json".into());

        let graph = DependencyGraph { edges, names };
        let cycles = detect_cycles(&graph);

        assert!(!cycles.is_empty(), "Should detect a cycle");
        // The cycle should contain both templates
        let cycle = &cycles[0];
        assert!(cycle.contains(&"a.json".to_string()));
        assert!(cycle.contains(&"b.json".to_string()));
    }

    #[test]
    fn test_detect_cycles_linear_no_cycle() {
        // A → B → C (linear chain, no cycle)
        let mut world = World::new();

        let a = world.spawn((
            Template { template_name: "a.json".into() },
            Requires(vec![]),
        )).id();
        let b = world.spawn((
            Template { template_name: "b.json".into() },
            Requires(vec!["a".into()]),
        )).id();
        let c = world.spawn((
            Template { template_name: "c.json".into() },
            Requires(vec!["b".into()]),
        )).id();

        let mut edges = HashMap::new();
        edges.insert(a, vec![]);
        edges.insert(b, vec![a]);
        edges.insert(c, vec![b]);

        let mut names = HashMap::new();
        names.insert(a, "a.json".into());
        names.insert(b, "b.json".into());
        names.insert(c, "c.json".into());

        let graph = DependencyGraph { edges, names };
        let cycles = detect_cycles(&graph);

        assert!(cycles.is_empty(), "Linear chain should have no cycles");
    }
}
