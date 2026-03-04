# VVERDAD Directory Structure

This document describes the directory conventions and naming rules for VVERDAD projects.

## Overview

A VVERDAD project is a directory tree (or `.vv` zip archive) containing data files and templates. The directory hierarchy maps directly to template key namespacing. When VVERDAD loads a project:

1. Directory and file names become dot-separated keys
2. Data files are loaded and stored with their full path-based key
3. Templates can reference data using these hierarchical keys
4. Rendered templates are written to the `_output/` directory

This structure enables organizing complex aerospace vehicle designs by subsystem while maintaining clear data provenance and template access patterns.

## Name-to-Key Mapping

Directory and file names are transformed into template keys using the following rules:

### Basic Mapping

The full path from the project root to a data file determines its template key:

```
propulsion/engine.yaml          → propulsion.engine
mission/trajectory.json         → mission.trajectory
power/solar_arrays/panels.toml  → power.solar_arrays.panels
```

### File Stem Extraction

Only the file stem (filename without extension) becomes part of the key:

```
propulsion/engine.yaml     → propulsion.engine (not propulsion.engine.yaml)
mission.json               → mission (not mission.json)
```

### Key Access in Templates

Data within files is accessed by appending the internal key to the path-based prefix:

**File**: `propulsion/engine.yaml`
```yaml
thrust: "110 kN"
isp: "465 s"
fuel: "LH2/LOX"
```

**Template access**:
```jinja
{{ propulsion.engine.thrust }}  {# "110 kN" #}
{{ propulsion.engine.isp }}     {# "465 s" #}
{{ propulsion.engine.fuel }}    {# "LH2/LOX" #}
```

### Sanitization

The `data_name()` function in `src/node.rs` sanitizes file and directory names:

- **Dots → Underscores**: `mass.properties.yaml` → `mass_properties`
- **Preserves**: Letters, numbers, hyphens, underscores
- **Case sensitivity**: Preserves original case (but avoid relying on case to distinguish keys)

This ensures all path components become valid template identifiers.

## Supported File Extensions

VVERDAD recognizes files by extension and processes them according to type.

### Template Files

Templates are processed with Minijinja and rendered to `_output/`:

| Extension | Format |
|-----------|--------|
| `.j2` | Jinja2 template |
| `.jinja` | Jinja2 template |
| `.jinja2` | Jinja2 template |
| `.tmpl` | Generic template |

Templates retain their base name in output (e.g., `report.md.j2` → `_output/report.md`).

### Text Data Formats

Human-readable structured data formats:

| Extension | Format | Notes |
|-----------|--------|-------|
| `.json` | JSON | Standard JSON |
| `.toml` | TOML | Configuration-friendly |
| `.yaml` | YAML | Whitespace-sensitive |
| `.yml` | YAML | Alternate extension |
| `.ron` | Rusty Object Notation | Rust-native format |

### Binary Data Formats

Compact binary serialization formats:

| Extension | Format | Notes |
|-----------|--------|-------|
| `.msgpack` | MessagePack | Cross-language binary format |
| `.mp` | MessagePack | Alternate extension |
| `.pickle` | Python Pickle | **Security warning**: Only load from trusted sources |
| `.pkl` | Python Pickle | Alternate extension |
| `.cbor` | CBOR | Concise Binary Object Representation |
| `.bson` | BSON | Binary JSON (MongoDB format) |

**Security Note**: Python pickle files can execute arbitrary code during deserialization. Only process pickle files from trusted sources.

### Tabular Data Formats

Spreadsheet and CSV formats with special dual-access structure:

| Extension | Format | Structure |
|-----------|--------|-----------|
| `.csv` | CSV | `{ row: [...], col: {...} }` |
| `.xlsx` | Excel | `{ sheet_name: { row: [...], col: {...} } }` |

See "CSV and Excel Data" section below for access patterns.

### Markdown Files

Markdown documents with optional YAML front matter:

| Extension | Format | Structure |
|-----------|--------|-----------|
| `.md` | Markdown | `{ content: "...", front_matter: {...} }` |

Front matter is delimited by `---` at the start of the file. If present, it is parsed as YAML and accessible via the `front_matter` key.

## Special Directories

### `_output/` Directory

The output directory where all rendered templates are written.

**Behavior**:
- Created automatically if it doesn't exist
- Overwritten on each run (all existing files deleted)
- Contents are merged back into the data tree after rendering
- Source data keys take precedence over output keys (user inputs are immutable)

