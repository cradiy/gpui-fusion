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
            root.join("recipe.json"),
            r#"{
  "abis": [
    "x86_64"
  ],
  "application-id": "dev.example.app",
  "build": [
    {
      "args": [
        "--version"
      ],
      "program": "rustc"
    }
  ],
  "template": "template"
}"#,
        )
        .unwrap();
        fs::write(
            root.join("gpuiforge.json"),
            r#"{
  "app": {
    "name": "A & B"
  },
  "platforms": {
    "android": {
      "recipe": "recipe.json",
      "variables": {
        "message": "$value"
      }
    }
  }
}"#,
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
    fs::remove_file(app.0.join("gpuiforge.json")).unwrap();
    fs::remove_file(app.0.join("recipe.json")).unwrap();
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
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(app.0.join("gpuiforge.json")).unwrap()).unwrap();
    assert_eq!(config["$schema"], "./gpuiforge.schema.json");
    assert!(app.0.join("gpuiforge.schema.json").is_file());
    assert!(
        config["platforms"]["android"]["build"]
            .as_array()
            .is_some_and(|steps| !steps.is_empty())
    );
    assert!(
        config["platforms"]["android"]["run"]
            .as_array()
            .is_some_and(|steps| !steps.is_empty())
    );
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
fn sync_prunes_disabled_modules_and_icons() {
    let app = Fixture::new();
    fs::write(
        app.0.join("Cargo.toml"),
        "[package]\nname = 'fixture-app'\nversion = '0.1.0'\n",
    )
    .unwrap();
    fs::write(app.0.join("icon.xml"), "<vector />").unwrap();
    let config = app.0.join("gpuiforge.json");
    fs::write(
        &config,
        r#"{
  "app": {
    "name": "Example"
  },
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "features": [
        "sharing",
        "notifications",
        "data-sync",
        "background-media"
      ],
      "permissions": ["android.permission.FOREGROUND_SERVICE", "android.permission.FOREGROUND_SERVICE_DATA_SYNC", "android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK"],
      "icon": "icon.xml",
      "notification-icon": "icon.xml"
    }
  }
}"#,
    )
    .unwrap();
    let output = app.0.join("target/gpuiforge/android");
    let check = app.run(&["sync", "--check"]);
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stderr).contains("create host/"));
    assert!(!output.exists());
    app.ok(&["sync"]);
    let state = output.join(".gpuiforge-generated.json");
    let modified = fs::metadata(&state).unwrap().modified().unwrap();
    app.ok(&["sync", "--check"]);
    assert_eq!(fs::metadata(&state).unwrap().modified().unwrap(), modified);
    let host = output.join("host/src/main/kotlin/dev/gpui/android");
    assert!(host.join("FileStore.kt").exists());
    assert!(host.join("NotificationStore.kt").exists());
    assert!(host.join("DataSyncService.kt").exists());
    assert!(host.join("MediaPlaybackService.kt").exists());
    assert!(host.join("MediaNotification.kt").exists());
    assert!(
        fs::read_to_string(output.join("host/src/main/AndroidManifest.xml"))
            .unwrap()
            .contains("android:foregroundServiceType=\"dataSync\"")
    );
    assert!(!host.join("MediaSession.kt").exists());
    assert!(!host.join("MediaTracks.kt").exists());
    assert!(!host.join("PictureInPictureHost.kt").exists());
    assert!(
        !fs::read_to_string(output.join("app/src/main/AndroidManifest.xml"))
            .unwrap()
            .contains("supportsPictureInPicture")
    );
    assert!(
        !fs::read_to_string(output.join("host/build.gradle.kts"))
            .unwrap()
            .contains("media3")
    );
    assert!(
        output
            .join("app/src/main/res/drawable-nodpi/gpui_notification_icon.xml")
            .exists()
    );
    assert!(
        fs::read_to_string(output.join("app/src/main/AndroidManifest.xml"))
            .unwrap()
            .contains("@drawable/gpui_app_icon")
    );
    fs::write(
        &config,
        r#"{
  "app": {
    "name": "Example"
  },
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "features": []
    }
  }
}"#,
    )
    .unwrap();
    let check = app.run(&["sync", "android", "--check"]);
    assert!(!check.status.success());
    assert!(
        String::from_utf8_lossy(&check.stderr)
            .contains("remove host/src/main/kotlin/dev/gpui/android/NotificationStore.kt")
    );
    assert!(host.join("NotificationStore.kt").exists());
    assert_eq!(fs::metadata(&state).unwrap().modified().unwrap(), modified);
    app.ok(&["sync", "android"]);
    app.ok(&["sync", "--check"]);
    for file in [
        "FileStore.kt",
        "NotificationStore.kt",
        "ShareIntent.kt",
        "DataSyncService.kt",
        "MediaPlaybackService.kt",
        "MediaNotification.kt",
    ] {
        assert!(!host.join(file).exists());
    }
    let manifest = fs::read_to_string(output.join("host/src/main/AndroidManifest.xml")).unwrap();
    assert!(!manifest.contains("<receiver"));
    assert!(!manifest.contains("<provider"));
    assert!(!manifest.contains("<service"));
    assert!(
        !output
            .join("app/src/main/res/drawable-nodpi/gpui_notification_icon.xml")
            .exists()
    );
    assert!(
        !output
            .join("app/src/main/res/raw/gpui_notification_keep.xml")
            .exists()
    );
    fs::write(host.join("GpuiView.kt"), "user edit").unwrap();
    let check = app.run(&["sync", "--check"]);
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stderr).contains("GpuiView.kt"));
    assert!(!app.run(&["sync"]).status.success());
    assert_eq!(
        fs::read_to_string(host.join("GpuiView.kt")).unwrap(),
        "user edit"
    );
}

