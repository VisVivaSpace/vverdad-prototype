//! `vv init` — Generate CI/CD configuration files for a VVERDAD project.
//!
//! Data structures are pure data (DOP), logic is in standalone functions,
//! and I/O is isolated in `write_init_files`.

use std::path::{Path, PathBuf};

use crate::error::VVError;

// =============================================================================
// Data Structures (pure data, no methods)
// =============================================================================

/// Configuration for what to generate.
pub struct InitConfig {
    pub project_dir: PathBuf,
    pub github: bool,
    pub gitlab: bool,
    pub hooks: bool,
    pub all: bool,
    pub force: bool,
}

/// A file to be written, produced by the pure `generate_files` function.
pub struct GeneratedFile {
    pub relative_path: PathBuf,
    pub content: String,
    pub executable: bool,
}

// =============================================================================
// Embedded Templates
// =============================================================================

const GITHUB_ACTIONS: &str = r#"# .github/workflows/vverdad.yml
name: VVERDAD Render

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

permissions:
  contents: write

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
          git add -f _output/
          git diff --cached --quiet || git commit -m "Update rendered outputs"
          git push
"#;

const GITLAB_CI: &str = r#"# .gitlab-ci.yml
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
    - git add -f _output/
    - git diff --cached --quiet || git commit -m "Update rendered outputs"
    - git push
"#;

const PRE_COMMIT_HOOK: &str = r#"#!/bin/sh
# .githooks/pre-commit
# Render to a temporary directory to catch template errors before committing.

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! vv . -d "$TMPDIR" -y 2>&1; then
    echo "VVERDAD: Template rendering failed. Fix errors before committing."
    exit 1
fi

echo "VVERDAD: Templates render successfully."
"#;

const PRE_PUSH_HOOK: &str = r#"#!/bin/sh
# .githooks/pre-push
# Full render and validation before pushing.

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! vv . -d "$TMPDIR" -y 2>&1; then
    echo "VVERDAD: Render failed. Fix errors before pushing."
    exit 1
fi

echo "VVERDAD: All checks passed."
"#;

// =============================================================================
// Pure Functions
// =============================================================================

/// Produces the list of files to write based on config flags. No I/O.
pub fn generate_files(config: &InitConfig) -> Vec<GeneratedFile> {
    let all = config.all || (!config.github && !config.gitlab && !config.hooks);

    let mut files = Vec::new();

    if config.github || all {
        files.push(GeneratedFile {
            relative_path: PathBuf::from(".github/workflows/vverdad.yml"),
            content: GITHUB_ACTIONS.to_string(),
            executable: false,
        });
    }

    if config.gitlab || all {
        files.push(GeneratedFile {
            relative_path: PathBuf::from(".gitlab-ci.yml"),
            content: GITLAB_CI.to_string(),
            executable: false,
        });
    }

    if config.hooks || all {
        files.push(GeneratedFile {
            relative_path: PathBuf::from(".githooks/pre-commit"),
            content: PRE_COMMIT_HOOK.to_string(),
            executable: true,
        });
        files.push(GeneratedFile {
            relative_path: PathBuf::from(".githooks/pre-push"),
            content: PRE_PUSH_HOOK.to_string(),
            executable: true,
        });
    }

    files
}

// =============================================================================
// I/O Functions
// =============================================================================

