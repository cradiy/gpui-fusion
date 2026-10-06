use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gpuiforge-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("template")).unwrap();
        fs::write(
            root.join("template/settings.txt.tmpl"),
            "{{app.name|xml}} {{var.message|kotlin}}",
        )
        .unwrap();
        fs::write(
            root.join("recipe.toml"),
            r#"
template = "template"
abis = ["x86_64"]
application-id = "dev.example.app"
[[build]]
program = "rustc"
args = ["--version"]
"#,
        )
        .unwrap();
        fs::write(
            root.join("gpuiforge.toml"),
            r#"
# Keep this comment when switching ownership.
[app]
name = 'A & B'
[platforms.android]
recipe = "recipe.toml"
[platforms.android.variables]
message = '$value'
"#,
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gpuiforge"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) {
        let result = self.run(args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bundled_android_generates_without_recipe_or_checkout() {
    let app = Fixture::new();
    fs::remove_file(app.0.join("gpuiforge.toml")).unwrap();
    fs::remove_file(app.0.join("recipe.toml")).unwrap();
    fs::remove_dir_all(app.0.join("template")).unwrap();
    fs::write(
        app.0.join("Cargo.toml"),
        "[package]\nname = 'fixture-app'\nversion = '0.1.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::create_dir(app.0.join("src")).unwrap();
    fs::write(app.0.join("src/main.rs"), "fn main() {}").unwrap();
    let binary = app
        .0
        .join(format!("gpuiforge{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_gpuiforge"), &binary).unwrap();
    for args in [
        vec!["init", "--android", "--application-id", "dev.example.app"],
        vec!["generate", "android"],
    ] {
        let result = Command::new(&binary)
            .args(args)
            .current_dir(&app.0)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let generated = app.0.join("target/gpuiforge/android");
    assert!(
        generated
            .join("host/src/main/kotlin/dev/gpui/android/GpuiView.kt")
            .is_file()
    );
    assert!(
        generated
            .join("gradle/wrapper/gradle-wrapper.jar")
            .is_file()
    );
    let activity =
        fs::read_to_string(generated.join("app/src/main/kotlin/dev/gpuiforge/app/MainActivity.kt"))
            .unwrap();
    assert!(activity.contains("fixture_app"));
    let host = generated.join("host/src/main/kotlin/dev/gpui/android/GpuiActivity.kt");
    fs::write(&host, "user-owned host").unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    app.ok(&["platform", "eject", "android"]);
    assert_eq!(
        fs::read_to_string(
            app.0
                .join("platforms/android/host/src/main/kotlin/dev/gpui/android/GpuiActivity.kt")
        )
        .unwrap(),
        "user-owned host"
    );
}

#[test]
fn edited_generated_sources_survive_eject_and_manual_build() {
    let app = Fixture::new();
    app.ok(&["generate", "android"]);
    let generated = app.0.join("target/gpuiforge/android/settings.txt");
    assert_eq!(
        fs::read_to_string(&generated).unwrap(),
        "A &amp; B \\$value"
    );
    app.ok(&["generate", "android"]);
    fs::write(&generated, "user-owned settings").unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    assert_eq!(
        fs::read_to_string(&generated).unwrap(),
        "user-owned settings"
    );
    fs::write(
        app.0.join("target/gpuiforge/android/Custom.kt"),
        "custom service",
    )
    .unwrap();
    app.ok(&["platform", "eject", "android"]);
    let config = fs::read_to_string(app.0.join("gpuiforge.toml")).unwrap();
    assert!(config.contains("management = \"manual\"") && config.contains("# Keep this comment"));
    let exported = app.0.join("platforms/android/settings.txt");
    assert_eq!(
        fs::read_to_string(&exported).unwrap(),
        "user-owned settings"
    );
    assert!(app.0.join("platforms/android/Custom.kt").is_file());
    fs::write(app.0.join("template/settings.txt.tmpl"), "upstream changes").unwrap();
    app.ok(&["build", "android"]);
    assert_eq!(
        fs::read_to_string(&exported).unwrap(),
        "user-owned settings"
    );
    assert!(!app.run(&["generate", "android"]).status.success());
    assert!(!app.run(&["platform", "eject", "android"]).status.success());
}

#[test]
fn invalid_configuration_cannot_overwrite_existing_project() {
    let app = Fixture::new();
    let generated = app.0.join("target/gpuiforge/android");
    fs::create_dir_all(&generated).unwrap();
    fs::write(generated.join("settings.txt"), "existing project").unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    assert_eq!(
        fs::read_to_string(generated.join("settings.txt")).unwrap(),
        "existing project"
    );
    let config = app.0.join("gpuiforge.toml");
    let original = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        original.replace(
            "[platforms.android]\n",
            "[platforms.android]\nproject-dir = '../outside'\n",
        ),
    )
    .unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    fs::write(
        &config,
        original.replace(
            "[platforms.android]\n",
            "[platforms.android]\nmanagment = 'manual'\n",
        ),
    )
    .unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    fs::write(&config, original).unwrap();
    assert!(!app.run(&["run"]).status.success());
}
