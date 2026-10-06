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
    ensure!(
        path.join("platforms/android-36.1/android.jar").is_file(),
        "Android SDK platform 36.1 is missing in {}; install it with Android Studio's SDK Manager",
        path.display()
    );
    Ok(path.display().to_string())
}

fn ndk(root: &Path, target: &str, api: u32) -> Result<String> {
    let path = std::env::var_os("ANDROID_NDK_HOME")
        .map(PathBuf::from)
        .context("set ANDROID_NDK_HOME to a complete Android NDK installation")?;
    let host = match std::env::consts::OS {
        "linux" => "linux-x86_64",
        "macos" => "darwin-x86_64",
        other => {
            anyhow::bail!("bundled Android builds require Linux or macOS; current host is {other}")
        }
    };
    let tools = path.join("toolchains/llvm/prebuilt").join(host).join("bin");
    for name in [
        format!("{target}{api}-clang"),
        format!("{target}{api}-clang++"),
        "llvm-ar".into(),
    ] {
        ensure!(
            tools.join(&name).is_file(),
            "NDK tool missing: {}; check ANDROID_NDK_HOME and min_sdk",
            tools.join(name).display()
        );
    }
    let compiler = tools.join(format!("{target}{api}-clang"));
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

fn rust_target(root: &Path, target: &str) -> Result<String> {
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
        let target = match abi {
            "arm64-v8a" => "aarch64-linux-android",
            "x86_64" => "x86_64-linux-android",
            _ => anyhow::bail!("unsupported Android ABI: {abi}"),
        };
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
    let executable = |name: &str| {
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    };
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
    Ok(home)
}
