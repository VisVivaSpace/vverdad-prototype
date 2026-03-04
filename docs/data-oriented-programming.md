# Data-Oriented Programming in VVERDAD

This document explains the data-oriented programming (DOP) paradigm and its application in the VVERDAD aerospace vehicle design data processing engine.

## What is Data-Oriented Programming?

Data-Oriented Programming is a programming paradigm that treats data as a first-class citizen, separate from the code that manipulates it. Unlike object-oriented programming (OOP), which encapsulates data and behavior together, DOP emphasizes:

1. Clear separation between data and the functions that operate on it
2. Use of generic, composable data structures
3. Immutable data transformations
4. Decoupling of data schema from data representation

### The Four Principles

#### Principle 1: Separate Code from Data

Code (behavior) lives in standalone functions. Data lives in plain structures with no methods.

**Anti-pattern (OOP):**
```rust
struct Node {
    path: PathBuf,
    children: HashMap<String, Entity>,
}

impl Node {
    fn load(&mut self) -> Result<()> { /* ... */ }
    fn render(&self) -> String { /* ... */ }
}
```

**DOP pattern:**
```rust
// Pure data component
struct NodeInfo {
    path: PathBuf,
    name: String,
}

// Behavior in standalone functions
fn load_directory(path: &Path) -> Result<HashMap<String, Entity>> { /* ... */ }
fn build_data_tree(root: Entity, queries: &Queries) -> Option<DataTree> { /* ... */ }
```

#### Principle 2: Represent Data with Generic Structures

Use maps, vectors, and enums instead of specialized classes. This enables format-agnostic processing.

**Key insight:** All aerospace design data, regardless of source format (JSON, YAML, TOML, RON, CSV, XLSX), can be represented with the same small set of data structures: scalars, maps, sequences, and domain-specific types (quantities, time epochs, tables).

#### Principle 3: Data is Immutable

Transformations create new data rather than mutating existing data. This ensures traceability and prevents unintended side effects.

**Benefits:**
- Data provenance tracking (critical for aerospace)
- Safe concurrent access
- Easier debugging and testing
- Clear data flow

#### Principle 4: Separate Data Schema from Data Representation

Schema validation is separate from data representation. Multiple formats can share the same validation logic.

**Example:** VVERDAD uses Serde's format-agnostic deserialization. A propulsion configuration can be stored in JSON, TOML, or YAML without changing the validation or processing logic.

## DOP in VVERDAD

VVERDAD is built on Bevy ECS, which provides a natural implementation of data-oriented principles.

### Principle 1: Code-Data Separation

**Pure data components** (`src/node.rs`):

```rust
#[derive(Component)]
struct NodeInfo {
    path: PathBuf,
    name: String,
}

#[derive(Component)]
#[require(NodeInfo)]
struct Directory {
    children: HashMap<String, Entity>,
}

#[derive(Component)]
#[require(NodeInfo)]
struct Datafile {
    contents: Value,
}
```

These structs have no `impl` blocks with business logic. They are pure data containers.

**Standalone functions** (`src/node.rs`):

```rust
fn load_directory(
    source: &dyn FileSource,
    path: &Path,
    commands: &mut Commands,
) -> Result<HashMap<String, Entity>, VVError>

fn build_data_tree(
    root: Entity,
    directories: &Query<&Directory>,
    datafiles: &Query<&Datafile>,
    merged_outputs: &Query<&MergedOutputChildren>,
) -> Option<DataTree>
```

Logic for loading, transforming, and querying data lives in functions that receive data as parameters.

**ECS systems as pure functions** (`src/lib.rs`):

```rust
fn load_data_system(
    mut commands: Commands,
    config: Res<VVConfig>,
    source: Res<FileSourceResource>,
) -> Result<(), VVError>

fn render_templates_system(
    data_root: Res<DataRoot>,
    template_env: Res<TemplateEnvironment>,
    directories: Query<&Directory>,
    datafiles: Query<&Datafile>,
) -> Result<(), VVError>
```

Systems are functions that operate on data stored in the ECS world. They have no hidden state.

### Principle 2: Generic Data Structures

