# VVERDAD

**V**is **V**iva **E**ngineering, **R**eview, **D**esign, and **A**rchitecture **D**atabase

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

This is a prototype of the VVERDAD data processing engine for aerospace vehicle design. It demonstrates a new, data-oriented approach to Model-Based Systems Engineering (MBSE) that applies Entity Component System (ECS) programming techniques from the video game industry to spacecraft design.

This prototype was developed under the NASA Small Business Innovation Research (SBIR) program, Topic S17.02: *Digital Engineering Applications for Science — Integrated Campaign and System Modeling*.

## Why VVERDAD?

The space industry faces a critical bottleneck: while launch costs have dropped significantly, spacecraft development remains expensive due to slow and inefficient design processes. Digital Engineering (DE) and Model-Based Systems Engineering (MBSE) are potential solutions, but current DE/MBSE tools face three challenges:

1. High deployment complexity and training difficulty for projects adopting DE/MBSE
2. Limited availability of discipline-focused engineering models that integrate with DE/MBSE systems
3. Limited compatibility between DE/MBSE tools and existing engineering and science tools

VVERDAD addresses these challenges by focusing on how data flows through the deisgn process.  This data focus enables VVERDAD to provide much of the functionality of traditional MBSE approaches, but in a way that integrates with exisitng analysis tools and processes.


### Data-Oriented MBSE

Traditional MBSE tools store model data inside objects alongside program logic, making maintenance, testing, and integration difficult. VVERDAD takes a different approach based on data-oriented programming principles:

| Principle | Benefit for MBSE |
|-----------|-----------------|
| **Separate data from code** | Model and analysis code is independent from design data, facilitating code reuse and reducing test complexity |
| **Generic data interface** | Standard data interfaces make it easier to expand model functionality without breaking existing relationships |
| **Immutable data structures** | Simplifies version control, concurrency, and asynchronous modeling |
| **Stateless functions** | Functions only need to be tested once — no edge cases from unexpected program state |

This architecture allows engineers to work with design data in standard formats (JSON, YAML, TOML, CSV) while VVERDAD handles integration, template rendering, and analysis execution. Engineers don't need to learn a proprietary data model — they only need to organize their data files in a way that makes sense to them.  VVERDAD's template system then allows each discipline to translate data from other teams into their own system.

### Preserving Document-Based Engineering

VVERDAD also preserves the benefits of document-based systems engineering — traceability, independent review, and forcing engineers to clarify their thinking through writing — while adding automated consistency checks and CI/CD integration. Engineering documents are generated as "views" into the design data, with plots, tables, and analysis results populated dynamically from templates.

## How It Works

VVERDAD processes a project directory containing design data files and Jinja2-compatible templates. It loads all data into a Rust-Bevy ECS database, renders templates with that data, and optionally executes analysis scripts in Docker containers. Results feed back into the database for downstream templates.

```
Engineer pushes design data to git repo
    → VVERDAD renders templates into analysis inputs
        → Analysis tools run in Docker containers
            → Results added to database
                → Templates render reports and dashboards
                    → All outputs pushed to repo for review
```

This workflow integrates with standard CI/CD pipelines (GitHub Actions, GitLab CI, git hooks) to automate the design-build-test cycle.

## Features

- **Multi-format data loading**: JSON, YAML, TOML, RON, MessagePack, CBOR, BSON, Pickle, CSV, XLSX
- **Physical quantities**: Automatic parsing of `"100 N"` or `"5.5 km/s"` with unit conversion filters
- **Time epochs**: UTC, TDB, TT, TAI time systems with conversion and leap second support
- **Template rendering**: Minijinja (Jinja2-compatible) with custom unit and time filters
- **Reactive rendering pipeline**: Templates render as their data dependencies become available
- **Analysis bundles**: Self-contained analysis packages with Docker execution
- **CI/CD scaffolding**: `vv init` generates GitHub Actions, GitLab CI, and git hook configurations
- **Data annotations**: Review comments and annotations as sidecar files alongside data
- **Archive support**: `.vv` zip archives as portable project packages