**Merging**:
After templates are rendered, `_output/` contents are loaded and merged at matching tree positions:

```
Project structure:
├── propulsion/engine.yaml       # Source data
├── template.md.j2               # Template
└── _output/
    ├── template.md              # Rendered output
    └── propulsion/
        └── analysis_result.csv  # Generated data

Merged tree:
propulsion:
  engine: {...}                  # Original source data
  analysis_result: {...}         # Loaded from _output/propulsion/analysis_result.csv
template: "..."                  # Loaded from _output/template.md (as Value::Markdown)
```

If a key exists in both source and output, the source version is kept and a warning is logged.

### `.analysis/` Directories

Analysis bundles are self-contained analysis packages identified by the `.analysis/` suffix.

**Structure**:
```
thermal_analysis.analysis/
├── manifest.ron           # Required: analysis configuration
├── script.py.j2           # Template files (rendered before execution)
├── config.json            # Static files (copied to _output/)
└── README.md              # Documentation (ignored)
```

**Execution Flow**:
1. Discovery: `discover_analyses_system` finds `.analysis/` directories and parses `manifest.ron`
2. Rendering: `render_analyses_system` copies static files and renders templates to `_output/`
3. Execution: `execute_analyses_system` runs Docker containers (if Docker available)
4. Validation: `validate_outputs_system` verifies expected output files exist

**Notes**:
- Analysis bundles require filesystem access for Docker mounting (not compatible with `.vv` archive input)
- `manifest.ron` must conform to the `Manifest` struct in `src/analysis/manifest.rs`
- See `resources/test/phobos_sr/propulsion/thermal_analysis.analysis/` for a working example

## Special Files

### Annotation Sidecars

Annotation files provide metadata about data files without modifying the original data.

**Data annotation sidecars** (`{stem}.annotations.ron`):
- Placed alongside data files (e.g., `engine.yaml` + `engine.annotations.ron`)
- RON format: `HashMap<String, Vec<AnnotationData>>`
- Map keys correspond to data keys within the file
- Loaded into `Value::Annotation` and stored under `_annotations` reserved key

**Example**:
```
propulsion/
├── engine.yaml
└── engine.annotations.ron
```

`engine.annotations.ron`:
```ron
{
  "thrust": [
    (
      level: Warning,
      message: "Thrust value based on preliminary design",
      source: Some("prelim-design-v2.pdf"),
    ),
  ],
}
```

**Markdown annotation sidecars** (`{name}.md.annotations.ron`):
- Placed alongside markdown files (e.g., `report.md` + `report.md.annotations.ron`)
- RON format: `Vec<MarkdownAnnotationData>`
- Each annotation specifies line/column ranges and metadata
- Loaded into `Value::MarkdownAnnotation` and stored under `_markdown_annotations` reserved key

See `docs/annotation-format.md` for full specification.

### Analysis Manifests

`manifest.ron` files define analysis bundle behavior.

**Location**: Must be at the root of a `.analysis/` directory.

**Format**: RON (Rusty Object Notation), deserializes to `Manifest` struct.

**Required fields**:
- `name`: Analysis name (string)
- `image`: Docker image to run (string)
- `provides`: Output files generated by the analysis (list of strings)

**Optional fields**:
- `dependencies`: Input files required (list of strings)
- `command`: Override container entrypoint (list of strings)

**Example**:
```ron
Manifest(
  name: "Thermal Analysis",
  image: "python:3.11-slim",
  dependencies: ["propulsion/engine.yaml"],
  provides: ["thermal_results.csv", "plots/temp_profile.png"],
  command: Some(["python", "script.py"]),
)
```

See `src/analysis/manifest.rs` for the canonical struct definition.

## Name Sanitization

Different file types apply different sanitization rules to ensure valid template keys.

### File and Directory Names

Handled by `data_name()` in `src/node.rs`:

```rust
// Example transformations:
"mass.properties" → "mass_properties"  // Dots → underscores
"solar-arrays"    → "solar-arrays"     // Hyphens preserved
"Power_Systems"   → "Power_Systems"    // Case preserved
```

### CSV Column Names

CSV headers are sanitized to valid template identifiers:

1. **Lowercase**: `"Thrust"` → `"thrust"`
2. **Spaces/Hyphens → Underscores**: `"Isp (s)"` → `"isp__s_"`, `"delta-v"` → `"delta_v"`
3. **Leading Digits**: `"1st_stage"` → `"_1st_stage"`

**Example**:
```csv
Engine Name,Thrust (N),Specific Impulse
RL-10,110000,465
```