**The `Value` enum** (`src/value.rs`):

All input formats normalize to a single 14-variant enum:

```rust
pub enum Value {
    // Scalars
    Float(f64),
    Integer(i64),
    Bool(bool),
    String(String),

    // Physical quantities
    Quantity { value: f64, unit: Unit },

    // Time epochs (days after J2000.0)
    Utc(f64),
    Tdb(f64),

    // Structured data
    Table { headers: Vec<String>, rows: Vec<Vec<Value>> },
    Markdown { content: String, front_matter: Option<Box<Value>> },

    // Metadata (non-serializing)
    Source { path: PathBuf },
    Annotation(AnnotationData),
    MarkdownAnnotation(MarkdownAnnotationData),

    // Compound types
    Map(HashMap<String, Value>),
    Seq(Vec<Value>),
}
```

**Format-agnostic processing:**

- JSON, TOML, YAML, RON all parse to `HashMap<String, Value>` (Map variant)
- CSV, XLSX parse to `Table { headers, rows }`
- Markdown files parse to `Markdown { content, front_matter }`

Template rendering operates on `Value` without knowing the source format.

**Composition over inheritance:**

Instead of a class hierarchy (`DataFile` → `JSONFile` → `PropulsionJSON`), VVERDAD uses:
- Generic `Datafile` component (holds any `Value`)
- Generic `Value::Map` (holds any key-value data)
- Generic `DataTree` serialization (works with any `Value`)

### Principle 3: Immutability

**Immutable resources:**

Systems receive data as `Res<>` (immutable reference) or `Res<Mut<>>` (explicit mutability):

```rust
fn render_templates_system(
    data_root: Res<DataRoot>,  // Immutable
    template_env: Res<TemplateEnvironment>,  // Immutable
    directories: Query<&Directory>,  // Immutable query
    datafiles: Query<&Datafile>,  // Immutable query
) -> Result<(), VVError>
```

**Transformations return new data:**

```rust
// build_data_tree doesn't mutate the ECS world
// It reads queries and returns a new DataTree
fn build_data_tree(
    root: Entity,
    directories: &Query<&Directory>,
    datafiles: &Query<&Datafile>,
    merged_outputs: &Query<&MergedOutputChildren>,
) -> Option<DataTree>
```

**Source data immutability:**

The `_output/` merging system protects source data from being overwritten:

```rust
// From src/node.rs
// When _output/ contains a key that exists in source data:
if current_children.contains_key(name) {
    warn!("Output key '{name}' collides with source data at {path}, skipping");
    continue;  // Source data takes precedence
}
```

This ensures that user input data is never modified by analysis outputs.

**Eager parsing as immutable transformation:**

When data is loaded, strings are eagerly parsed into structured types (quantities, time epochs). This happens once at load time, creating an immutable transformation:

```
"2025-01-15T00:00:00 UTC" → Value::Utc(9496.0)
"100 N" → Value::Quantity { value: 100.0, unit: Unit::Newton }
```

### Principle 4: Schema-Representation Separation

**Format-agnostic deserialization pipeline:**

All text formats follow the same path:

```
File bytes
  ↓ (format-specific parser)
serde_value::Value
  ↓ (from_serde_value)
Value enum
  ↓ (NodeContents::Datafile)
ECS component
```

The intermediate `serde_value::Value` provides format independence. The same `from_serde_value()` function handles JSON, TOML, YAML, and RON.

**Schema validation is separate:**

VVERDAD doesn't enforce a schema at load time. Instead:
- Data is loaded permissively into `Value` structures
- Templates reference expected keys (`{{ propulsion.thrust }}`)
- Missing keys cause template rendering errors (not load errors)
- Analysis manifests declare expected data via `depends_on` lists

This separation allows:
- Loading partial or evolving datasets
- Flexible data schemas across different aerospace vehicle types
- Schema evolution without code changes

**CSV/XLSX as first-class formats:**

CSV and XLSX bypass `serde_value` and construct `Value::Table` directly. This demonstrates format flexibility: the schema (headers, rows) is inferred from the file structure, not declared in code.

## Why DOP for Aerospace Engineering?

