use crate::config::Project;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn sdk() -> Result<String> {
    let path = std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
        .context("set ANDROID_HOME to an Android SDK installation")?;
    let installed = fs::read_dir(path.join("platforms"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(version) = name.strip_prefix("android-") else {
                return false;
            };
            let mut parts = version.split('.');
            let major = parts.next().and_then(|part| part.parse::<u32>().ok());
            let minor = parts.next().unwrap_or("0").parse::<u32>().ok();
            matches!((major, minor), (Some(major), Some(minor)) if (major, minor) >= (36, 1))
                && parts.next().is_none()
                && entry.path().join("android.jar").is_file()
        });
    ensure!(
        installed,
        "Android SDK platform 36.1 or later is missing in {}; install it with Android Studio's SDK Manager",
        path.display()
    );
    Ok(path.display().to_string())
}

pub fn ndk(root: &Path, target: &str, api: u32) -> Result<String> {
    let path = std::env::var_os("ANDROID_NDK_HOME")
        .map(PathBuf::from)
        .context("set ANDROID_NDK_HOME to a complete Android NDK installation")?;
    let tools = crate::host::NdkTools::new(&path, target, api)?;
    tools.validate()?;
    let path = crate::host::canonicalize(&path)?;
    let compiler = crate::host::NdkTools::new(&path, target, api)?.cc;
    let output = Command::new(&compiler)
        .arg("--version")
        .current_dir(root)
        .output()
        .with_context(|| format!("starting NDK compiler {}", compiler.display()))?;
    ensure!(
        output.status.success(),
        "NDK compiler failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(path.display().to_string())
}

pub fn rust_target(root: &Path, target: &str) -> Result<String> {
    let output = Command::new("rustc")
        .args(["--print", "target-libdir", "--target", target])
        .current_dir(root)
        .output()
        .context("starting rustc")?;
    ensure!(
        output.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let directory = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let installed = fs::read_dir(&directory).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libstd-") && name.ends_with(".rlib")
        })
    });
    ensure!(
        installed,
        "Rust standard library missing; run `rustup target add {target}` for the application's toolchain"
    );
    Ok(directory.display().to_string())
}

pub fn inspect(
    project: &Project,
    selected_abi: Option<&str>,
) -> Result<Vec<(String, Result<String>)>> {
    let platform = project.platform("android")?;
    let api: u32 = platform
        .variables
        .get("min_sdk")
        .context("missing min_sdk")?
        .parse()
        .context("min_sdk must be an integer")?;
    ensure!(api >= 26, "min_sdk must be at least 26");
    let mut checks = vec![
        (
            "JDK".into(),
            java_development_kit().map(|path| path.display().to_string()),
        ),
        ("Android SDK".into(), sdk()),
    ];
    let abis: Vec<&str> = selected_abi
        .map(|abi| vec![abi])
        .unwrap_or_else(|| platform.abis.iter().map(String::as_str).collect());
    for abi in abis {
        let target = target(abi)?;
        checks.push((format!("NDK ({abi})"), ndk(&project.root, target, api)));
        checks.push((format!("Rust ({abi})"), rust_target(&project.root, target)));
    }
    Ok(checks)
}

pub fn preflight(project: &Project, selected_abi: Option<&str>) -> Result<()> {
    let failures: Vec<_> = inspect(project, selected_abi)?
        .into_iter()
        .filter_map(|(name, result)| result.err().map(|error| format!("{name}: {error:#}")))
        .collect();
    ensure!(
        failures.is_empty(),
        "Android prerequisites are missing:\n{}",
        failures.join("\n")
    );
    Ok(())
}

pub fn java_development_kit() -> Result<PathBuf> {
    use crate::host::executable;
    let configured_home = std::env::var_os("JAVA_HOME").map(PathBuf::from);
    let java = configured_home
        .as_ref()
        .map(|home| home.join("bin").join(executable("java")))
        .unwrap_or_else(|| PathBuf::from(executable("java")));
    let output = Command::new(&java)
        .args(["-XshowSettings:properties", "-version"])
        .output()
        .with_context(|| {
            format!(
                "starting {}; set JAVA_HOME to a complete JDK",
                java.display()
            )
        })?;
    ensure!(
        output.status.success(),
        "{} failed: {}",
        java.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let home = match configured_home {
        Some(home) => home,
        None => String::from_utf8_lossy(&output.stderr)
            .lines()
            .find_map(|line| line.trim().strip_prefix("java.home = ").map(PathBuf::from))
            .context("could not determine java.home; set JAVA_HOME to a complete JDK")?,
    };
    let javac = home.join("bin").join(executable("javac"));
    let output = Command::new(&javac)
        .arg("-version")
        .output()
        .with_context(|| {
            format!(
                "{} is unavailable; set JAVA_HOME to a complete JDK containing bin/javac",
                javac.display()
            )
        })?;
    ensure!(
        output.status.success(),
        "{} failed: {}; select a complete JDK with JAVA_HOME",
        javac.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    crate::host::canonicalize(&home)
}

pub fn target(abi: &str) -> Result<&'static str> {
    match abi {
        "arm64-v8a" => Ok("aarch64-linux-android"),
        "x86_64" => Ok("x86_64-linux-android"),
        _ => anyhow::bail!("unsupported Android ABI: {abi}"),
    }
}
