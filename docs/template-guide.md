# VVERDAD Template Authoring Guide

This guide covers how to write templates for VVERDAD, the aerospace vehicle design data processing engine. VVERDAD uses Minijinja, a Rust implementation of the Jinja2 template language.

## Getting Started

### Template File Extensions

Templates are recognized by these extensions:
- `.j2`
- `.jinja`
- `.jinja2`
- `.tmpl`

### Output Naming

The template extension is stripped when generating output files:
- `report.md.j2` → `_output/report.md`
- `analysis_script.py.jinja` → `_output/analysis_script.py`
- `config.json.tmpl` → `_output/config.json`

### Template Placement

Templates can be placed anywhere in your project directory tree:
- Top-level templates render to `_output/` at the project root
- Templates in subdirectories render to corresponding paths in `_output/`
- Templates inside `.analysis/` directories are rendered during analysis execution and follow the manifest's source/destination mapping

Example project structure:
```
my-project/
├── propulsion/
│   ├── engines.yaml
│   └── thrust_report.md.j2
├── mission/
│   └── timeline.json
└── summary.txt.j2
```

Renders to:
```
my-project/_output/
├── propulsion/
│   └── thrust_report.md
└── summary.txt
```

## Accessing Data

### Basic Dot Notation

Access data using dot notation with keys derived from file paths:

```jinja
{{ propulsion.thrust }}
{{ mission.launch_date }}
{{ power.battery_capacity }}
```

### Nested Data Access

For nested data structures within files:

```jinja
{{ propulsion.engine.isp }}
{{ mission.phases.launch.delta_v }}
{{ power.solar_array.efficiency }}
```

### Directory Hierarchy Mapping

The project's directory structure maps directly to the data namespace:

**File location:** `subsystems/propulsion/engine.yaml`
**File contents:**
```yaml
thrust: 100000
isp: 450
```

**Template access:**
```jinja
{{ subsystems.propulsion.engine.thrust }}
{{ subsystems.propulsion.engine.isp }}
```

### Data Flattening

All data files are loaded and flattened into a single namespace. Files in the same directory share a common parent in the hierarchy:

**Files:**
- `propulsion/engine.yaml`
- `propulsion/fuel.json`
- `propulsion/thermal.toml`

**Access:**
```jinja
{{ propulsion.engine.thrust }}
{{ propulsion.fuel.type }}
{{ propulsion.thermal.max_temp }}
```

## Unit Conversion Filters

VVERDAD provides filters for working with physical quantities. These filters enable unit conversion and manipulation directly in templates.

### Converting Units

The `to` filter converts a quantity to a specified unit and returns a formatted string:

```jinja
{{ propulsion.thrust | to("lbf") }}
```

**Output:** `"22480.9 lbf"` (if thrust was 100 kN)

**Common usage:**
```jinja
Thrust: {{ propulsion.thrust | to("kN") }}
Delta-V: {{ mission.delta_v | to("km/s") }}
Mass: {{ structure.dry_mass | to("lb") }}
```

### Extracting Numeric Values

The `value` filter extracts the numeric value in a specified unit:

```jinja
{{ propulsion.thrust | value("N") }}
```

**Output:** `100000.0` (as a float)

**Usage in calculations:**
```jinja
{% set thrust_n = propulsion.thrust | value("N") %}
{% set mass_kg = structure.total_mass | value("kg") %}
Thrust-to-weight ratio: {{ thrust_n / (mass_kg * 9.81) }}
```

### Extracting Units

The `unit` filter returns just the unit string:

```jinja
{{ propulsion.thrust | unit }}
```

**Output:** `"kN"` (or whatever unit the quantity is stored in)

**Usage:**
```jinja
The thrust is measured in {{ propulsion.thrust | unit }}.
```

### Normalizing to SI

The `si` filter converts a quantity to SI base units:

```jinja
{{ propulsion.thrust | si }}
{{ mission.distance | si }}
{{ power.output | si }}
```

**Examples:**
- `"100 kN"` → `"100000 N"`
- `"5.2 AU"` → `"777926100000 m"`
- `"500 hp"` → `"372850 W"`