VVERDAD processes aerospace vehicle design data with unique requirements:

### 1. Heterogeneous Data Formats

Vehicle design data comes from diverse sources:
- Propulsion data in YAML (from supplier databases)
- Mission parameters in JSON (from trajectory optimization tools)
- Power budgets in XLSX (from electrical engineers)
- Orbital elements in RON (from astrodynamics simulations)

**DOP solution:** All formats normalize to the same `Value` enum. Templates and analysis pipelines work with any format.

### 2. Data Provenance and Traceability

Safety-critical aerospace systems require tracing every data point to its source.

**DOP solution:**
- `Value::Source { path }` metadata tracks file origins
- Immutable transformations preserve provenance
- `_output/` merging separates derived data from source data

### 3. Auditability

Design reviews need to answer: "What data produced this analysis result?"

**DOP solution:**
- Data flow is explicit: load → transform → render
- No hidden state in objects
- Systems are pure functions with clear inputs/outputs

### 4. Future Parallelism

Large vehicle designs benefit from parallel processing (e.g., analyzing multiple subsystems concurrently).

**DOP solution:**
- Bevy ECS systems can run in parallel automatically
- Immutable data prevents race conditions
- Pure functions are trivially parallelizable

### 5. Physical Unit Safety

Aerospace calculations mix unit systems (metric, imperial, CGS). Unit errors cause mission failures.

**DOP solution:**
- `Value::Quantity { value, unit }` makes units explicit
- Template filters (`|to("lbf")`, `|si`) provide safe conversions
- Units are data, not behavior embedded in classes

## DOP + ECS: A Natural Fit

Bevy ECS embodies data-oriented principles:

**Entities are IDs, not objects:**

```rust
// Not: Vehicle { id, propulsion, power, structure }
// Instead:
let vehicle = commands.spawn_empty().id();  // Just an ID
commands.entity(vehicle).insert(Propulsion { thrust: 100.0 });
commands.entity(vehicle).insert(Power { capacity: 500.0 });
```

**Components are pure data:**

```rust
#[derive(Component)]
struct Propulsion {
    thrust: f64,  // No methods, just data
    isp: f64,
}
```

**Systems are functions:**

```rust
fn calculate_delta_v(
    query: Query<(&Propulsion, &Structure)>
) {
    for (prop, structure) in query.iter() {
        // Pure function: input (components) → output (delta-v)
    }
}
```

**Queries separate data selection from processing:**

```rust
// Schema (what data to fetch)
Query<(&Directory, &NodeInfo)>

// Representation (how to process it)
fn build_data_tree(dirs: &Query<(&Directory, &NodeInfo)>) -> DataTree
```

### ECS Enables DOP at Scale

For VVERDAD:
- **Entities** represent files/directories (just IDs)
- **Components** hold file contents, metadata (pure data)
- **Systems** load, transform, render (pure functions)
- **Resources** provide global context (config, output sink)
- **Queries** separate data selection from processing

The ECS architecture prevents common OOP pitfalls:
- No inheritance hierarchies
- No hidden state in objects
- No complex object graphs
- No polymorphism gymnastics

## Practical Benefits

### Testability

Pure functions with explicit inputs/outputs are trivial to test:

```rust
#[test]
fn test_build_data_tree() {
    let mut world = World::new();
    let root = world.spawn(Directory::default()).id();
    // ... populate world with test data

    let tree = build_data_tree(
        root,
        &world.query::<&Directory>(),
        &world.query::<&Datafile>(),
        &world.query::<&MergedOutputChildren>(),
    );

    assert!(tree.is_some());
}
```

No mocking, no dependency injection, no test harnesses.

### Debuggability

Data is visible in the debugger. No hidden state in private fields or closures.

```rust
// What data does this entity have?
dbg!(world.entity(id).archetype());

// What's in this component?
dbg!(world.get::<Datafile>(id));
```

### Refactorability

Logic changes don't require changing data structures. Data structure changes don't require changing all the logic.

Example: Adding `Value::Quantity` required:
- Updating the `Value` enum
- Adding parsing in `from_serde_value()`
- Adding template filters

