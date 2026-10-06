use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

#[derive(Parser)]
#[command(about = "Prepare an Android host library from a GPUI application package")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a library manifest without modifying the application package.
    Prepare {
        #[arg(long)]
        manifest_path: PathBuf,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Prepare {
            manifest_path,
            workspace,
            output,
        } => prepare(&manifest_path, &workspace, &output),
    }
}

fn read_manifest(path: &Path) -> Result<Table> {
    fs::read_to_string(path)?
        .parse()
        .with_context(|| format!("reading {}", path.display()))
}

fn absolute_path(table: &mut Table, key: &str, base: &Path) -> Result<()> {
    if let Some(Value::String(path)) = table.get_mut(key) {
        let resolved = base.join(&*path);
        *path = resolved
            .to_str()
            .context("manifest path must be UTF-8")?
            .to_owned();
    }
    Ok(())
}

fn dependencies(table: &mut Table, workspace: &Table, root: &Path, package: &Path) -> Result<()> {
    for section in ["dependencies", "build-dependencies", "dev-dependencies"] {
        let Some(dependencies) = table.get_mut(section).and_then(Value::as_table_mut) else {
            continue;
        };
        for (name, value) in dependencies {
            let inherited = value.get("workspace").and_then(Value::as_bool) == Some(true);
            if inherited {
                let source = workspace
                    .get("dependencies")
                    .and_then(|deps| deps.get(name))
                    .with_context(|| format!("workspace dependency {name} missing"))?;
                let mut resolved = match source {
                    Value::String(version) => {
                        Table::from_iter([("version".into(), Value::String(version.clone()))])
                    }
                    Value::Table(table) => table.clone(),
                    _ => bail!("invalid dependency {name}"),
                };
                for (key, field) in value.as_table().context("invalid inherited dependency")? {
                    if key == "workspace" {
                        continue;
                    }
                    if key == "features" {
                        let features = resolved
                            .entry(key.clone())
                            .or_insert_with(|| Value::Array(Vec::new()));
                        features.as_array_mut().context("invalid features")?.extend(
                            field
                                .as_array()
                                .context("invalid features")?
                                .iter()
                                .cloned(),
                        );
                    } else {
                        resolved.insert(key.clone(), field.clone());
                    }
                }
                *value = Value::Table(resolved);
            }
            if let Some(spec) = value.as_table_mut() {
                absolute_path(spec, "path", if inherited { root } else { package })?;
            }
        }
    }
    Ok(())
}

fn prepare(manifest_path: &Path, workspace_path: &Path, output: &Path) -> Result<()> {
    let manifest_path = manifest_path.canonicalize()?;
    let package_dir = manifest_path
        .parent()
        .context("package directory missing")?;
    let root = workspace_path.canonicalize()?;
    let root_manifest = read_manifest(&root.join("Cargo.toml"))?;
    let workspace = root_manifest
        .get("workspace")
        .and_then(Value::as_table)
        .context("workspace missing")?;
    let mut manifest = read_manifest(&manifest_path)?;
    ensure!(
        !manifest.contains_key("lib") && !package_dir.join("src/lib.rs").exists(),
        "Android packaging currently requires a binary package without a companion library"
    );
    ensure!(
        !manifest.contains_key("bin"),
        "Android packaging currently uses src/main.rs; explicit bin targets are unsupported"
    );
    ensure!(
        package_dir.join("src/main.rs").is_file(),
        "src/main.rs missing"
    );
    let package = manifest
        .get_mut("package")
        .and_then(Value::as_table_mut)
        .context("package missing")?;
    for (key, value) in package.iter_mut() {
        if value.get("workspace").and_then(Value::as_bool) == Some(true) {
            *value = workspace
                .get("package")
                .and_then(|package| package.get(key))
                .with_context(|| format!("workspace package field {key} missing"))?
                .clone();
        }
    }
    package.remove("workspace");
    for key in ["autobins", "autoexamples", "autotests", "autobenches"] {
        package.insert(key.into(), Value::Boolean(false));
    }
    for key in ["bin", "example", "test", "bench", "workspace"] {
        manifest.remove(key);
    }
    dependencies(&mut manifest, workspace, &root, package_dir)?;
    if let Some(targets) = manifest.get_mut("target").and_then(Value::as_table_mut) {
        for (_, target) in targets.iter_mut() {
            dependencies(
                target.as_table_mut().context("invalid target")?,
                workspace,
                &root,
                package_dir,
            )?;
        }
    }
    if manifest
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        manifest.insert(
            "lints".into(),
            workspace
                .get("lints")
                .context("workspace lints missing")?
                .clone(),
        );
    }
    for key in ["profile", "patch"] {
        if let Some(value) = root_manifest.get(key) {
            manifest.insert(key.into(), value.clone());
        }
    }
    if let Some(patches) = manifest.get_mut("patch").and_then(Value::as_table_mut) {
        for (_, source) in patches.iter_mut() {
            for (_, dependency) in source.as_table_mut().context("invalid patch")?.iter_mut() {
                if let Some(spec) = dependency.as_table_mut() {
                    absolute_path(spec, "path", &root)?;
                }
            }
        }
    }
    manifest.insert(
        "workspace".into(),
        Value::Table(Table::from_iter([(
            "resolver".into(),
            workspace
                .get("resolver")
                .cloned()
                .unwrap_or(Value::String("2".into())),
        )])),
    );
    manifest.insert(
        "lib".into(),
        Value::Table(Table::from_iter([
            (
                "path".into(),
                Value::String(
                    package_dir
                        .join("src/main.rs")
                        .to_str()
                        .context("source path must be UTF-8")?
                        .into(),
                ),
            ),
            (
                "crate-type".into(),
                Value::Array(vec![Value::String("cdylib".into())]),
            ),
        ])),
    );
    fs::create_dir_all(output)?;
    let output = output.canonicalize()?;
    ensure!(
        output != package_dir && output != root,
        "output must be a separate build directory"
    );
    for entry in fs::read_dir(package_dir)? {
        let entry = entry?;
        if matches!(
            entry.file_name().to_str(),
            Some("Cargo.toml" | "Cargo.lock" | "target" | ".git")
        ) {
            continue;
        }
        let dest = output.join(entry.file_name());
        if fs::symlink_metadata(&dest).is_err() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(entry.path(), dest)?;
            #[cfg(not(unix))]
            bail!("Android packaging requires a Linux or macOS build host");
        }
    }
    fs::write(output.join("Cargo.toml"), toml::to_string(&manifest)?)?;
    fs::copy(root.join("Cargo.lock"), output.join("Cargo.lock"))?;
    Ok(())
}
