# CI/CD Integration Guide

VVERDAD is a stateless CLI tool designed for automation. It reads input, produces output, and requires no persistent state between runs. This makes it straightforward to integrate into CI/CD pipelines and git hooks.

## Key CLI Flags for Automation

| Flag | Purpose |
|------|---------|
| `-y` | Skip confirmation prompts (essential for non-interactive environments) |
| `-d <DIR>` | Write output to a specific directory |
| `-f <FILE>` | Write output to a `.vv` archive |

## Quick Setup with `vv init`

Generate CI/CD configuration files automatically:

```bash
vv init                       # All configs in current directory
vv init --github              # GitHub Actions workflow only
vv init --gitlab              # GitLab CI/CD pipeline only
vv init --hooks               # Git pre-commit and pre-push hooks
vv init --all ./my-project    # All configs in a specific directory
vv init --force               # Overwrite existing files
```

Generated files:
- `.github/workflows/vverdad.yml` — GitHub Actions workflow
- `.gitlab-ci.yml` — GitLab CI/CD pipeline
- `.githooks/pre-commit` — Validates templates before commits
- `.githooks/pre-push` — Full render validation before push

After generating, review and customize the files, then:
1. For git hooks: `git config core.hooksPath .githooks`
2. Commit and push to activate CI/CD

The sections below describe what each generated file does and how to customize it.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (including when Docker is unavailable — analysis bundles are skipped with a warning) |
| 1 | Fatal error (missing input, parse failure, template error) |

Docker is optional. If unavailable, analysis bundles are discovered and rendered but not executed. The process still exits 0.

## Installation in CI

### From Git

Install `vv` directly from the repository:

```bash
cargo install --git https://github.com/VisVivaSpace/vverdad-prototype.git
```

The `vv init` generated workflows use this approach with binary caching so the install only runs when the cache is cold.

### Caching

Cache the installed binary between CI runs. The generated GitHub Actions workflow caches `~/.cargo/bin/vv` so subsequent runs skip the install step entirely. For GitLab CI, the `$CARGO_HOME/bin/` directory is cached.

For a cold cache, `cargo install` also benefits from caching these directories:

- `~/.cargo/registry` — downloaded crates
- `~/.cargo/git` — git-based dependencies

### Docker-in-Docker

If your pipeline needs analysis bundle execution, the CI runner must have Docker access. Options:

- **Docker socket mount**: Bind `/var/run/docker.sock` into the CI runner
- **Docker-in-Docker (DinD)**: Run a Docker daemon inside the CI container
- **Skip Docker**: If only template rendering is needed, Docker is not required

## GitHub Actions

> **Quick start**: Run `vv init --github` to generate this file automatically.

Complete workflow that installs `vv` from git and renders a project:

```yaml
# .github/workflows/vverdad.yml
name: VVERDAD Render

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  render:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache vv binary
        id: cache-vv
        uses: actions/cache@v4
        with:
          path: ~/.cargo/bin/vv
          key: ${{ runner.os }}-vv-${{ hashFiles('.github/workflows/vverdad.yml') }}

      - name: Install vv
        if: steps.cache-vv.outputs.cache-hit != 'true'
        run: cargo install --git https://github.com/VisVivaSpace/vverdad-prototype.git

      - name: Render project
        run: vv . -d artifacts/ -y

      - name: Upload outputs
        uses: actions/upload-artifact@v4
        with:
          name: vverdad-output
          path: artifacts/_output/

      - name: Commit outputs
        if: github.event_name == 'push'
        run: |
          git config user.name "VVERDAD CI"
          git config user.email "vverdad@ci"
          cp -r artifacts/_output/ _output/
          git add _output/
          git diff --cached --quiet || git commit -m "Update rendered outputs"
          git push
```

The binary is cached between runs, so the install step only runs on cache miss. To force a rebuild (e.g. after a new vv release), change the workflow file or clear the cache.

Rendered outputs are committed back to the repository on push (not on pull requests, to avoid polluting PR branches). The `git diff --cached --quiet` check prevents empty commits when outputs haven't changed.

GitHub-hosted `ubuntu-latest` runners include Docker by default, so analysis bundles execute without additional setup.

