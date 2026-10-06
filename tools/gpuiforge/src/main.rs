mod android;
mod config;
mod execute;
mod generate;
include!(concat!(env!("OUT_DIR"), "/android_files.rs"));

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use config::Project;
use std::{fs, io::Write, path::PathBuf};

#[derive(Parser)]
#[command(version, about = "Generate, build and run GPUI applications")]
struct Cli {
    #[arg(long, global = true, default_value = "gpuiforge.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Create configuration for an existing Cargo application.
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        android_recipe: Option<PathBuf>,
        /// Configure Android using the bundled template.
        #[arg(long, conflicts_with = "android_recipe")]
        android: bool,
        #[arg(long)]
        application_id: Option<String>,
    },
    /// Generate a managed platform project.
    Generate { platform: String },
    /// Build a configured platform (prompts when omitted).
    Build {
        platform: Option<String>,
        #[arg(long)]
        release: bool,
    },
    /// Build and run on a platform and device (prompts when omitted).
    Run {
        platform: Option<String>,
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        release: bool,
    },
    /// Check local build tools and configured project paths.
    Doctor,
    #[command(hide = true)]
    PrepareAndroid {
        #[arg(long)]
        manifest_path: PathBuf,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Manage native project ownership.
    Platform {
        #[command(subcommand)]
        command: PlatformAction,
    },
}

#[derive(Subcommand)]
enum PlatformAction {
    /// Export generated sources and switch configuration to manual management.
    Eject {
        platform: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    if let Action::PrepareAndroid {
        manifest_path,
        workspace,
        output,
    } = cli.command
    {
        return android::prepare(&manifest_path, &workspace, &output);
    }
    if let Action::Init {
        name,
        android_recipe,
        android,
        application_id,
    } = cli.command
    {
        return init(&cli.config, name, android_recipe, android, application_id);
    }
    let project = Project::load(&cli.config)?;
    match cli.command {
        Action::Generate { platform } => {
            println!("{}", generate::generate(&project, &platform)?.display())
        }
        Action::Build { platform, release } => {
            let name = execute::platform(&project, platform)?;
            execute::build(&project, &name, release, None)?;
        }
        Action::Run {
            platform,
            device,
            release,
        } => {
            let name = execute::platform(&project, platform)?;
            execute::run(&project, &name, release, device.as_deref())?;
        }
        Action::Doctor => execute::doctor(&project)?,
        Action::Platform {
            command: PlatformAction::Eject { platform, output },
        } => {
            let destination =
                output.unwrap_or_else(|| PathBuf::from(format!("platforms/{platform}")));
            println!(
                "{}",
                generate::eject(&project, &platform, &destination)?.display()
            );
        }
        Action::Init { .. } | Action::PrepareAndroid { .. } => unreachable!(),
    }
    Ok(())
}

fn init(
    path: &std::path::Path,
    name: Option<String>,
    recipe: Option<PathBuf>,
    android: bool,
    id: Option<String>,
) -> Result<()> {
    ensure!(
        !path.exists(),
        "configuration already exists: {}",
        path.display()
    );
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let manifest: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("Cargo.toml"))
            .context("init requires an existing Cargo application")?,
    )?;
    let package = manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .context("Cargo.toml must describe an application package")?;
    let mut doc = toml_edit::DocumentMut::new();
    doc["app"]["name"] = toml_edit::value(name.unwrap_or_else(|| package.to_owned()));
    for (kind, command) in [("build", "build"), ("run", "run")] {
        let mut step = toml_edit::Table::new();
        step["program"] = toml_edit::value("cargo");
        step["args"] = toml_edit::value(
            [command, "-p", package, "--profile", "{{cargo_profile}}"]
                .into_iter()
                .collect::<toml_edit::Array>(),
        );
        let mut steps = toml_edit::ArrayOfTables::new();
        steps.push(step);
        doc["platforms"]["desktop"][kind] = toml_edit::Item::ArrayOfTables(steps);
    }
    let android = android || recipe.is_some();
    ensure!(
        android || id.is_none(),
        "--application-id requires --android or --android-recipe"
    );
    if android {
        let id = id.context("--application-id is required for Android")?;
        if let Some(recipe) = recipe {
            doc["platforms"]["android"]["recipe"] = toml_edit::value(
                recipe
                    .canonicalize()?
                    .to_str()
                    .context("recipe path must be UTF-8")?,
            );
        }
        doc["platforms"]["android"]["application-id"] = toml_edit::value(id);
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(doc.to_string().as_bytes())?;
    println!("{}", path.display());
    Ok(())
}