### Graceful Degradation

If a value is not a quantity (plain number or string), filters pass it through unchanged:

```jinja
{{ mission.name | to("km") }}  {# Returns "Phobos Sample Return" unchanged #}
{{ status.code | unit }}        {# Returns "200" unchanged #}
```

This allows templates to work with mixed data types without errors.

### Common Units Reference

**Length:** m, km, cm, mm, ft, mi, nmi, AU, ly
**Mass:** kg, g, lb, ton, tonne
**Time:** s, min, hr, d, yr
**Force:** N, kN, MN, lbf, kip
**Pressure:** Pa, kPa, MPa, bar, atm, psi
**Energy:** J, kJ, MJ, kWh, cal, BTU
**Power:** W, kW, MW, hp
**Angle:** rad, deg, arcmin, arcsec
**Velocity:** m/s, km/s, km/h, mph, ft/s
**Angular velocity:** rad/s, deg/s, rpm

## Time Filters

VVERDAD supports aerospace time systems (UTC, TDB, TAI, TT) with conversion and formatting filters.

### Converting to Calendar Strings

```jinja
{{ mission.launch_epoch | to_utc }}
{{ mission.arrival_epoch | to_tdb }}
```

**Output:**
```
2030-01-15T14:30:00.000 UTC
2035-07-22T08:15:30.123 TDB
```

### Julian Date Conversion

```jinja
{{ mission.epoch | jd }}
{{ mission.epoch | mjd }}
```

**Output (as floats):**
```
2451545.0      # Julian Date
51544.5        # Modified Julian Date
```

### Usage Example

```jinja
Launch Window Analysis
Launch: {{ mission.launch | to_utc }} (JD {{ mission.launch | jd }})
Arrival: {{ mission.arrival | to_utc }} (JD {{ mission.arrival | jd }})
Transit time: {{ (mission.arrival | jd) - (mission.launch | jd) }} days
```

### Supported Input Formats

Epoch values can be specified in data files as:
- ISO 8601 strings: `"2030-01-15T14:30:00.000 UTC"`
- SPICE-style: `"15-JAN-2030 14:30:00 UTC"`
- Numeric with suffix: `"10957.6 UTC"` (days since J2000.0)

The suffix determines the time system: UTC, TDB, TT, or TAI.

## Tabular Data

### CSV Files

CSV files provide both row-oriented and column-oriented access.

**Example CSV file** (`engines.csv`):
```csv
name,thrust_n,isp_s,mass_kg
RL-10,110000,465,167
RS-25,2279000,452,3527
Merlin,845000,311,470
```

**Row-oriented access (0-indexed):**
```jinja
{{ engines.row[0][0] }}        {# "RL-10" #}
{{ engines.row[1][1] }}        {# 2279000 #}
{{ engines.row[2][3] }}        {# 470 #}
```

**Column-oriented access:**
```jinja
{{ engines.col.name[0] }}      {# "RL-10" #}
{{ engines.col.thrust_n[1] }}  {# 2279000 #}
{{ engines.col.isp_s[2] }}     {# 311 #}
```

**Iteration over rows:**
```jinja
{% for r in engines.row %}
Engine: {{ r[0] }}
  Thrust: {{ r[1] }} N
  Isp: {{ r[2] }} s
  Mass: {{ r[3] }} kg
{% endfor %}
```

**Iteration over columns:**
```jinja
{% for name in engines.col.name %}
{{ loop.index0 }}: {{ name }}
{% endfor %}
```

### XLSX Files

Excel files load each worksheet as a separate sheet with CSV-compatible access:

**Template access:**
```jinja
{{ data.engines.col.name[0] }}           {# First sheet: "engines" #}
{{ data.missions.col.target[0] }}        {# Second sheet: "missions" #}

{% for r in data.engines.row %}
Engine: {{ r[0] }}, Thrust: {{ r[1] }} N
{% endfor %}
```

### Column Name Sanitization

