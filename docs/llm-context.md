# VVERDAD LLM Context Guide

This document provides quick-reference context for LLMs assisting users working with VVERDAD projects. For developers modifying the VVERDAD codebase itself, see `CLAUDE.md`.

## What VVERDAD Does

VVERDAD is a stateless CLI tool that processes a project directory containing design data files (JSON, YAML, TOML, RON, CSV, XLSX, etc.) and Minijinja templates (.j2), rendering templates with the loaded data and optionally executing analysis bundles in Docker containers. It's designed for aerospace vehicle design workflows but is general-purpose for any template-driven data pipeline. The tool loads all data files into a hierarchical key-value structure based on directory and file names, then uses that structure to fill Jinja2-style templates, generating output files in an `_output/` directory.

**When to use VVERDAD:**
- Generating reports from structured design data
- Creating CI/CD artifacts from configuration files
- Running containerized analyses with templated inputs
- Transforming multi-format data into standardized outputs

**When NOT to use VVERDAD:**
- Real-time data processing or streaming
- Interactive applications requiring user input during execution
- Database operations or persistent state management
- Long-running services or daemons

## CLI Commands

VVERDAD provides two commands:

**Process a project** (default):
```bash
vv <INPUT> [-d <DIR>] [-f <FILE>] [-y]
```

**Generate CI/CD scaffolding**:
```bash
vv init [DIR] [--github] [--gitlab] [--hooks] [--all] [--force]
```

When no flags are specified, `vv init` generates all CI/CD files (GitHub Actions, GitLab CI, git hooks).

## API Overview

VVERDAD provides a Rust library API for embedding in other tools:

```rust
use vverdad::{create_app, create_app_with_output, run_app};

// Create app with in-place output (writes _output/ inside input directory)
let mut app = create_app("path/to/project".into())?;

// Create app with custom output destination
let mut app = create_app_with_output("path/to/project".into(), output_config)?;

// Execute the full processing pipeline
run_app(&mut app);

// Check if any errors occurred during processing
if app.has_errors {
    // Handle errors
}
```

**Input formats:**
- Directory path: standard filesystem directory
- Archive path: `.vv` file (zip archive with project contents)

**Output options:**
- In-place: writes `_output/` inside input directory
- Directory (`-d`): copies project to directory, renders `_output/` inside
- Archive (`-f`): creates `.vv` zip with project + `_output/`

## Data Flow

VVERDAD executes six processing stages in order:

1. **Load** — Read all files from the project directory into ECS components
   - Data files parsed into `Value` types based on extension
   - Templates loaded into Minijinja environment
   - Directory structure mapped to key namespaces

2. **Discover** — Find analysis bundles and parse their manifests
   - Locate directories with `.analysis/` suffix
   - Parse `manifest.ron` files
   - Register analysis metadata

3. **Validate** — Check template dependencies
   - Extract `provides()` calls from templates
   - Verify required keys exist in loaded data
   - Report missing dependencies

4. **Render** — Fill templates with data and write output
   - Build hierarchical data tree from ECS components
   - Render all templates with Minijinja
   - Write rendered output to `_output/` directory
   - Merge `_output/` contents back into data tree

5. **Execute** — Run Docker containers for analysis bundles
   - Copy rendered templates and static files to container
   - Execute analysis command specified in manifest
   - Capture container logs and exit codes
   - Skip if Docker not available (prints warning)

6. **Validate Outputs** — Verify analysis outputs
   - Check that expected output files exist
   - Report missing files as errors

## Project Directory Structure

A VVERDAD project is organized as follows:

```
project/
├── subsystem1/
│   ├── data.yaml          # Data file: accessed as {{ subsystem1.data.* }}
│   ├── config.json        # Data file: accessed as {{ subsystem1.config.* }}
│   └── report.md.j2       # Template: outputs to _output/subsystem1/report.md
├── subsystem2/
│   ├── params.toml        # Data file: accessed as {{ subsystem2.params.* }}
│   └── analysis.analysis/ # Analysis bundle directory
│       ├── manifest.ron   # Analysis bundle manifest
│       ├── input.txt.j2   # Template for analysis input
│       └── script.py      # Static file (copied to container)
├── engines.csv            # CSV data: accessed as {{ engines.row[i] }} or {{ engines.col.name }}
├── data.xlsx              # Excel data: accessed as {{ data.sheet_name.row[i] }}
├── summary.md.j2          # Template: outputs to _output/summary.md
└── _output/               # Generated output directory (created by VVERDAD)
    ├── subsystem1/
    │   └── report.md      # Rendered from subsystem1/report.md.j2
    ├── subsystem2/
    │   └── analysis.analysis/
    │       ├── input.txt  # Rendered analysis input
    │       └── results/   # Analysis execution outputs
    └── summary.md         # Rendered from summary.md.j2
```