Becomes accessible as:
```jinja
{{ engines.col.engine_name[0] }}         {# "RL-10" #}
{{ engines.col.thrust__n_[0] }}          {# 110000 #}
{{ engines.col.specific_impulse[0] }}    {# 465 #}
```

### Excel Sheet Names

XLSX sheet names use the same sanitization rules as CSV columns:

```
Sheet: "Propulsion Systems" → propulsion_systems
Sheet: "1st Stage"          → _1st_stage
```

**Access pattern**:
```jinja
{{ data.propulsion_systems.row[0] }}
{{ data._1st_stage.col.engine[0] }}
```

## The `_`-Prefix Convention

Keys starting with underscore (`_`) are reserved for metadata and are filtered from:
- Template serialization (not visible in `{{ debug_data }}`)
- Analysis bundle `provides()` dependency checking

**Reserved keys**:
- `_source`: File provenance (`Value::Source { path }`)
- `_annotations`: Data annotations (`Value::Annotation`)
- `_markdown_annotations`: Markdown annotations (`Value::MarkdownAnnotation`)

**User-defined `_` keys**:
Projects can use `_`-prefixed keys for custom metadata that should not appear in templates.

See `docs/underscore-convention.md` for full specification.

## CSV and Excel Data

Tabular formats provide dual-access patterns for row-oriented and column-oriented operations.

### CSV Structure

**File** (`engines.csv`):
```csv
name,thrust_n,isp_s
RL-10,110000,465
RS-25,2279000,452
Merlin,845000,311
```

**Loaded as**:
```json
{
  "row": [
    ["RL-10", 110000, 465],
    ["RS-25", 2279000, 452],
    ["Merlin", 845000, 311]
  ],
  "col": {
    "name": ["RL-10", "RS-25", "Merlin"],
    "thrust_n": [110000, 2279000, 845000],
    "isp_s": [465, 452, 311]
  }
}
```

**Template access** (0-indexed):
```jinja
{# Row-oriented #}
{{ engines.row[0][0] }}        {# "RL-10" #}
{{ engines.row[2][1] }}        {# 845000 #}

{# Column-oriented #}
{{ engines.col.name[0] }}      {# "RL-10" #}
{{ engines.col.thrust_n[1] }}  {# 2279000 #}

{# Iteration #}
{% for r in engines.row %}
Engine: {{ r[0] }}, Thrust: {{ r[1] }} N, Isp: {{ r[2] }} s
{% endfor %}

{% for name in engines.col.name %}
{{ loop.index0 }}: {{ name }}
{% endfor %}
```

### Excel Structure

XLSX files create a map of sheet names to CSV-like structures:

**File** (`spacecraft.xlsx`) with sheets "Engines" and "Missions":

**Loaded as**:
```json
{
  "engines": {
    "row": [["RL-10", 110000, 465]],
    "col": {
      "name": ["RL-10"],
      "thrust_n": [110000],
      "isp_s": [465]
    }
  },
  "missions": {
    "row": [["PSR-001", "Phobos", 5.2]],
    "col": {
      "mission_id": ["PSR-001"],
      "target": ["Phobos"],
      "delta_v": [5.2]
    }
  }
}
```

**Template access**:
```jinja
{# Sheet access #}
{{ spacecraft.engines.col.name[0] }}     {# "RL-10" #}
{{ spacecraft.missions.col.target[0] }}  {# "Phobos" #}

{# Iterate over sheets #}
{% for r in spacecraft.engines.row %}
Engine: {{ r[0] }}
{% endfor %}
```

### Type Inference

CSV and Excel values are automatically parsed:

- `"42"` → `42` (Integer)
- `"3.14"` → `3.14` (Float)
- `"true"` / `"false"` → `true` / `false` (Bool)
- `"100 N"` → `Quantity { value: 100.0, unit: Unit::Newton }` (Quantity)
- `"2025-01-01T12:00:00 UTC"` → `Utc(...)` (Time epoch)
- Otherwise → `String`

This allows numeric operations and unit conversions directly on tabular data.

## Example Project Layout

The `phobos_sr` test fixture demonstrates typical project organization:

