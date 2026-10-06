use crate::{
    config::{Management, Project, Step, Variables, expand},
    generate,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::Command,
};

pub fn choose(prompt: &str, choices: &[String]) -> Result<usize> {
    ensure!(!choices.is_empty(), "no available choices for {prompt}");
    ensure!(
        io::stdin().is_terminal(),
        "{prompt}: non-interactive execution requires an explicit selection"
    );
    eprintln!("{prompt}");
    for (i, choice) in choices.iter().enumerate() {
        eprintln!("  {}. {choice}", i + 1);
    }
    loop {
        eprint!("Selection [1]: ");
        io::stderr().flush()?;
        let mut input = String::new();
        ensure!(
            io::stdin().read_line(&mut input)? > 0,
            "selection cancelled"
        );
        let input = input.trim();
        if input.is_empty() {
            return Ok(0);
        }
        if let Ok(index) = input.parse::<usize>()
            && (1..=choices.len()).contains(&index)
        {
            return Ok(index - 1);
        }
        eprintln!("Enter a number from 1 to {}.", choices.len());
    }
}

pub fn platform(project: &Project, selected: Option<String>) -> Result<String> {
    if let Some(name) = selected {
        project.platform(&name)?;
        return Ok(name);
    }
    let names: Vec<_> = project.platforms.keys().cloned().collect();
    let labels: Vec<_> = names
        .iter()
        .map(|name| {
            if name == "desktop" {
                format!("Desktop ({})", std::env::consts::OS)
            } else {
                name.clone()
            }
        })
        .collect();
    Ok(names[choose("Choose a platform", &labels)?].clone())
}

pub fn steps(project: &Project, steps: &[Step], vars: &Variables) -> Result<()> {
    steps_with_env(project, steps, vars, &std::collections::BTreeMap::new())
}

fn steps_with_env(
    project: &Project,
    steps: &[Step],
    vars: &Variables,
    env: &std::collections::BTreeMap<String, std::ffi::OsString>,
) -> Result<()> {
    for step in steps {
        let program = expand(&step.program, vars)?;
        let args: Vec<_> = step
            .args
            .iter()
            .map(|a| expand(a, vars))
            .collect::<Result<_>>()?;
        let cwd = step
            .cwd
            .as_deref()
            .map(|p| expand(p, vars))
            .transpose()?
            .map(PathBuf::from)
            .unwrap_or_else(|| project.root.clone());
        let cwd = project.root.join(cwd);
        eprintln!("Running {program} {}", args.join(" "));
        let mut command = Command::new(&program);
        command.args(&args).current_dir(&cwd);
        for (key, value) in &step.env {
            command.env(key, expand(value, vars)?);
        }
        command.envs(env);
        let status = if let Some(pattern) = &step.error_pattern {
            let output = command
                .output()
                .with_context(|| format!("starting {program} in {}", cwd.display()))?;
            io::stdout().write_all(&output.stdout)?;
            io::stderr().write_all(&output.stderr)?;
            ensure!(
                !String::from_utf8_lossy(&output.stdout).contains(pattern)
                    && !String::from_utf8_lossy(&output.stderr).contains(pattern),
                "{program} reported an error matching {pattern:?}"
            );
            output.status
        } else {
            command
                .status()
                .with_context(|| format!("starting {program} in {}", cwd.display()))?
        };
        ensure!(status.success(), "{program} failed: {status}");
    }
    Ok(())
}