**Key mapping rules:**
- Directory names become key namespaces: `subsystem1/data.yaml` → `{{ subsystem1.data }}`
- File names (minus extension) become keys: `data.yaml` → `{{ subsystem1.data }}`
- Dots in filenames become underscores: `my.file.yaml` → `{{ my_file }}`
- Keys starting with `_` are filtered from template access (reserved for metadata)

**Recognized data file extensions:**
- Text: `.json`, `.yaml`, `.yml`, `.toml`, `.ron`
- Binary: `.msgpack`, `.mp`, `.pickle`, `.pkl`, `.cbor`, `.bson`
- Tabular: `.csv`, `.xlsx`
- Markdown: `.md` (with YAML front matter support)

**Template extensions:**
- `.j2`, `.jinja`, `.jinja2`, `.tmpl`

**Special directories:**
- `_output/`: Generated output, merged back into data tree (source data takes precedence)
- `*.analysis/`: Analysis bundle directories with manifest.ron

## Template Quick Reference

VVERDAD templates use Minijinja (Jinja2-compatible) syntax with custom filters:

### Unit Conversion Filters

| Filter | Input | Output | Example | Result |
|--------|-------|--------|---------|--------|
| `to("unit")` | quantity | converted quantity string | `"100 kN" \| to("lbf")` | `"22480.9 lbf"` |
| `value("unit")` | quantity | numeric value (float) | `"100 kN" \| value("N")` | `100000.0` |
| `unit` | quantity | unit symbol (string) | `"100 kN" \| unit` | `"kN"` |
| `si` | quantity | SI-normalized quantity | `"100 kN" \| si` | `"100000 N"` |

### Time Conversion Filters

| Filter | Input | Output | Example | Result |
|--------|-------|--------|---------|--------|
| `to_utc` | epoch | UTC epoch string | `"2024-01-01T12:00:00 TDB" \| to_utc` | `"2024-01-01T11:58:54.816 UTC"` |
| `to_tdb` | epoch | TDB epoch string | `"2024-01-01T12:00:00 UTC" \| to_tdb` | `"2024-01-01T12:01:05.184 TDB"` |
| `jd` | epoch | Julian Date (float) | `"2024-01-01T00:00:00 UTC" \| jd` | `2460310.5` |
| `mjd` | epoch | Modified Julian Date (float) | `"2024-01-01T00:00:00 UTC" \| mjd` | `60310.0` |

### Data Validation

