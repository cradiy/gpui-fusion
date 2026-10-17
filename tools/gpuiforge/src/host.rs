use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    dunce::canonicalize(path).with_context(|| format!("resolving {}", path.display()))
}

pub struct NdkTools {
    pub cc: PathBuf,
    pub cxx: PathBuf,
    pub ar: PathBuf,
}

impl NdkTools {
    pub fn new(ndk: &Path, target: &str, api: u32) -> Result<Self> {
        let os = std::env::consts::OS;
        let tag = match os {
            "linux" => "linux-x86_64",
            "macos" => "darwin-x86_64",
            "windows" => "windows-x86_64",
            _ => bail!("unsupported Android build host: {os}"),
        };
        let bin = ndk.join("toolchains/llvm/prebuilt").join(tag).join("bin");
        let wrapper = if os == "windows" { ".cmd" } else { "" };
        Ok(Self {
            cc: bin.join(format!("{target}{api}-clang{wrapper}")),
            cxx: bin.join(format!("{target}{api}-clang++{wrapper}")),
            ar: bin.join(if os == "windows" {
                "llvm-ar.exe"
            } else {
                "llvm-ar"
            }),
        })
    }

    pub fn validate(&self) -> Result<()> {
        for path in [&self.cc, &self.cxx, &self.ar] {
            ensure!(
                path.is_file(),
                "NDK tool missing: {}; check ANDROID_NDK_HOME and min_sdk",
                path.display()
            );
        }
        Ok(())
    }
}

#[cfg(unix)]
fn stage_source(source: &Path, destination: &Path) -> Result<()> {
    if fs::symlink_metadata(destination).is_err() {
        std::os::unix::fs::symlink(source, destination)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn stage_source(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "source snapshots cannot contain symlinks: {}",
        source.display()
    );
    if let Ok(existing) = fs::symlink_metadata(destination) {
        ensure!(
            !existing.file_type().is_symlink(),
            "refusing to overwrite symlink: {}",
            destination.display()
        );
        if existing.is_dir() != metadata.is_dir() {
            remove_snapshot(destination)?;
        }
    }
    if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(destination)? {
            let entry = entry?;
            match fs::symlink_metadata(source.join(entry.file_name())) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    remove_snapshot(&entry.path())?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            stage_source(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, destination)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn remove_snapshot(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "refusing to remove symlink: {}",
        path.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            remove_snapshot(&entry?.path())?;
        }
        fs::remove_dir(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn stage_package(package: &Path, output: &Path) -> Result<()> {
    #[cfg(not(unix))]
    let mut staged = std::collections::BTreeSet::new();
    for entry in fs::read_dir(package)? {
        let entry = entry?;
        if matches!(
            entry.file_name().to_str(),
            Some(
                "Cargo.toml" | "Cargo.lock" | "target" | ".git" | ".gpuiforge-source-snapshot.json"
            )
        ) {
            continue;
        }
        if output.starts_with(entry.path()) {
            continue;
        }
        let dest = output.join(entry.file_name());
        stage_source(&entry.path(), &dest)?;
        #[cfg(not(unix))]
        staged.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("source name must be UTF-8"))?,
        );
    }
    #[cfg(not(unix))]
    {
        let state = output.join(".gpuiforge-source-snapshot.json");
        if state.exists() {
            let previous: std::collections::BTreeSet<String> =
                serde_json::from_slice(&fs::read(&state)?)?;
            for name in previous.difference(&staged) {
                let mut components = Path::new(name).components();
                ensure!(
                    matches!(components.next(), Some(std::path::Component::Normal(_)))
                        && components.next().is_none(),
                    "invalid source snapshot entry: {name}"
                );
                ensure!(
                    !matches!(
                        name.as_str(),
                        "Cargo.toml"
                            | "Cargo.lock"
                            | "target"
                            | ".git"
                            | ".gpuiforge-source-snapshot.json"
                    ),
                    "invalid source snapshot entry: {name}"
                );
                let stale = output.join(name);
                if fs::symlink_metadata(&stale).is_ok() {
                    remove_snapshot(&stale)?;
                }
            }
        }
        fs::write(state, serde_json::to_vec(&staged)?)?;
    }
    Ok(())
}
