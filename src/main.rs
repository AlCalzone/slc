use slc::{ParsedProject, Project, SDK};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("slc: error: {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
slc — Silicon Labs SLC project generator

Usage:
    slc generate --sdk <path> [--output <dir>] <project.slcp>

Options:
    -s, --sdk <path>       Path to the SDK .slcs file, or a directory containing one
    -o, --output <dir>     Output directory (default: the project's directory)
    -h, --help             Show this help

`generate` resolves the project's components against the SDK and writes the
autogen/ and config/ trees under the output directory.";

fn run() -> Result<(), Box<dyn Error>> {
    let mut sdk_arg: Option<String> = None;
    let mut out_arg: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-s" | "--sdk" => {
                sdk_arg = Some(args.next().ok_or("--sdk requires a value")?);
            }
            "-o" | "--output" => {
                out_arg = Some(args.next().ok_or("--output requires a value")?);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}").into());
            }
            other => positionals.push(other.to_string()),
        }
    }

    // The only supported subcommand is `generate`; accept it explicitly or
    // treat a lone project path as an implicit generate.
    let mut project_path: Option<String> = None;
    for p in positionals {
        if p == "generate" {
            continue;
        }
        if project_path.is_some() {
            return Err(format!("unexpected argument: {p}").into());
        }
        project_path = Some(p);
    }

    let project_path =
        project_path.ok_or("missing project file (.slcp)\n\n".to_string() + USAGE)?;
    let sdk_arg = sdk_arg.ok_or("missing --sdk\n\n".to_string() + USAGE)?;
    let sdk_path = resolve_sdk_path(&sdk_arg)?;

    let sdk = SDK::parse(&sdk_path)?;
    let project = Project::parse(&project_path)?;

    let out_dir = match out_arg {
        Some(o) => PathBuf::from(o),
        None => Path::new(&project_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    };

    let resolved = project.resolve_components(&sdk)?;
    let parsed = ParsedProject::new(&sdk, &project, &resolved);
    let written = parsed.generate(&out_dir)?;

    println!(
        "Resolved {} component(s); wrote {} file(s) to {}",
        resolved.components.len(),
        written.len(),
        out_dir.display()
    );
    Ok(())
}

/// Accept either a `.slcs` file directly or a directory containing exactly one.
fn resolve_sdk_path(arg: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = Path::new(arg);
    if path.is_dir() {
        let mut found = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "slcs"));
        let first = found
            .next()
            .ok_or_else(|| format!("no .slcs file in {arg}"))?;
        Ok(first)
    } else {
        Ok(path.to_path_buf())
    }
}
