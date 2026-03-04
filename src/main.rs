use clap::{Args, Parser, Subcommand};
use miette::Result;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use vverdad::config::{OutputType, validate_run_args};
use vverdad::error::format_diagnostic;
use vverdad::init::{InitConfig, generate_files, write_init_files};
use vverdad::{create_app_with_output, run_app};

#[derive(Parser, Debug)]
#[command(name = "vv", args_conflicts_with_subcommands = true)]
#[command(author, version, about = "VVERDAD — data processing engine for aerospace vehicle design")]
#[command(
    long_about = "VVERDAD — data processing engine for aerospace vehicle design.

Loads design data files (JSON, YAML, TOML, RON, CSV, XLSX, and binary formats)
from a project directory or .vv archive, makes them available via dot-notation
derived from the folder/file structure, and renders Jinja2-compatible templates
to produce reports, analysis inputs, and other engineering artifacts.

Templates can reference physical quantities with unit conversion (e.g. to(\"lbf\"))
and time epochs with system conversion (e.g. to_tdb). Analysis bundles in
.analysis/ directories execute in Docker containers, with results feeding back
into the data tree for downstream templates.

Outputs are written to _output/ inside the project by default.",
    after_long_help = "WORKFLOW:
  1. Organize design data in a project directory
  2. Write Jinja2 templates (.j2) that reference the data
  3. Run 'vv ./project' to render all templates
  4. Rendered outputs appear in _output/

CI/CD SETUP:
  Generate pipeline configs and git hooks with 'vv init':

    vv init                      Generate all CI/CD configs (GitHub, GitLab, hooks)
    vv init --github ./project   GitHub Actions workflow only
    vv init --gitlab             GitLab CI/CD pipeline only
    vv init --hooks              Git pre-commit and pre-push hooks only

  Then: review generated files, run 'git config core.hooksPath .githooks'
  if using hooks, and commit to activate.

EXAMPLES:
    vv ./my-project              Render in-place to ./my-project/_output/
    vv project.vv                Render from a .vv archive
    vv ./project -d ./release    Copy project to ./release/, render there
    vv ./project -f release.vv   Package project + outputs into an archive

DOCUMENTATION:
    See docs/ci-cd-integration.md for full CI/CD guide
    See docs/template-guide.md for template authoring reference"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    run_args: RunArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate CI/CD pipeline configs and git hooks for a VVERDAD project
    #[command(
        after_help = "GENERATED FILES:
    .github/workflows/vverdad.yml   GitHub Actions: build, render, upload artifacts
    .gitlab-ci.yml                  GitLab CI: build + render stages
    .githooks/pre-commit            Validate templates before each commit
    .githooks/pre-push              Full render validation before push

NEXT STEPS:
    1. Review and customize the generated files
    2. For git hooks: git config core.hooksPath .githooks
    3. Commit and push to activate CI/CD"
    )]
    Init(InitArgs),
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Project directory or .vv archive file
    #[arg(value_name = "INPUT")]
    project: Option<PathBuf>,

    /// Output to a directory (creates full project copy with _output inside)
    #[arg(short = 'd', long = "output-dir", value_name = "DIR")]
    output_dir: Option<PathBuf>,

    /// Output to a .vv archive file (creates archive with project + _output)
    #[arg(short = 'f', long = "output-file", value_name = "FILE")]
    output_file: Option<PathBuf>,

    /// Skip confirmation prompts (assume yes)
    #[arg(short = 'y', long = "yes")]
    yes: bool,
}

#[derive(Args, Debug)]
struct InitArgs {
    /// Target project directory (default: current directory)
    #[arg(value_name = "DIR", default_value = ".")]
    directory: PathBuf,

    /// Generate GitHub Actions workflow
    #[arg(long)]
    github: bool,

    /// Generate GitLab CI/CD pipeline
    #[arg(long)]
    gitlab: bool,

    /// Generate git hooks (pre-commit and pre-push)
    #[arg(long)]
    hooks: bool,

    /// Generate all CI/CD configurations (default when no flags specified)
    #[arg(long, short)]
    all: bool,

    /// Overwrite existing files without prompting
    #[arg(long)]
    force: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init(args)) => run_init(args),
        None => run_process(cli.run_args),
    }
}

