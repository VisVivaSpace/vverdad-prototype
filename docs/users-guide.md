# VVERDAD User's Guide

This guide walks through setting up and using VVERDAD for aerospace design data management.  It covers installation, creating a project, working with data and templates, running analyses, using annotations for design review, and automating workflows with CI/CD.

For a complete working example, see the [vverdad-test-mission](https://github.com/VisVivaSpace/vverdad-test-mission) repository, which demonstrates a Phobos sample return mission concept.

## Installation

### From a Prebuilt Binary

Download the latest release binary for your platform:

```bash
# Linux (x86_64)
curl -sL https://github.com/VisVivaSpace/vverdad-prototype/releases/latest/download/vv-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv vv /usr/local/bin/

# macOS (Apple Silicon)
curl -sL https://github.com/VisVivaSpace/vverdad-prototype/releases/latest/download/vv-aarch64-apple-darwin.tar.gz | tar xz
sudo mv vv /usr/local/bin/
```

### From Source

Requires Rust 1.85 or later:

```bash
git clone https://github.com/VisVivaSpace/vverdad-prototype.git
cd vverdad-prototype
cargo install --path .
```

### Verify Installation

```bash
vv --help
```

## Creating a Project

A VVERDAD project is a directory containing data files and templates.  There is no configuration file or database to set up — the directory structure itself defines the data namespace.

### Step 1: Create the Project Directory

```bash
mkdir my-spacecraft
cd my-spacecraft
```

### Step 2: Add Design Data

Create data files in any supported format.  Organize them by subsystem or however makes sense for your project:

```bash
mkdir propulsion power mission
```

Create `propulsion/engine.yaml`:

```yaml
name: Main Engine
thrust: "22.2 kN"
isp: "321 s"
mass: "150 kg"
propellant: MMH/NTO
```

Create `power/solar_array.toml`:

```toml
type = "Triple-Junction GaAs"
area = "12.5 m^2"
efficiency = 0.295
eol_power = "2800 W"
```

Create `mission/overview.json`:

```json
{
  "name": "Phobos Sample Return",
  "launch_date": "2031-04-15T00:00:00 UTC",
  "destination": "Phobos",
  "duration": "3.5 yr",
  "delta_v": "5.2 km/s"
}
```

VVERDAD supports JSON, YAML, TOML, RON, CSV, XLSX, MessagePack, CBOR, BSON, and Pickle.  All formats normalize to the same internal data type, so you can use whichever format your team prefers.

### Step 3: Add Templates

Templates use Jinja2 syntax (via Minijinja) and are identified by their file extension: `.j2`, `.jinja`, `.jinja2`, or `.tmpl`.

Create `mission_summary.md.j2`:

```jinja
# {{ mission.overview.name }}

## Mission Overview

- **Destination:** {{ mission.overview.destination }}
- **Launch Date:** {{ mission.overview.launch_date | to_utc }}
- **Mission Duration:** {{ mission.overview.duration }}
- **Total Delta-V:** {{ mission.overview.delta_v }}

## Propulsion

- **Engine:** {{ propulsion.engine.name }}
- **Thrust:** {{ propulsion.engine.thrust | to("lbf") }}
- **Specific Impulse:** {{ propulsion.engine.isp | value("s") }} s

## Power

- **Solar Array Type:** {{ power.solar_array.type }}
- **Array Area:** {{ power.solar_array.area }}
- **EOL Power:** {{ power.solar_array.eol_power }}
```

### Step 4: Run VVERDAD

```bash
vv .
```

VVERDAD loads all data files, renders all templates, and writes outputs to the `_output/` directory:

```
my-spacecraft/_output/
└── mission_summary.md
```

The rendered `mission_summary.md` will contain the values from your data files, with unit conversions applied where specified.

## How Data Access Works

### Directory-to-Namespace Mapping

The project directory structure maps directly to dot-separated keys in templates.  File and directory names become keys:

| File Path | Template Access |
|-----------|----------------|
| `propulsion/engine.yaml` → `thrust` field | `{{ propulsion.engine.thrust }}` |
| `power/solar_array.toml` → `area` field | `{{ power.solar_array.area }}` |
| `mission/overview.json` → `name` field | `{{ mission.overview.name }}` |

Nested data within files extends the dot path:

```yaml
# propulsion/engine.yaml
performance:
  sea_level:
    thrust: "100 kN"
  vacuum:
    thrust: "110 kN"
```

```jinja
Vacuum Thrust: {{ propulsion.engine.performance.vacuum.thrust }}
```

### Physical Quantities

VVERDAD automatically parses strings like `"22.2 kN"` or `"5.5 km/s"` as physical quantities with a numeric value and unit.  Templates can convert between units:

```jinja
{{ propulsion.engine.thrust }}              {# "22.2 kN" (original) #}
{{ propulsion.engine.thrust | to("lbf") }}  {# "4993.15 lbf" (converted) #}
{{ propulsion.engine.thrust | value("N") }} {# 22200.0 (numeric value in N) #}
{{ propulsion.engine.thrust | unit }}        {# "kN" (original unit string) #}
{{ propulsion.engine.thrust | si }}          {# "22200 N" (SI base units) #}
```

### Time Epochs

Time strings in recognized formats (ISO 8601, SPICE-style) are parsed as time epochs.  VVERDAD supports UTC, TDB, TT, and TAI time scales:

```jinja
{{ mission.overview.launch_date }}                {# original format #}
{{ mission.overview.launch_date | to_utc }}       {# convert to UTC #}
{{ mission.overview.launch_date | to_tdb }}       {# convert to TDB #}
{{ mission.overview.launch_date | jd }}            {# Julian Date #}
{{ mission.overview.launch_date | mjd }}           {# Modified Julian Date #}
```

### Tabular Data (CSV and Excel)

CSV and XLSX files are loaded as tables with row and column access:

Given `engines.csv`:

```csv
name,thrust_N,isp_s
RS-25,2279000,452
Merlin 1D,845000,311
RD-180,4152000,338
```

Access in templates:

```jinja
{# Column access #}
{% for name in engines.col.name %}
- {{ name }}
{% endfor %}

{# Row access #}
{% for row in engines.row %}
- {{ row.name }}: {{ row.thrust_n }} N, {{ row.isp_s }} s Isp
{% endfor %}

{# Individual cell #}
First engine: {{ engines.col.name[0] }}
```

Note: CSV column headers are sanitized to lowercase with spaces replaced by underscores (`Thrust (N)` becomes `thrust__n_`).

For Excel files, sheets are accessed by name:

```jinja
{{ budget.Sheet1.col.component[0] }}
{{ budget.Power.col.watts[2] }}
```

## Running Analysis Tools

VVERDAD can execute engineering analysis scripts in Docker containers through analysis bundles.

### Creating an Analysis Bundle

An analysis bundle is a directory with a `.analysis/` suffix containing a manifest file and templates or scripts:

```
propulsion/
├── engine.yaml
└── thermal.analysis/
    ├── manifest.ron
    ├── run_thermal.py.j2
    └── reference_data.csv
```

The manifest file (`manifest.ron`) declares the analysis configuration:

```ron
AnalysisManifest(
    name: "Propulsion Thermal Analysis",
    image: "python:3.12-slim",
    sources: {
        "run_thermal.py.j2": "run_thermal.py",
    },
    statics: {
        "reference_data.csv": "reference_data.csv",
    },
    expected_outputs: [
        "thermal_results.json",
    ],
)
```

- **`name`**: Display name for the analysis
- **`image`**: Docker image to use for execution
- **`sources`**: Template files to render (key: template, value: output filename in container)
- **`statics`**: Files to copy to the container without rendering
- **`expected_outputs`**: Files the analysis should produce (validated after execution)

### Template for Analysis Input

The template `run_thermal.py.j2` has access to the full project data:

```jinja
import json

thrust = {{ propulsion.engine.thrust | value("N") }}
isp = {{ propulsion.engine.isp | value("s") }}

# ... thermal analysis calculations ...

results = {"max_temp": max_temp, "heat_flux": heat_flux}
with open("thermal_results.json", "w") as f:
    json.dump(results, f)
```

### Running with Docker

When Docker is available, VVERDAD discovers analysis bundles, renders their templates, executes them in containers, and validates outputs:

```bash
vv .
```

Analysis outputs appear in `_output/` and are loaded back into the data tree for use by downstream templates.  The reactive rendering pipeline iterates until no new data is produced, allowing chains of dependent analyses.

### Running without Docker

If Docker is unavailable, VVERDAD still discovers and renders analysis bundle templates, but skips execution.  This allows template-only workflows without Docker infrastructure.  The process exits successfully (exit code 0) with a warning.

## Data Annotations

VVERDAD's annotation system allows review comments, questions, issues, and suggestions to be attached to specific data points without modifying the original data files.  Annotations are stored in sidecar files alongside the data they annotate.

### Creating a Data Annotation

For a data file `propulsion/engine.yaml`, create a sidecar file `propulsion/engine.annotations.ron`:

```ron
{
    "thrust": [
        Annotation(
            ann_type: Comment,
            author: "Jane Smith",
            text: "Verified against RD-180 spec sheet, rev 3.2",
            status: Resolved,
            tags: ["verification", "propulsion"],
            replies: [],
        ),
    ],
    "isp": [
        Annotation(
            ann_type: Question,
            author: "Bob Chen",
            text: "Is this sea-level or vacuum Isp?",
            status: Open,
            tags: ["clarification"],
            replies: [
                Reply(
                    author: "Jane Smith",
                    text: "Vacuum Isp. Will add a note to the data file.",
                    created: "2024-11-15",
                ),
            ],
        ),
    ],
}
```

### Annotation Types

| Type | Use Case |
|------|----------|
| `Comment` | General notes and observations |
| `Question` | Requests for clarification |
| `Issue` | Problems that need resolution |
| `Suggestion` | Proposed changes or improvements |

### Status Values

| Status | Meaning |
|--------|---------|
| `Open` | Needs attention |
| `InProgress` | Being worked on |
| `Resolved` | Addressed |
| `Accepted` | Suggestion accepted |
| `Rejected` | Suggestion rejected |

### Markdown Annotations

For markdown files, annotations can reference specific positions in the document.  For a file `reports/summary.md`, the sidecar is `reports/summary.md.annotations.ron`:

```ron
[
    MarkdownAnnotation(
        ann_type: Comment,
        author: "Review Board",
        text: "This section needs updated mass numbers from the latest PDR.",
        status: Open,
        tags: ["review"],
        heading: "Mass Budget",
        paragraph: 2,
        replies: [],
    ),
]
```

Annotations are part of the data tree (stored under `_annotations` and `_markdown_annotations` keys) but do not modify the original data files and are not serialized into template output by default.

## CI/CD Integration

VVERDAD integrates with standard CI/CD pipelines to automate the design-build-test cycle.

### Generate CI/CD Configuration

```bash
vv init                       # All configurations in current directory
vv init --github              # GitHub Actions workflow only
vv init --gitlab              # GitLab CI/CD pipeline only
vv init --hooks               # Git pre-commit and pre-push hooks only
vv init --all ./my-project    # All configurations in a specific directory
vv init --force               # Overwrite existing configuration files
```

### Git Hooks

The generated git hooks provide local validation:

- **Pre-commit hook**: Validates templates before allowing commits — catches syntax errors early
- **Pre-push hook**: Runs a full render validation before pushing — ensures all templates render successfully with the current data

After generating hooks:

```bash
git config core.hooksPath .githooks
```

### GitHub Actions

The generated `.github/workflows/vverdad.yml` workflow:

1. Installs the `vv` binary
2. Runs VVERDAD on the project
3. Commits rendered outputs back to the repository

This closes the continuous integration loop: engineers push data changes, VVERDAD renders templates and runs analyses, and the results are committed back for review.

### GitLab CI/CD

The generated `.gitlab-ci.yml` pipeline provides equivalent functionality for GitLab-hosted repositories.

### CLI Flags for Automation

| Flag | Purpose |
|------|---------|
| `-y` | Skip confirmation prompts (required for non-interactive CI environments) |
| `-d <DIR>` | Write output to a specific directory instead of `_output/` |
| `-f <FILE>` | Write output to a `.vv` archive file |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (including when Docker is unavailable — analysis bundles are skipped with a warning) |
| 1 | Fatal error (missing input, parse failure, template error) |

## Working with the Example Project

The [vverdad-test-mission](https://github.com/VisVivaSpace/vverdad-test-mission) repository contains a complete example based on a Phobos sample return mission concept.  It demonstrates:

- Multi-format design data (YAML, JSON, TOML, CSV)
- Templates for engineering reports (mission summary, telecom summary, mass and power budgets)
- Python analysis scripts with Docker execution (telecom link budget)
- Julia Jupyter notebooks (mass and power margin analysis)
- Julia Quarto documents (subsystem summary report rendered to HTML)

### Running the Example

```bash
git clone https://github.com/VisVivaSpace/vverdad-test-mission.git
vv ./vverdad-test-mission
```

Rendered outputs appear in `vverdad-test-mission/_output/`.  If Docker is installed, analysis scripts execute and their results are incorporated into downstream reports.

### Exploring the Data

After running, examine the output directory to see how templates pulled data from across the project:

```bash
ls vverdad-test-mission/_output/
```

The example project demonstrates the key workflow: design data is organized by subsystem, templates reference data from any subsystem using dot notation, and reports are generated with consistent, up-to-date values from the design database.

## Common Patterns

### Conditional Content

```jinja
{% if propulsion.engine.thrust | value("N") > 20000 %}
This engine meets the minimum thrust requirement.
{% else %}
**WARNING:** Engine thrust is below the 20 kN minimum.
{% endif %}
```

### Iterating Over Data

```jinja
{% for key, value in propulsion | items %}
- {{ key }}: {{ value }}
{% endfor %}
```

### Calculations in Templates

```jinja
{% set thrust_n = propulsion.engine.thrust | value("N") %}
{% set mass_kg = spacecraft.dry_mass | value("kg") %}
{% set twr = thrust_n / (mass_kg * 9.81) %}
Thrust-to-weight ratio: {{ twr | round(2) }}
```

### Multi-Format Data

The same data can be stored in any supported format without changing templates.  These are equivalent:

**YAML:**
```yaml
thrust: "22.2 kN"
```

**JSON:**
```json
{"thrust": "22.2 kN"}
```

**TOML:**
```toml
thrust = "22.2 kN"
```

All three are accessed identically in templates: `{{ propulsion.engine.thrust }}`.

## Further Reading

- [Template Guide](template-guide.md) — Complete template authoring reference with all filters and patterns
- [Directory Structure](directory-structure.md) — Detailed project directory conventions and naming rules
- [Value Type Specification](value-type-spec.md) — The VVERDAD Value enum and type system
- [Time Systems](time-systems.md) — UTC/TDB time support, conversions, and leap seconds
- [Annotation Format](annotation-format.md) — Full annotation sidecar specification
- [Underscore Convention](underscore-convention.md) — The `_`-prefix metadata key convention
- [Data-Oriented Programming](data-oriented-programming.md) — Design philosophy and principles
- [CI/CD Integration](ci-cd-integration.md) — Detailed CI/CD setup and customization
- [LLM Context](llm-context.md) — Quick reference for LLM-assisted usage
