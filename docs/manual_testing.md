# Manual Testing Guide

This guide covers manual testing procedures for VVERDAD features that require human verification beyond automated tests.

## Prerequisites

```bash
# Build the project
cargo build

# Run automated tests first
cargo test

# For Docker tests (optional)
cargo test -- --ignored
```

## 1. CLI Interface

### 1.1 Basic Usage

```bash
# Run with test data
cargo run -- resources/test/phobos_sr

# Verify output was created
ls -la resources/test/phobos_sr/output/
```

**Expected**: Output directory contains `template.md` with rendered content.

### 1.2 Error Handling

```bash
# Non-existent path
cargo run -- /nonexistent/path
```

**Expected**: Error message and exit code 1.

### 1.3 Help Text

```bash
cargo run -- --help
```

**Expected**: Usage information displayed.

---

## 2. Template Rendering

### 2.1 Basic Template

```bash
cargo run -- resources/test/phobos_sr
cat resources/test/phobos_sr/output/template.md
```

**Expected output should contain**:
- `Thrust: 100 N`
- `Initial Power: 200 kW`
- `Launch Date: 12-DEC-2030`

### 2.2 Unit Conversion Filters

```bash
cat resources/test/phobos_sr/output/template_units.md
```

**Expected**: Unit conversions applied via Minijinja filters (`to`, `value`, `unit`, `si`).

### 2.3 YAML Front Matter Support

Test with project-test repository (has YAML files with markdown documentation):

```bash
cargo run -- /Users/nstrange/git/vis_viva/clawd/project-test
```

**Expected**: YAML files with `---` delimiters and trailing markdown parse correctly.

---

## 3. Analysis Bundle Discovery

### 3.1 Bundle Detection

```bash
cargo run -- resources/test/phobos_sr
ls -la resources/test/phobos_sr/output/propulsion/thermal_analysis.analysis/
```

**Expected**: Analysis bundle directory created with:
- `manifest.ron` (copied)
- `script.py` (rendered from template)
- `input.json` (rendered from template)
- `materials_db.json` (static file copied)

### 3.2 Project-Test Bundles

```bash
cargo run -- /Users/nstrange/git/vis_viva/clawd/project-test
ls -la /Users/nstrange/git/vis_viva/clawd/project-test/OUTPUT/
```

**Expected**: Two analysis bundles rendered:
- `thermal/thermal_balance.analysis/`
- `propulsion/delta_v.analysis/`

---

## 4. Docker Execution (Optional)

Requires Docker daemon running.

### 4.1 Docker Availability

```bash
# Check Docker is available
docker ps

# Run Docker integration tests
cargo test -- --ignored docker
```

### 4.2 Full Pipeline with Docker

```bash
# Ensure Docker is running
cargo run -- resources/test/phobos_sr

# Check for execution results
cat resources/test/phobos_sr/output/propulsion/thermal_analysis.analysis/thermal_results.json
```

**Expected**: Analysis script runs in container and produces output file.

### 4.3 Without Docker

```bash
# Stop Docker daemon, then run
cargo run -- resources/test/phobos_sr
```

**Expected**: Warning message about Docker unavailable, but processing completes successfully (analyses discovered and rendered, just not executed).

---

## 5. End-to-End Workflow

### 5.1 Complete Test Run

```bash
# Clean previous output
rm -rf resources/test/phobos_sr/output/

# Run full pipeline
cargo run -- resources/test/phobos_sr

# Verify all outputs
ls -laR resources/test/phobos_sr/output/
```

**Expected outputs**:
```
output/
├── template.md
├── template_units.md
└── propulsion/
    └── thermal_analysis.analysis/
        ├── manifest.ron
        ├── script.py
        ├── input.json
        └── materials_db.json
```

### 5.2 Idempotency

```bash
# Run twice
cargo run -- resources/test/phobos_sr
cargo run -- resources/test/phobos_sr

# Outputs should be identical
```

### 5.3 Project-Test Full Run

```bash
rm -rf /Users/nstrange/git/vis_viva/clawd/project-test/OUTPUT/
cargo run -- /Users/nstrange/git/vis_viva/clawd/project-test
ls -laR /Users/nstrange/git/vis_viva/clawd/project-test/OUTPUT/
```

---

## 6. Error Scenarios

### 6.1 Invalid YAML

Create a file with invalid YAML syntax and verify error handling.

### 6.2 Missing Template Variable

Create a template referencing non-existent data and verify error message.

### 6.3 Invalid Manifest

Create an analysis bundle with malformed `manifest.ron` and verify error handling.

---

## Test Data Locations

| Item | Path |
|------|------|
| Test project | `resources/test/phobos_sr/` |
| Project-test repo | `/Users/nstrange/git/vis_viva/clawd/project-test` |
| Test data files | `propulsion.yaml`, `mission.json`, `power.toml` |
| Test templates | `template.md.j2`, `template_units.md.j2` |
| Test analysis | `propulsion/thermal_analysis.analysis/` |

---

## Quick Verification Checklist

- [ ] `cargo test` passes (68+ tests)
- [ ] `cargo test -- --ignored` passes (Docker integration tests)
- [ ] `cargo run -- resources/test/phobos_sr` produces correct output
- [ ] Template variables are substituted correctly
- [ ] Analysis bundles are discovered and rendered
- [ ] Project-test repository processes without errors
- [ ] Docker execution works (if Docker available)
- [ ] Error messages are helpful for invalid inputs