pub fn build(project: &Project, name: &str, release: bool, abi: Option<&str>) -> Result<Variables> {
    let platform = project.platform(name)?;
    let mut signing_env = std::collections::BTreeMap::new();
    if name == "android" {
        signing_env.insert("GPUIFORGE_SIGNING_ENABLED".into(), "0".into());
        if release && let Some(signing) = &platform.signing {
            let keystore = signing
                .keystore
                .canonicalize()
                .context("signing keystore not found")?;
            ensure!(keystore.is_file(), "signing keystore must be a file");
            ensure!(
                !keystore.starts_with(project.directory(name)?),
                "keep the signing keystore outside the generated Android project"
            );
            signing_env.insert("GPUIFORGE_SIGNING_ENABLED".into(), "1".into());
            signing_env.insert(
                "GPUIFORGE_SIGNING_KEYSTORE".into(),
                keystore.into_os_string(),
            );
            signing_env.insert(
                "GPUIFORGE_SIGNING_KEY_ALIAS".into(),
                signing.key_alias.clone().into(),
            );
            for (destination, source) in [
                (
                    "GPUIFORGE_SIGNING_STORE_PASSWORD",
                    &signing.store_password_env,
                ),
                ("GPUIFORGE_SIGNING_KEY_PASSWORD", &signing.key_password_env),
            ] {
                let value = std::env::var_os(source)
                    .filter(|value| !value.is_empty())
                    .with_context(|| {
                        format!("set environment variable {source} for release signing")
                    })?;
                signing_env.insert(destination.into(), value);
            }
        }
    }
    if let Some(abi) = abi {
        ensure!(name == "android", "--abi is only valid for Android");
        ensure!(
            platform.abis.iter().any(|enabled| enabled == abi),
            "Android ABI {abi} is not enabled; configured ABIs: {}",
            platform.abis.join(", ")
        );
    }
    ensure!(
        !platform.build.is_empty(),
        "no build steps configured for {name}"
    );
    if platform.default_android_build && platform.management == Management::Managed {
        crate::android_tools::preflight(project, abi)?;
    }
    if (platform.bundled || platform.template.is_some())
        && platform.management == Management::Managed
    {
        generate::generate(project, name)?;
    }
    if platform.management == Management::Manual {
        ensure!(
            project.directory(name)?.is_dir(),
            "manual project directory is missing"
        );
    }
    let vars = project.variables(name, release, abi)?;
    steps_with_env(project, &platform.build, &vars, &signing_env)?;
    if let Some(artifact) = &platform.artifact {
        let artifact = project.directory(name)?.join(expand(artifact, &vars)?);
        ensure!(
            artifact.is_file(),
            "build did not produce {}",
            artifact.display()
        );
        println!("{}", artifact.display());
    }
    Ok(vars)
}

pub fn run(project: &Project, name: &str, release: bool, device: Option<&str>) -> Result<()> {
    let platform = project.platform(name)?;
    if name != "android" {
        ensure!(device.is_none(), "--device is only valid for Android");
        ensure!(
            !platform.run.is_empty(),
            "no run steps configured for {name}"
        );
        let vars = project.variables(name, release, None)?;
        return steps(project, &platform.run, &vars);
    }
    ensure!(
        !platform.run.is_empty(),
        "no run steps configured for android"
    );
    let (device, abi) = crate::devices::select(&platform.abis, device)?;
    let mut vars = build(project, name, release, Some(&abi))?;
    let artifact = platform
        .artifact
        .as_ref()
        .context("android.artifact is required to run")?;
    let artifact = project.directory(name)?.join(expand(artifact, &vars)?);
    vars.insert(
        "artifact".into(),
        artifact.to_str().context("APK path must be UTF-8")?.into(),
    );
    vars.insert("device".into(), device);
    vars.insert(
        "adb".into(),
        crate::devices::adb()
            .to_str()
            .context("ADB path must be UTF-8")?
            .into(),
    );
    steps(project, &platform.run, &vars)
}

pub fn doctor(project: &Project) -> Result<()> {
    let mut missing = false;
    for program in ["cargo", "rustc"] {
        let available = Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        println!(
            "{program}: {}",
            if available { "available" } else { "missing" }
        );
        missing |= !available;
    }
    if project.platforms.contains_key("android") {
        if project.platform("android")?.default_android_build {
            for (name, result) in crate::android_tools::inspect(project, None)? {
                match result {
                    Ok(detail) => println!("{name}: {detail}"),
                    Err(error) => {
                        println!("{name}: {error:#}");
                        missing = true;
                    }
                }
            }
        } else {
            match crate::android_tools::java_development_kit() {
                Ok(home) => println!("JDK: {} (java and javac available)", home.display()),
                Err(error) => {
                    println!("JDK: {error:#}");
                    missing = true;
                }
            }
            for name in ["ANDROID_HOME", "ANDROID_NDK_HOME"] {
                let path = std::env::var_os(name).map(PathBuf::from);
                let available = path.as_ref().is_some_and(|p| p.is_dir());
                println!(
                    "{name}: {}",
                    path.map(|p| p.display().to_string())
                        .unwrap_or_else(|| "not set".into())
                );
                missing |= !available;
            }
        }
        match crate::devices::list() {
            Ok(devices) => println!(
                "Android devices: {}",
                devices
                    .iter()
                    .map(crate::devices::Device::label)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Err(error) => {
                println!("adb: {error}");
                missing = true;
            }
        }
    }
    for (name, platform) in &project.platforms {
        println!(
            "{name}: {}",
            if platform.management == Management::Manual {
                "manual"
            } else {
                "managed"
            }
        );
        if platform.management == Management::Manual {
            missing |= !project.directory(name)?.is_dir();
        } else if let Some(template) = &platform.template {
            println!("  template: {}", template.display());
            missing |= !template.is_dir();
        }
        project.variables(name, false, None)?;
    }
    if missing {
        bail!("one or more prerequisites are missing");
    }
    Ok(())
}