## Quick Start

```bash
# Run on a project directory
vv /path/to/project

# Run on a .vv archive
vv project.vv

# Output to a separate directory
vv /path/to/project -d /path/to/output

# Output to a .vv archive
vv /path/to/project -f output.vv

# Generate CI/CD configuration files
vv init
vv --github ./project    # GitHub Actions only
vv init --hooks ./project     # Git hooks only
```

## Project Structure

A VVERDAD project is a directory of data files and templates:

```
project/
├── propulsion/
│   ├── engine.yaml              # Design data: {{ propulsion.engine.thrust }}
│   └── thermal.analysis/        # Analysis bundle with manifest + scripts
│       ├── manifest.ron
│       └── script.py.j2
├── power/
│   └── solar_array.toml         # Design data: {{ power.solar_array.area }}
├── engines.csv                  # Tabular data: {{ engines.col.name[0] }}
├── report.md.j2                 # Template → _output/report.md
└── _output/                     # Generated outputs (merged back into data)
```

Data files are accessible in templates via dot notation derived from the directory and file structure. Templates use Jinja2 syntax with custom filters for unit conversion and time system operations.

## Template Example

```jinja
# Propulsion Summary

Thrust: {{ propulsion.engine.thrust | to("lbf") }}
Specific Impulse: {{ propulsion.engine.isp | value("s") }} seconds
Solar Array Area: {{ power.solar_array.area | si }}

Launch Date: {{ mission.launch_date | to_tdb }}

{% for r in engines.row %}
- {{ r[0] }}: {{ r[1] }} N thrust, {{ r[2] }} s Isp
{% endfor %}
```

## Applicability

VVERDAD's approach is applicable broadly to aerospace design problems:

- **Robotic science missions**: Coordinating subsystem design with science planning and instrument design
- **Human exploration missions**: Managing multi-element flight systems across NASA centers and international partners
- **Mission operations**: Transferring design knowledge to operations teams
- **Technology programs**: Making design data accessible for mission impact assessment

## Documentation

- [Template Guide](docs/template-guide.md) — Template authoring with filters and data access
- [Directory Structure](docs/directory-structure.md) — Project directory conventions
- [Value Type Specification](docs/value-type-spec.md) — The VVERDAD Value enum implementation
- [Time Systems](docs/time-systems.md) — UTC/TDB time support and conversions
- [Annotation Format](docs/annotation-format.md) — Data annotation sidecar files
- [Underscore Convention](docs/underscore-convention.md) — `_`-prefix metadata key convention
- [Data-Oriented Programming](docs/data-oriented-programming.md) — Why DOP for aerospace data
- [CI/CD Integration](docs/ci-cd-integration.md) — GitHub Actions, GitLab CI, and git hooks
- [LLM Context](docs/llm-context.md) — Quick reference for LLM-assisted usage

## Architecture

VVERDAD is built on [Bevy ECS](https://bevyengine.org/) and written in Rust. The ECS architecture was chosen for its data-oriented design, which maps naturally to engineering data management:

- **Entities** represent design data files, directories, and templates
- **Components** store data values, metadata, and processing state
- **Systems** perform data loading, template rendering, and analysis execution

The reactive rendering pipeline uses dependency tracking to render templates as their data becomes available, with cycle detection to catch circular dependencies before processing begins.

See the [docs/](docs/) directory for detailed technical documentation.

## Prototype Status

This is the Phase I SBIR prototype. It demonstrates the core data processing pipeline — data ingestion, template rendering, and containerized analysis execution. A Phase II effort would mature this into a production-ready tool with MBSE visualization, mission planning views, and expanded analysis integration.

## License

This project is licensed under the [MIT License](LICENSE).

Copyright (c) 2026 Nathan Strange