CSV column headers and XLSX sheet names are automatically sanitized:
- Converted to lowercase
- Spaces and hyphens → underscores
- Leading digits get `_` prefix

**Examples:**
- `"First Name"` → `first_name`
- `"Thrust (N)"` → `thrust__n_`
- `"1st Stage"` → `_1st_stage`

### Type Inference

Values in CSV and XLSX files are automatically parsed:
- `"42"` → `42` (integer)
- `"3.14159"` → `3.14159` (float)
- `"true"` / `"false"` → `true` / `false` (boolean)
- `"100 N"` → `Quantity { value: 100, unit: N }`
- Otherwise → string

This enables direct numeric operations:

```jinja
{% for r in engines.row %}
Thrust-to-weight: {{ r[1] / (r[3] * 9.81) | round(2) }}
{% endfor %}
```

## Quantities and Epochs in Templates

### Quantity Serialization

Quantity values serialize as unit-suffixed strings in templates:

```jinja
{{ propulsion.thrust }}
```

**Output:** `"100 kN"`

This means you can display them directly or apply filters:

```jinja
Thrust: {{ propulsion.thrust }}
Thrust (Imperial): {{ propulsion.thrust | to("lbf") }}
Thrust (SI): {{ propulsion.thrust | si }}
```

### Epoch Serialization

Epoch values serialize as calendar strings with time system suffix:

```jinja
{{ mission.launch }}
```

**Output:** `"2030-01-15T14:30:00.000 UTC"`

Use filters for format conversion:

```jinja
Launch: {{ mission.launch | to_utc }}
Launch (JD): {{ mission.launch | jd }}
Launch (TDB): {{ mission.launch | to_tdb }}
```

### Filter Chaining

Combine filters to extract specific representations:

```jinja
{# Convert to Newtons, then extract numeric value #}
{% set thrust_n = propulsion.thrust | to("N") | value("N") %}

{# Extract unit after conversion #}
{{ propulsion.thrust | to("lbf") | unit }}  {# "lbf" #}

{# Convert epoch to JD, then format #}
JD {{ mission.launch | jd | round(6) }}
```

## Markdown Data

Markdown files (`.md`) are loaded as structured data with separate content and front matter fields.

### Basic Access

```jinja
{{ readme.content }}
```

**Output:** The raw markdown body text.

### Front Matter Access

If the markdown file has YAML front matter:

```markdown
---
title: Mission Overview
author: Flight Dynamics Team
date: 2030-01-15
---

# Mission Overview

This document describes...
```

**Template access:**
```jinja
# {{ readme.front_matter.title }}

Author: {{ readme.front_matter.author }}
Date: {{ readme.front_matter.date }}

{{ readme.content }}
```

### Use Cases

- Embedding documentation into generated reports
- Extracting metadata from markdown files
- Including authored content in technical documents

## Analysis Bundle Templates

Templates inside `.analysis/` directories have access to the full project data tree and are rendered according to the analysis manifest.

### Manifest Structure

**Example:** `thermal_analysis.analysis/manifest.ron`
```ron
Manifest(
    name: "Thermal Analysis",
    description: "Steady-state thermal model",
    inputs: ["propulsion/engine.yaml"],
    outputs: ["thermal_results.csv"],
    static: ["static/config.json"],
    templates: {
        "templates/input.dat.j2": "input.dat",
    },
)
```

### Template Data Access

Templates in analysis bundles can reference any data in the project:

**File:** `thermal_analysis.analysis/templates/input.dat.j2`
```jinja
# Thermal Analysis Input
THRUST {{ propulsion.engine.thrust | value("N") }}
ISP {{ propulsion.engine.isp }}
AMBIENT_TEMP {{ environment.temperature | value("K") }}
ALTITUDE {{ mission.orbit.altitude | value("km") }}
```

This template has full access to the project's data tree, not just the declared inputs.

## Common Patterns

### Conditional Rendering

```jinja
{% if propulsion.thrust %}
Thrust: {{ propulsion.thrust | to("kN") }}
{% else %}
Thrust: Not specified
{% endif %}
```

### Iteration Over Maps