VVERDAD automatically detects which variables a template references (via Minijinja's `undeclared_variables()`) and validates them against available data keys before rendering. No explicit declaration is needed in templates.

## Common Patterns

### Adding a Data File

1. Create file in desired directory: `subsystem/data.yaml`
2. Add data in YAML format:
   ```yaml
   thrust: "110 kN"
   isp: "465 s"
   mass: "167 kg"
   ```
3. Access in templates as `{{ subsystem.data.thrust }}`

### Adding a Template

1. Create template file: `report.md.j2`
2. Write template content:
   ```jinja
   # Engine Report

   Thrust: {{ subsystem.data.thrust | to("lbf") }}
   Specific Impulse: {{ subsystem.data.isp | value("s") }} seconds
   ```
3. Run VVERDAD: output appears as `_output/report.md`

### Working with CSV Files

Given `engines.csv`:
```csv
name,thrust_n,isp_s
RL-10,110000,465
RS-25,2279000,452
```

Access in templates:
```jinja
{# Row-oriented access (0-indexed) #}
First engine: {{ engines.row[0][0] }}
First engine thrust: {{ engines.row[0][1] }} N

{# Column-oriented access #}
All engine names: {{ engines.col.name }}
First engine thrust: {{ engines.col.thrust_n[0] }} N

{# Iteration #}
{% for r in engines.row %}
{{ r[0] }}: {{ r[1] }} N, {{ r[2] }} s
{% endfor %}
```

### Working with Excel Files

Given `data.xlsx` with sheets "Engines" and "Missions":
```jinja
{# Access by sheet name (sanitized: lowercase, spaces→underscores) #}
{{ data.engines.col.name[0] }}
{{ data.missions.row[0][0] }}

{# Iterate over sheet rows #}
{% for r in data.engines.row %}
Engine: {{ r[0] }}
{% endfor %}
```

### Creating an Analysis Bundle

1. Create directory: `thermal_analysis.analysis/`
2. Create manifest: `thermal_analysis.analysis/manifest.ron`
   ```ron
   Analysis(
       id: "thermal_analysis",
       version: "1.0.0",
       image: "python:3.11-slim",
       entrypoint: "analyze.py",
       inputs: [
           Input(key: "power.heat_load", required: true),
       ],
       outputs: [
           Output(key: "results/temperature_profile.csv"),
       ],
       templates: [
           Template(source: "input.json.j2", destination: "input.json"),
       ],
       static_files: ["analyze.py"],
   )
   ```
3. Create template: `thermal_analysis.analysis/input.json.j2`
   ```jinja
   {
       "heat_load": {{ power.heat_load | value("W") }}
   }
   ```
4. Create script: `thermal_analysis.analysis/analyze.py`
5. Run VVERDAD: analysis renders templates, executes in Docker, outputs to `_output/thermal_analysis.analysis/results/`

### Multi-Format Data Access

Data is accessible regardless of source format:
```jinja
{# These all work the same way #}
{{ propulsion.thrust }}  {# from propulsion.yaml #}
{{ power.P0 }}           {# from power.json #}
{{ structure.mass }}     {# from structure.toml #}
{{ mission.target }}     {# from mission.ron #}
```

## Troubleshooting

### Missing Keys in Templates

**Symptom:** Template references `{{ subsystem.data.key }}` but key not found

**Solutions:**
- Check file name: `subsystem/data.yaml` must exist
- Check key exists in file: open YAML and verify `key:` entry
- Check key name mapping: dots in filenames become underscores
- Run with validation enabled to see missing dependencies

### Unsupported File Format

**Symptom:** File not loaded as data

**Solutions:**
- Check extension is in supported list (see Data File Extensions above)
- Rename file to use supported extension
- Convert to supported format (JSON, YAML, TOML recommended)

### Docker Not Available

**Symptom:** Warning printed: "Docker not available, skipping analysis execution"

**Solutions:**
- Install Docker and ensure daemon is running
- Analysis templates still render to `_output/`, can be run manually
- For CI/CD, ensure Docker available in pipeline environment

### Pre-1972 Date Errors

**Symptom:** UTC time parsing fails for dates before 1972-01-01

**Explanation:** VVERDAD's leap second table starts in 1972

**Solutions:**
- Use TDB time system for pre-1972 dates (TDB has no leap seconds)
- Convert dates to TDB in source data: `"1969-07-20T20:17:40 TDB"`

### Stale Output Data

**Symptom:** Old data appears in templates that reference `_output/` keys

**Explanation:** `_output/` is merged back into data tree

**Solutions:**
- Delete `_output/` directory before running VVERDAD
- Source data always takes precedence over output data
- Check for key name collisions between source and output

### Quantity Not Parsed

**Symptom:** Quantity string appears as plain string, unit filters fail

**Solutions:**
- Ensure format is `"<number> <unit>"` with space separator: `"100 N"` not `"100N"`
- Check unit is recognized: see `src/units/` for supported units
- Use structured format in data files:
  ```yaml
  thrust:
    value: 100
    unit: "kN"
  ```

### Template Rendering Errors

**Symptom:** Minijinja error during template rendering

**Solutions:**
- Check template syntax: ensure `{{`, `}}`, `{%`, `%}` are balanced
- Verify filter names: `to`, `value`, `unit`, `si`, `to_utc`, `to_tdb`, `jd`, `mjd`
- Check provides() declarations match actual data keys
- Test with minimal template to isolate issue

### Analysis Bundle Execution Fails

**Symptom:** Docker container exits with non-zero code

**Solutions:**
- Check container logs in terminal output
- Verify image name in manifest.ron matches available Docker image
- Ensure command in manifest.ron is correct for image
- Test command manually: `docker run <image> <command>`
- Check input_files rendered correctly in `_output/<bundle>/`

## Key Differences from CLAUDE.md

**CLAUDE.md** is for developers modifying the VVERDAD codebase:
- Explains Bevy ECS architecture and internal data structures
- Documents Rust API implementation details
- Describes development workflow and testing strategy
- Covers internal modules like `value.rs`, `node.rs`, `source.rs`

**This document** is for users and LLMs helping users:
- Focuses on project directory structure and template syntax
- Provides quick-reference tables for filters and patterns
- Explains how to add data files, templates, and analysis bundles
- Covers common user issues and troubleshooting

If you're helping a user **create or debug a VVERDAD project**, use this document.

If you're helping a user **modify VVERDAD's source code**, use CLAUDE.md.