/// Writes generated files to the project directory.
///
/// - Creates parent directories as needed.
/// - Skips existing files unless `force` is true.
/// - Sets executable permission on hooks (unix only).
/// - Returns the list of paths actually written.
pub fn write_init_files(
    project_dir: &Path,
    files: &[GeneratedFile],
    force: bool,
) -> Result<Vec<PathBuf>, VVError> {
    if !project_dir.is_dir() {
        return Err(VVError::InitDirectoryNotFound(project_dir.to_path_buf()));
    }

    let mut written = Vec::new();

    for file in files {
        let dest = project_dir.join(&file.relative_path);

        if dest.exists() && !force {
            eprintln!(
                "  Skipped: {} (already exists, use --force to overwrite)",
                file.relative_path.display()
            );
            continue;
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&dest, &file.content)?;

        #[cfg(unix)]
        if file.executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }

        eprintln!("  Created: {}", file.relative_path.display());
        written.push(dest);
    }

    Ok(written)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(github: bool, gitlab: bool, hooks: bool) -> InitConfig {
        InitConfig {
            project_dir: PathBuf::from("."),
            github,
            gitlab,
            hooks,
            all: false,
            force: false,
        }
    }

    #[test]
    fn test_generate_github_only() {
        let files = generate_files(&config_with(true, false, false));
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].relative_path,
            PathBuf::from(".github/workflows/vverdad.yml")
        );
        assert!(!files[0].executable);
    }

    #[test]
    fn test_generate_gitlab_only() {
        let files = generate_files(&config_with(false, true, false));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from(".gitlab-ci.yml"));
        assert!(!files[0].executable);
    }

    #[test]
    fn test_generate_hooks_only() {
        let files = generate_files(&config_with(false, false, true));
        assert_eq!(files.len(), 2);
        assert_eq!(
            files[0].relative_path,
            PathBuf::from(".githooks/pre-commit")
        );
        assert_eq!(files[1].relative_path, PathBuf::from(".githooks/pre-push"));
        assert!(files[0].executable);
        assert!(files[1].executable);
    }

    #[test]
    fn test_generate_all() {
        let files = generate_files(&config_with(true, true, true));
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn test_default_all_when_no_flags() {
        let files = generate_files(&config_with(false, false, false));
        assert_eq!(files.len(), 4, "No flags should produce all 4 files");
    }

    #[test]
    fn test_write_creates_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: tmp.path().to_path_buf(),
            github: true,
            gitlab: false,
            hooks: false,
            all: false,
            force: false,
        };
        let files = generate_files(&config);
        let written = write_init_files(tmp.path(), &files, false).unwrap();

        assert_eq!(written.len(), 1);
        assert!(written[0].exists());

        let content = std::fs::read_to_string(&written[0]).unwrap();
        assert!(content.contains("VVERDAD Render"));
    }

    #[test]
    fn test_write_skips_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: tmp.path().to_path_buf(),
            github: true,
            gitlab: false,
            hooks: false,
            all: false,
            force: false,
        };
        let files = generate_files(&config);

        // First write
        write_init_files(tmp.path(), &files, false).unwrap();

        // Modify the file so we can verify it's preserved
        let dest = tmp.path().join(".github/workflows/vverdad.yml");
        std::fs::write(&dest, "original content").unwrap();

        // Second write without force — should skip
        let written = write_init_files(tmp.path(), &files, false).unwrap();
        assert!(written.is_empty(), "Should skip existing files");

        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "original content", "Original should be preserved");
    }

    #[test]
    fn test_write_force_overwrites() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: tmp.path().to_path_buf(),
            github: true,
            gitlab: false,
            hooks: false,
            all: false,
            force: true,
        };
        let files = generate_files(&config);

        // First write
        write_init_files(tmp.path(), &files, false).unwrap();

        // Modify the file
        let dest = tmp.path().join(".github/workflows/vverdad.yml");
        std::fs::write(&dest, "original content").unwrap();

        // Second write with force — should overwrite
        let written = write_init_files(tmp.path(), &files, true).unwrap();
        assert_eq!(written.len(), 1, "Should overwrite with force");

        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(
            content.contains("VVERDAD Render"),
            "Should be overwritten with template content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_hooks_executable() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let config = InitConfig {
            project_dir: tmp.path().to_path_buf(),
            github: false,
            gitlab: false,
            hooks: true,
            all: false,
            force: false,
        };
        let files = generate_files(&config);
        let written = write_init_files(tmp.path(), &files, false).unwrap();

        assert_eq!(written.len(), 2);
        for path in &written {
            let perms = std::fs::metadata(path).unwrap().permissions();
            let mode = perms.mode();
            assert!(
                mode & 0o111 != 0,
                "Hook {} should be executable, mode: {:o}",
                path.display(),
                mode
            );
        }
    }

    #[test]
    fn test_nonexistent_directory_error() {
        let files = generate_files(&config_with(true, false, false));
        let result = write_init_files(Path::new("/nonexistent/path"), &files, false);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), VVError::InitDirectoryNotFound(_)),
            "Should return InitDirectoryNotFound"
        );
    }
}