But didn't require:
- Changing file loading logic
- Changing the ECS component structure
- Changing template rendering logic

## Comparison with OOP

| Aspect | OOP | DOP (VVERDAD) |
|--------|-----|---------------|
| Data-behavior coupling | Data and methods in classes | Pure data in components, logic in functions |
| Data representation | Custom classes per format | Generic `Value` enum |
| State management | Mutable object graphs | Immutable transformations |
| Polymorphism | Inheritance hierarchies | Enum variants + match expressions |
| Testing | Mock objects, DI frameworks | Pure functions, no mocking needed |
| Concurrency | Locks, mutexes, careful design | Immutable data + ECS parallelism |

VVERDAD could have been built with OOP:
```rust
trait DataFile {
    fn load(&mut self) -> Result<()>;
    fn render(&self, template: &str) -> String;
}

struct JSONDataFile { /* ... */ }
impl DataFile for JSONDataFile { /* ... */ }

struct YAMLDataFile { /* ... */ }
impl DataFile for YAMLDataFile { /* ... */ }
```

But this creates:
- Code duplication (each format implements same rendering logic)
- Hidden state (mutable fields in file objects)
- Tight coupling (rendering logic tied to file type)
- Testing complexity (need to mock file I/O)

The DOP approach separates concerns:
- `Value` enum (data representation)
- `from_serde_value()` (format parsing)
- `build_data_tree()` (data transformation)
- Template rendering (output generation)

Each piece is independently testable and composable.

## Guidelines for VVERDAD Development

When adding features, maintain DOP principles:

### 1. Keep Components Data-Only

**Do:**
```rust
#[derive(Component)]
struct AnalysisBundle {
    manifest: Manifest,
    static_files: Vec<PathBuf>,
}
```

**Don't:**
```rust
#[derive(Component)]
struct AnalysisBundle {
    manifest: Manifest,
}

impl AnalysisBundle {
    fn execute(&mut self) -> Result<()> { /* ... */ }
}
```

### 2. Write Pure Functions

**Do:**
```rust
fn validate_dependencies(
    manifest: &Manifest,
    available_data: &HashSet<String>,
) -> Result<(), VVError>
```

**Don't:**
```rust
impl Manifest {
    fn validate(&mut self, context: &Context) -> Result<()> {
        // Mutates self, depends on external context
    }
}
```

### 3. Use Generic Data Structures

**Do:**
```rust
// Add new variant to Value enum
enum Value {
    // ... existing variants
    Matrix { rows: usize, cols: usize, data: Vec<f64> },
}
```

**Don't:**
```rust
// Create specialized struct
struct MatrixFile {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}
```

### 4. Isolate Side Effects

**Do:**
```rust
// Pure function
fn build_docker_config(manifest: &Manifest) -> DockerConfig

// System that performs I/O
fn execute_analyses_system(
    config: Res<DockerConfig>,
    sink: Res<OutputSink>,
) -> Result<(), VVError>
```

**Don't:**
```rust
// Side effects mixed with logic
fn execute_analysis(manifest: &Manifest) -> Result<()> {
    let config = build_config(manifest);
    docker::run(config)?;  // Hidden I/O
    fs::write("output.txt", result)?;  // Hidden I/O
}
```

## References

- **CLAUDE.md**: See "Coding Skills" section for skill-specific patterns
  - `data-oriented-programming` skill
  - `ecs-patterns` skill
  - `functional-programming` skill

- **Bevy ECS Documentation**: https://docs.rs/bevy_ecs/
  - Component design patterns
  - System function signatures
  - Query composition

- **VVERDAD Architecture**: See `VVERDAD_ARCHITECTURE.md` for system-level design

- **Value Type System**: See `docs/value-type-spec.md` for `Value` enum details

- **Data-Oriented Design** (book): Richard Fabian
  - Focus: memory layout and cache efficiency
  - VVERDAD applies conceptual principles, not low-level optimization

- **Functional Programming in Rust**: Similar principles (immutability, pure functions), different focus
  - FP emphasizes mathematical foundations
  - DOP emphasizes data modeling and transformation