```
phobos_sr/
├── propulsion.yaml                      # Top-level data: {{ propulsion.thrust }}
├── mission.json                         # Top-level data: {{ mission.delta_v }}
├── power.toml                           # Top-level data: {{ power.P0 }}
├── template.md.j2                       # Report template → _output/template.md
├── propulsion/                          # Subsystem directory
│   ├── engine_data.csv                  # Tabular data: {{ propulsion.engine_data.row }}
│   └── thermal_analysis.analysis/       # Analysis bundle
│       ├── manifest.ron                 # Analysis configuration
│       ├── script.py.j2                 # Template: rendered before execution
│       ├── config.json                  # Static file: copied to _output/
│       └── README.md                    # Documentation (not loaded)
├── mission/
│   ├── trajectory.yaml                  # Nested data: {{ mission.trajectory.apoapsis }}
│   └── trajectory.annotations.ron       # Annotations for trajectory.yaml
└── _output/                             # Generated output directory
    ├── template.md                      # Rendered from template.md.j2
    └── propulsion/
        └── thermal_analysis.analysis/
            ├── script.py                # Rendered from script.py.j2
            ├── config.json              # Copied from static file
            └── thermal_results.csv      # Generated by analysis execution
```

**Key access patterns**:
```jinja
{# Top-level files #}
{{ propulsion.thrust }}                              # From propulsion.yaml
{{ mission.delta_v }}                                # From mission.json
{{ power.P0 }}                                       # From power.toml

{# Nested data #}
{{ mission.trajectory.apoapsis }}                    # From mission/trajectory.yaml
{{ propulsion.engine_data.row[0] }}                  # From propulsion/engine_data.csv

{# Output merging (after rendering) #}
{{ propulsion.thermal_analysis.thermal_results }}    # From _output/propulsion/.../thermal_results.csv
```

## Project Input Formats

VVERDAD accepts two input formats with identical logical structure:

### Directory Input

Standard filesystem directory:
```bash
vv ./phobos_sr
```

All file operations use normal filesystem APIs. Analysis bundles execute with Docker volume mounts.

### VV Archive Input

Zip file with `.vv` extension:
```bash
vv phobos_sr.vv
```

The archive is treated as a virtual directory. File operations use zip extraction APIs.

**Limitations**:
- Analysis bundles cannot execute (Docker requires filesystem paths for volume mounts)
- Discovery and rendering work correctly, but execution is skipped with a warning

### Output Formats

Control output location with command-line flags:

| Flag | Behavior |
|------|----------|
| (none) | In-place: writes `_output/` inside input |
| `-d <DIR>` | Copies project to `<DIR>/`, renders `<DIR>/_output/` |
| `-f <FILE>` | Creates `.vv` archive containing project + `_output/` |
| `-y` | Skip confirmation prompts (useful for CI/scripting) |

Run `vv --help` for full command-line reference.

## Best Practices

### Organization

- **Group by subsystem**: Use directories for major vehicle subsystems (propulsion, power, thermal, etc.)
- **Flat when possible**: Avoid deep nesting unless it maps to real subsystem hierarchy
- **Consistent naming**: Use lowercase with underscores or hyphens, avoid spaces

### File Naming

- **Descriptive stems**: `engine_performance.yaml` is clearer than `data1.yaml`
- **Avoid dots**: Use underscores instead (`mass_properties.yaml`, not `mass.properties.yaml`)
- **Match templates**: If a template expects `{{ mission.trajectory }}`, name the file `mission/trajectory.yaml`

### Data vs Templates

- **Separate concerns**: Keep data files (`.yaml`, `.json`) separate from templates (`.j2`)
- **Template naming**: Use double extensions to show output format (`report.md.j2` → `report.md`)
- **Partial templates**: Store reusable template fragments in a `templates/` subdirectory

### Analysis Bundles

- **One analysis per bundle**: Each `.analysis/` directory should perform a single, well-defined analysis
- **Clear naming**: Use descriptive names like `thermal_analysis.analysis`, not `analysis1.analysis`
- **Document dependencies**: List all required inputs in `manifest.ron` dependencies field
- **Standalone execution**: Bundle should run correctly with only the declared dependencies

### Version Control

- **Ignore `_output/`**: Add `_output/` to `.gitignore` (regenerated on each run)
- **Commit data and templates**: Check in all `.yaml`, `.json`, `.j2` files
- **Document structure**: Include a README.md at the project root explaining the layout
- **Track manifests**: Commit `manifest.ron` files for analysis bundles

## Related Documentation

- `docs/value-type-spec.md` - Full specification of the custom `Value` type system
- `docs/time-systems.md` - Aerospace time system support (UTC/TDB/TT/TAI)
- `docs/annotation-format.md` - Annotation sidecar file format and usage
- `docs/underscore-convention.md` - Reserved keys and metadata filtering
- `CLAUDE.md` - Developer instructions and architecture overview