#[test]
fn invalid_icon_does_not_modify_generated_project() {
    let app = Fixture::new();
    fs::write(
        app.0.join("Cargo.toml"),
        "[package]\nname = 'fixture-app'\nversion = '0.1.0'\n",
    )
    .unwrap();
    let config = app.0.join("gpuiforge.json");
    let mut base = serde_json::json!({"app":{"name":"Example"},"platforms":{"android":{"application-id":"dev.example.app"}}});
    fs::write(&config, base.to_string()).unwrap();
    app.ok(&["sync"]);
    let state = app
        .0
        .join("target/gpuiforge/android/.gpuiforge-generated.json");
    let modified = fs::metadata(&state).unwrap().modified().unwrap();
    base["platforms"]["android"]["icon"] = "missing.png".into();
    fs::write(&config, base.to_string()).unwrap();
    for args in [&["sync", "--check"][..], &["sync"][..]] {
        let result = app.run(args);
        assert!(!result.status.success());
        let message = String::from_utf8_lossy(&result.stderr);
        assert!(message.contains("platforms.android.icon") && message.contains("missing.png"));
        assert_eq!(fs::metadata(&state).unwrap().modified().unwrap(), modified);
    }
}

#[test]
fn published_schemas_match_configuration_types() {
    let app = Fixture::new();
    for (args, expected) in [
        (vec!["schema"], include_str!("../gpuiforge.schema.json")),
        (
            vec!["schema", "--recipe"],
            include_str!("../recipe.schema.json"),
        ),
    ] {
        let result = app.run(&args);
        assert!(result.status.success());
        let actual: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            actual,
            serde_json::from_str::<serde_json::Value>(expected).unwrap()
        );
    }
}