Reference: [GitHub Actions documentation](https://docs.github.com/en/actions)

## GitLab CI/CD

> **Quick start**: Run `vv init --gitlab` to generate this file automatically.

Complete pipeline that installs `vv` from git and renders a project:

```yaml
# .gitlab-ci.yml
image: rust:latest

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo

cache:
  paths:
    - $CARGO_HOME/bin/
    - $CARGO_HOME/registry/
    - $CARGO_HOME/git/

stages:
  - install
  - render

install:
  stage: install
  script:
    - cargo install --git https://github.com/VisVivaSpace/vverdad-prototype.git
  artifacts:
    paths:
      - $CARGO_HOME/bin/vv

render:
  stage: render
  script:
    - $CARGO_HOME/bin/vv . -d output/ -y
  artifacts:
    paths:
      - output/_output/
  after_script:
    - git config user.name "VVERDAD CI"
    - git config user.email "vverdad@ci"
    - cp -r output/_output/ _output/
    - git add _output/
    - git diff --cached --quiet || git commit -m "Update rendered outputs"
    - git push
```

### With Docker Execution

To run analysis bundles in GitLab CI, add a Docker-in-Docker stage:

```yaml
execute:
  stage: execute
  image: docker:latest
  services:
    - docker:dind
  variables:
    DOCKER_HOST: tcp://docker:2375
  before_script:
    - apk add --no-cache cargo
    - cargo install --git https://github.com/VisVivaSpace/vverdad-prototype.git
  script:
    - $CARGO_HOME/bin/vv . -d output/ -y
  artifacts:
    paths:
      - output/_output/
```

Reference: [GitLab CI/CD documentation](https://docs.gitlab.com/ci/)

## Git Hooks

> **Quick start**: Run `vv init --hooks` to generate both hooks automatically.

Use git hooks to validate templates before committing or pushing.

### pre-commit: Validate Templates

Run VVERDAD to catch template errors before they reach the repository:

```bash
#!/bin/sh
# .githooks/pre-commit

# Render to a temporary directory
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! vv project/ -d "$TMPDIR" -y 2>&1; then
    echo "VVERDAD: Template rendering failed. Fix errors before committing."
    exit 1
fi

echo "VVERDAD: Templates render successfully."
```

### pre-push: Full Render and Validation

Run a full render (optionally with analysis execution) before pushing:

```bash
#!/bin/sh
# .githooks/pre-push

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! vv project/ -d "$TMPDIR" -y 2>&1; then
    echo "VVERDAD: Render failed. Fix errors before pushing."
    exit 1
fi

echo "VVERDAD: All checks passed."
```

### Installing Hooks

Point git to your hooks directory:

```bash
git config core.hooksPath .githooks
```

Make hooks executable:

```bash
chmod +x .githooks/pre-commit .githooks/pre-push
```

### pre-commit Framework Integration

If your project uses the [pre-commit framework](https://pre-commit.com), add a local hook:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: vverdad-render
        name: VVERDAD template validation
        entry: bash -c 'TMPDIR=$(mktemp -d) && trap "rm -rf $TMPDIR" EXIT && vv project/ -d "$TMPDIR" -y'
        language: system
        pass_filenames: false
        always_run: true
```

Reference: [Git hooks documentation](https://git-scm.com/docs/githooks)

## Common Patterns

### Artifact Archiving

Create a distributable `.vv` archive containing both source data and rendered outputs:

```bash
vv project/ -f release-v1.2.0.vv -y
```

The archive is a zip file that can be shared, stored, or loaded by another VVERDAD run.

### Diff-Based Validation

Compare rendered outputs between branches to review changes in generated artifacts:

```bash
# Render the base branch
git stash
vv project/ -d /tmp/base-output -y
git stash pop

# Render the current branch
vv project/ -d /tmp/current-output -y

# Compare outputs
diff -r /tmp/base-output/_output /tmp/current-output/_output
```

### Multi-Project Pipelines

Process multiple project directories in a single pipeline:

```bash
for project in projects/*/; do
    vv "$project" -d "output/$(basename "$project")" -y
done
```

### Docker-Optional Pipelines

Split rendering (no Docker required) from execution (Docker required) into separate stages. The render stage always succeeds. The execute stage is optional or can run on a different runner class:

```yaml
# GitHub Actions example
jobs:
  render:
    runs-on: ubuntu-latest
    steps:
      - run: vv project/ -d artifacts/ -y
      # Docker warnings are non-fatal; exit code is still 0

  execute:
    needs: render
    runs-on: ubuntu-latest  # Has Docker by default
    if: github.ref == 'refs/heads/main'  # Only execute on main
    steps:
      - run: vv project/ -d artifacts/ -y
```

## Environment Variables and Configuration

VVERDAD requires no environment variables. All configuration is provided via CLI flags.

The only external dependency is Docker, which is optional and detected automatically via the Docker socket at `/var/run/docker.sock`.

### Security Considerations

**Pickle files**: The Python pickle format can execute arbitrary code during deserialization. If your CI pipeline processes untrusted project archives (e.g., from external pull requests), be aware that `.pickle` and `.pkl` files in the project could compromise the CI runner. Consider:

- Reviewing project archives before processing
- Running in a sandboxed environment
- Removing pickle support if processing untrusted data

## Troubleshooting

### "Docker daemon not available"

This warning is expected in CI runners without Docker. It is non-fatal — template rendering completes normally, and the process exits 0. Analysis bundles are skipped.

### Stale `_output/` Directory

Previous run outputs in `_output/` are merged back into the data tree on the next run. In CI, always use `-d` to write to a clean output directory, or delete `_output/` before re-running:

```bash
rm -rf project/_output/
vv project/ -y
```

### Pipeline Hangs on Prompt

VVERDAD prompts for confirmation when the output destination already exists. In CI, always pass `-y` to skip prompts:

```bash
vv project/ -d output/ -y  # Never hangs
```

### Permission Errors

Ensure the CI runner has write access to the output directory. When using `-d`, the directory is created if it does not exist, but the parent directory must be writable.