fn run_init(args: InitArgs) -> Result<()> {
    let config = InitConfig {
        project_dir: args.directory.clone(),
        github: args.github,
        gitlab: args.gitlab,
        hooks: args.hooks,
        all: args.all,
        force: args.force,
    };

    let files = generate_files(&config);

    eprintln!(
        "VVERDAD init: generating CI/CD configuration in {}",
        args.directory.display()
    );
    eprintln!();

    match write_init_files(&args.directory, &files, args.force) {
        Ok(written) => {
            eprintln!();
            if written.is_empty() {
                eprintln!("No files were written. Use --force to overwrite existing files.");
            } else {
                eprintln!("Next steps:");
                eprintln!("  1. Review the generated files and customize as needed");

                let has_hooks = files.iter().any(|f| f.executable);
                if has_hooks {
                    eprintln!("  2. For git hooks: git config core.hooksPath .githooks");
                    eprintln!("  3. Commit and push to activate CI/CD");
                } else {
                    eprintln!("  2. Commit and push to activate CI/CD");
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", format_diagnostic(&e));
            std::process::exit(1);
        }
    }
}

fn run_process(args: RunArgs) -> Result<()> {
    // Validate CLI arguments (pure function — no I/O, no exit)
    let project_path = validate_run_args(&args.project, &args.output_dir, &args.output_file)
        .unwrap_or_else(|e| {
            eprintln!("{}", format_diagnostic(&e));
            std::process::exit(1);
        });

    // Determine output type and handle prompts
    let output_type = determine_output_type(&args)?;

    // Print processing info
    let is_archive = project_path.extension().map(|e| e == "vv").unwrap_or(false);
    if is_archive {
        println!("Processing archive: {}", project_path.display());
    } else {
        println!("Processing directory: {}", project_path.display());
    }

    match &output_type {
        OutputType::InPlace => {
            println!("Output: in-place (_output/ inside input)");
        }
        OutputType::Directory(dir) => {
            println!("Output: directory {}", dir.display());
        }
        OutputType::Archive(archive) => {
            println!("Output: archive {}", archive.display());
        }
    }

    let had_errors = {
        let mut app = match create_app_with_output(project_path, output_type) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Error: {}", format_diagnostic(&e));
                std::process::exit(1);
            }
        };
        run_app(&mut app);
        app.has_errors
    }; // App is dropped here, ensuring ZipSink is finalized

    // Exit with error code if there were errors
    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}

/// Determines the output type from CLI args, prompting for confirmation if needed
fn determine_output_type(args: &RunArgs) -> Result<OutputType> {
    if let Some(ref dir) = args.output_dir {
        // Check for input = output conflict
        if let Some(ref input) = args.project {
            if paths_conflict(input, dir) {
                eprintln!(
                    "Error: Output directory cannot be the same as or inside the input directory"
                );
                std::process::exit(1);
            }
        }

        // Check if directory exists and is not empty
        if dir.exists() && dir.is_dir() {
            let is_empty = dir
                .read_dir()
                .map(|mut d| d.next().is_none())
                .unwrap_or(true);
            if !is_empty
                && !args.yes
                && !prompt_confirm(&format!(
                    "Directory {} exists and is not empty. Overwrite?",
                    dir.display()
                ))
            {
                eprintln!("Aborted.");
                std::process::exit(1);
            }
        }

        return Ok(OutputType::Directory(dir.clone()));
    }

    if let Some(ref file) = args.output_file {
        // Check for input = output conflict
        if let Some(ref input) = args.project {
            if input == file {
                eprintln!("Error: Output file cannot be the same as the input file");
                std::process::exit(1);
            }
        }

        // Ensure .vv extension
        let file = if file.extension().map(|e| e == "vv").unwrap_or(false) {
            file.clone()
        } else {
            let mut f = file.clone();
            f.set_extension("vv");
            eprintln!(
                "Warning: Adding .vv extension to output file: {}",
                f.display()
            );
            f
        };

        // Check if file exists
        if file.exists()
            && !args.yes
            && !prompt_confirm(&format!(
                "File {} already exists. Overwrite?",
                file.display()
            ))
        {
            eprintln!("Aborted.");
            std::process::exit(1);
        }

        return Ok(OutputType::Archive(file));
    }

    // Default: in-place output
    Ok(OutputType::InPlace)
}

/// Prompts the user for confirmation, returns true if they answer yes
fn prompt_confirm(message: &str) -> bool {
    print!("{} [y/N] ", message);
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Checks if two paths conflict (one is inside the other or they're the same)
fn paths_conflict(input: &Path, output: &Path) -> bool {
    // Canonicalize paths for comparison (if they exist)
    let input_canonical = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());
    let output_canonical = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());

    // Check if they're the same
    if input_canonical == output_canonical {
        return true;
    }

    // Check if output is inside input
    output_canonical.starts_with(&input_canonical)
}