#[test]
fn missing_android_tools_stop_before_generating_or_starting_gradle() {
    let app = Fixture::new();
    fs::write(
        app.0.join("Cargo.toml"),
        "[package]\nname = 'fixture-app'\nversion = '0.1.0'\n",
    )
    .unwrap();
    fs::remove_file(app.0.join("gpuiforge.json")).unwrap();
    app.ok(&["init", "--android", "--application-id", "dev.example.app"]);
    let result = Command::new(env!("CARGO_BIN_EXE_gpuiforge"))
        .args(["build", "android"])
        .env("JAVA_HOME", app.0.join("missing-jdk"))
        .env("ANDROID_HOME", app.0.join("missing-sdk"))
        .env("ANDROID_NDK_HOME", app.0.join("missing-ndk"))
        .current_dir(&app.0)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("JAVA_HOME")
            && stderr.contains("SDK platform")
            && stderr.contains("NDK tool missing"),
        "{stderr}"
    );
    assert!(!stderr.contains("Running bash"));
    assert!(!app.0.join("target/gpuiforge/android").exists());
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
    let config = fs::read_to_string(app.0.join("gpuiforge.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["platforms"]["android"]["management"], "manual");
    assert_eq!(config["app"]["name"], "A & B");
    assert_eq!(
        config["platforms"]["android"]["variables"]["message"],
        "$value"
    );
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
    let config = app.0.join("gpuiforge.json");
    let original: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    let mut changed = original.clone();
    changed["platforms"]["android"]["project-dir"] = "../outside".into();
    fs::write(&config, changed.to_string()).unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    let mut changed = original.clone();
    changed["platforms"]["android"]["managment"] = "manual".into();
    fs::write(&config, changed.to_string()).unwrap();
    assert!(!app.run(&["generate", "android"]).status.success());
    fs::write(&config, original.to_string()).unwrap();
    assert!(!app.run(&["run"]).status.success());
}

#[cfg(unix)]
#[test]
fn android_run_obeys_configured_steps_and_stops_on_adb_errors() {
    use std::os::unix::fs::PermissionsExt;
    let app = Fixture::new();
    let sdk = app.0.join("sdk/platform-tools");
    fs::create_dir_all(&sdk).unwrap();
    let adb = sdk.join("adb");
    fs::write(
        &adb,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$FORGE_LOG"
case "$*" in
  'devices -l')
    printf 'List of devices attached\nfake-device\tdevice model:Test_Phone\nlocked\tunauthorized\nsleeping\toffline\n'
    if [ "$FORGE_DEVICES" = multiple ]; then printf 'second\tdevice model:Second_Phone\n'; fi ;;
  *getprop*)
    if [ "$FORGE_DEVICES" = incompatible ]; then printf 'armeabi-v7a\n'; else printf 'x86_64\n'; fi ;;
  *install*) if [ "$FORGE_FAILURE" = install ]; then exit 1; fi ;;
  *start*) if [ "$FORGE_FAILURE" = launch ]; then printf 'Error: launch failed\n'; fi ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&adb, fs::Permissions::from_mode(0o755)).unwrap();
    let mut recipe: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(app.0.join("recipe.json")).unwrap()).unwrap();
    recipe["artifact"] = "fixture.apk".into();
    recipe["build"] = serde_json::json!([{
        "program": "sh",
        "args": ["-c", "printf apk > \"$1\"", "sh", "{{project_dir}}/fixture.apk"]
    }]);
    fs::write(app.0.join("recipe.json"), recipe.to_string()).unwrap();
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(app.0.join("gpuiforge.json")).unwrap()).unwrap();
    let defaults: serde_json::Value =
        serde_json::from_str(include_str!("../android.json")).unwrap();
    config["platforms"]["android"]["activity"] = "dev.example.MainActivity".into();
    config["platforms"]["android"]["run"] = defaults["run"].clone();
    config["platforms"]["android"]["run"][1]["args"]
        .as_array_mut()
        .unwrap()
        .push("-S".into());
    fs::write(app.0.join("gpuiforge.json"), config.to_string()).unwrap();
    let log = app.0.join("adb.log");
    let listed = Command::new(env!("CARGO_BIN_EXE_gpuiforge"))
        .args(["--config", "missing.json", "devices"])
        .env("ANDROID_HOME", app.0.join("sdk"))
        .env("FORGE_LOG", &log)
        .current_dir(&app.0)
        .output()
        .unwrap();
    assert!(listed.status.success());
    let listing = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listing.contains("Test Phone | x86_64 | ready")
            && listing.contains("unauthorized")
            && listing.contains("offline")
    );
    let calls = fs::read_to_string(&log).unwrap();
    assert!(!calls.contains("-s locked") && !calls.contains("-s sleeping"));
    for failure in ["", "install", "launch"] {
        fs::write(&log, "").unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_gpuiforge"))
            .args(["run", "android", "--device", "fake-device"])
            .env("ANDROID_HOME", app.0.join("sdk"))
            .env("FORGE_LOG", &log)
            .env("FORGE_FAILURE", failure)
            .current_dir(&app.0)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            failure.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let calls = fs::read_to_string(&log).unwrap();
        assert!(calls.contains("-s fake-device install -r "));
        assert_eq!(
            calls.contains("dev.example.app/dev.example.MainActivity -S"),
            failure != "install"
        );
    }
    for (state, device, succeeds, diagnostic) in [
        ("", None, true, ""),
        ("multiple", None, false, "--device SERIAL"),
        ("", Some("locked"), false, "unauthorized"),
        ("", Some("sleeping"), false, "offline"),
        ("incompatible", None, false, "no available Android device"),
    ] {
        fs::write(&log, "").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_gpuiforge"));
        command.args(["run", "android"]);
        if let Some(device) = device {
            command.args(["--device", device]);
        }
        let output = command
            .env("ANDROID_HOME", app.0.join("sdk"))
            .env("FORGE_LOG", &log)
            .env("FORGE_DEVICES", state)
            .current_dir(&app.0)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        let calls = fs::read_to_string(&log).unwrap();
        assert_eq!(calls.contains(" install -r "), succeeds);
    }
}