```jinja
{% for key, val in propulsion | items %}
{{ key }}: {{ val }}
{% endfor %}
```

### Default Values

```jinja
{{ propulsion.thrust | default("N/A") }}
{{ mission.name | default("Unnamed Mission") }}
```

### Unit Conversion Tables

```jinja
| Parameter | Value (SI) | Value (Imperial) |
|-----------|------------|------------------|
| Thrust | {{ propulsion.thrust | si }} | {{ propulsion.thrust | to("lbf") }} |
| Isp | {{ propulsion.isp | si }} | {{ propulsion.isp | to("lbf*s/lb") }} |
| Mass | {{ structure.mass | si }} | {{ structure.mass | to("lb") }} |
```

### Conditional Units

```jinja
{% if units == "imperial" %}
Thrust: {{ propulsion.thrust | to("lbf") }}
{% else %}
Thrust: {{ propulsion.thrust | si }}
{% endif %}
```

### Numeric Calculations with Units

```jinja
{% set thrust_n = propulsion.thrust | value("N") %}
{% set mass_kg = structure.total_mass | value("kg") %}
{% set g = 9.80665 %}

Thrust-to-Weight Ratio: {{ (thrust_n / (mass_kg * g)) | round(2) }}
```

### Multi-Line Expressions

```jinja
{% set total_delta_v =
    mission.launch_dv | value("m/s") +
    mission.transfer_dv | value("m/s") +
    mission.landing_dv | value("m/s")
%}

Total Delta-V Budget: {{ total_delta_v }} m/s
```

### Date Range Calculations

```jinja
{% set launch_jd = mission.launch | jd %}
{% set arrival_jd = mission.arrival | jd %}
{% set transit_days = arrival_jd - launch_jd %}

Launch: {{ mission.launch | to_utc }}
Arrival: {{ mission.arrival | to_utc }}
Transit Time: {{ transit_days | round(1) }} days
```

### Generating Lists

```jinja
{% for i in range(10) %}
{{ i }}: {{ propulsion.thrust | value("N") * (i + 1) / 10 }} N
{% endfor %}
```

### CSV-Based Reports

```jinja
# Engine Comparison Report

{% for row in engines.row %}
## {{ row[0] }}
- Thrust: {{ row[1] }} N
- Specific Impulse: {{ row[2] }} s
- Mass: {{ row[3] }} kg
- T/W Ratio: {{ (row[1] / (row[3] * 9.81)) | round(2) }}
{% endfor %}
```

### XLSX Multi-Sheet Access

```jinja
# Mission Summary

## Engine Selection
{% for name in spacecraft.engines.col.name %}
- {{ name }}: {{ spacecraft.engines.col.thrust_n[loop.index0] }} N
{% endfor %}

## Mission Phases
{% for phase in spacecraft.missions.col.phase_name %}
- {{ phase }}: {{ spacecraft.missions.col.delta_v[loop.index0] }} m/s
{% endfor %}
```

### Nested Data with Defaults

```jinja
{% set engine = propulsion.engine | default({}) %}
Thrust: {{ engine.thrust | default("TBD") }}
Isp: {{ engine.isp | default("TBD") }}
Mass: {{ engine.mass | default("TBD") }}
```

## Best Practices

1. **Use filters liberally** - Unit conversions and formatting should happen in templates, not in data files
2. **Extract values for calculations** - Use `value()` to get numbers for arithmetic
3. **Set defaults** - Use `default()` to handle missing data gracefully
4. **Keep templates readable** - Use whitespace and comments for complex expressions
5. **Test with real data** - Always verify template rendering with actual project data
6. **Document assumptions** - Use Jinja comments `{# ... #}` to explain non-obvious logic

## Reference

For complete Jinja2 syntax documentation, see: https://jinja.palletsprojects.com/templates/

VVERDAD-specific features (units, time, CSV, XLSX, markdown) are documented in:
- `docs/value-type-spec.md` - Complete Value type reference
- `docs/time-systems.md` - Time system details
- `docs/underscore-convention.md` - Data tree conventions
