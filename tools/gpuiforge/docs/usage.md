# Configuration and workflows

Run GPUiForge in a directory containing `gpuiforge.json`, or pass
`--config path/to/gpuiforge.json`. Diagnostics and commands go to stderr;
generated directories and build artifact paths go to stdout. Child processes
inherit the terminal streams. CLI syntax errors exit with 2; execution errors
exit with 1.

## Application setup

`gpuiforge init` creates desktop configuration for an existing Cargo package.
To also configure Android, supply `--android`
and `--application-id dev.example.app`. Existing configuration is never replaced.
Initialization writes the default build and run steps into the configuration so
they can be edited directly. It also writes `gpuiforge.schema.json` beside the
configuration and sets `$schema` to that local file for offline editor completion
and validation. Configuration uses strict JSON; comments and trailing commas are
not accepted. Cargo manifests remain TOML.

To export the schema independently:

```sh
gpuiforge schema > gpuiforge.schema.json
gpuiforge schema --recipe > recipe.schema.json
```

Application configuration can reference `"$schema": "./gpuiforge.schema.json"`;
custom recipes can reference `"$schema": "./recipe.schema.json"`. GPUiForge does
not fetch schema URLs. The CLI validates configuration independently and also
checks filesystem paths and platform-specific requirements.

```json
{
  "app": {
    "name": "My App"
  },
  "platforms": {
    "android": {
      "abis": ["arm64-v8a", "x86_64"],
      "application-id": "dev.example.app",
      "build": [
        {
          "args": ["gradlew", "--no-daemon", "-PgpuiAbis={{abis}}", "-PgpuiForgeExecutable={{tool_path}}", ":app:assemble{{variant}}"],
          "cwd": "{{project_dir}}",
          "program": "bash"
        }
      ],
      "run": [
        {
          "args": ["-s", "{{device}}", "install", "-r", "{{artifact}}"],
          "program": "{{adb}}"
        },
        {
          "args": ["-s", "{{device}}", "shell", "am", "start", "-W", "-n", "{{application_id}}/{{activity}}"],
          "error-pattern": "Error:",
          "program": "{{adb}}"
        }
      ]
    },
    "desktop": {
      "build": [
        {
          "args": ["build", "--profile", "{{cargo_profile}}"],
          "program": "cargo"
        }
      ],
      "run": [
        {
          "args": ["run", "--profile", "{{cargo_profile}}"],
          "program": "cargo"
        }
      ]
    }
  }
}
```

GPUiForge includes the Kotlin host, Gradle wrapper, Android template and Rust
packaging support. No recipe path or GPUI source checkout is required. The native
library name is derived from the Cargo package name, replacing hyphens with
underscores. The default Android build supports Linux and macOS build hosts,
ARM64 and x86_64 Android, and packages
Rust applications using `src/main.rs` without a companion library or explicit
`[[bin]]`.

`abis` selects the packaged architectures: `arm64-v8a` uses Rust target
`aarch64-linux-android`, and `x86_64` uses `x86_64-linux-android`. Keep one entry
for a single-architecture build or both for a multi-ABI APK. Device runs select
a supported ABI from this list and fail if none matches the device.

Use `gpuiforge build android --abi arm64-v8a` or `--abi x86_64` to build only one
enabled architecture without changing the configuration. The override also limits
prerequisite checks to that architecture and can be combined with `--release`.
With `--abi`, an omitted platform defaults to Android. Disabled ABIs and use with
another platform are rejected before generation or build steps begin.

Optional Android settings can override the defaults:

```json
{
  "platforms": {
    "android": {
      "variables": {
        "min_sdk": "26",
        "target_sdk": "36",
        "version_code": "1",
        "version_name": "0.1.0"
      }
    }
  }
}
```

`gpuiforge devices` lists connected Android devices with their model, ABI and
connection status. It works without a project configuration and does not query
device properties on unauthorized or offline devices.

`gpuiforge run` prompts for a configured platform. Android runs automatically use
a sole authorized device whose ABI is enabled in the project. Multiple compatible
devices produce a selection menu; non-interactive runs require `--device SERIAL`
in that case. Offline, unauthorized and incompatible devices are not candidates.
An explicitly selected unavailable device produces an error, without substituting
another device. Passing `--device` defaults an omitted platform to Android.

Android runs build only the selected device ABI. `gpuiforge build android` builds
all configured ABIs. For an explicit selection, use
`gpuiforge run android --device emulator-5554`.
Android `run` selects the device and ABI, executes the configured build steps,
then executes the configured run steps in order. Installation and launch are
performed only by those run steps.
Desktop/Web `run` executes its configured run steps, which must build or serve
the application as needed. It does not separately execute the build steps.

Default managed Android builds check the selected JDK's `java` and `javac`, SDK
platform 36.1, NDK compilers for `min_sdk`, and Rust standard libraries before
generating files or starting Gradle. `run` checks the device ABI; `build` checks
all configured ABIs. Set `JAVA_HOME`, `ANDROID_HOME` (or `ANDROID_SDK_ROOT`) and
`ANDROID_NDK_HOME` to complete installations. Custom build steps and manually
maintained projects control their own prerequisite handling.

`gpuiforge doctor` checks Cargo, Rust, Java, Android SDK/NDK tools, target standard
libraries for the default Android build, ADB, and configured paths. It reports
all missing prerequisites together. It does not install tools, start emulators, or certify
driver compatibility. Platform selection lists configured targets; builds report
unavailable tools. GPUiForge does not provide a bundled Web recipe.

## Android host features and icons

Select optional host modules in the Android platform configuration:

```json
{
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "features": ["files", "sharing", "notifications"],
      "icon": "assets/app.png",
      "notification-icon": "assets/notification.xml"
    }
  }
}
```

With `"features": []` (the default), the host provides window rendering, input,
IME, lifecycle, permissions and other core system integration. Android TalkBack
semantics are not implemented.
Optional modules are:

| Feature | Capability |
| --- | --- |
| `files` | Document pickers, persistent document grants, private/public storage and file provider |
| `sharing` | Sending and receiving shares; automatically includes `files` |
| `credentials` | Android credential storage |
| `media` | Media playback, decoding and picture-in-picture windows, including the Media3 dependency |
| `notifications` | General notifications, actions and inline replies |
| `media-notifications` | System media session and playback notification; independent of `media` |
| `background-media` | Explicit foreground playback leases; includes `media-notifications`; requires `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_MEDIA_PLAYBACK` in `permissions` |
| `data-sync` | Session-bound foreground execution for application-owned transfers; requires `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_DATA_SYNC` in `permissions` |

Disabled modules omit their Kotlin sources, Manifest components and module-specific
dependencies. Applications must enable the modules used by their Rust APIs;
calling an unavailable host capability returns an error. Rust Cargo dependencies
are configured separately. Permissions remain explicit in `permissions`.

`icon` sets the application and launcher icon. `notification-icon` sets the default
small icon for general, media and data-sync notifications; it requires one of those
features. Both paths resolve relative to `gpuiforge.json` and accept PNG, WebP or
Android drawable XML. Small notification icons should be monochrome with a
transparent background. A per-notification resource icon overrides this default;
missing resources fall back to the configured notification icon, then the app icon
(or Android's generic application icon when neither is configured).
Adaptive launcher icons with multiple resources can be maintained in an exported
native project.

Run `gpuiforge sync` (or `gpuiforge sync android`) after editing the configuration.
It regenerates the managed project without building or requiring an Android SDK,
and removes previously generated files for disabled features or removed icons.
Build and run also synchronize managed projects automatically.
Use `gpuiforge sync --check` to check without writing files or starting Gradle.
It exits with 0 when the managed project matches the current configuration and
bundled templates, or 1 with the paths that need creation, updating or removal.
An absent generated project also needs synchronization. Modified generated files
and invalid resources are reported as errors; the check does not repair them.
On Unix, the check also detects changed executable permission bits.
Modified generated files are protected: restore them or use `platform eject android`
to take ownership. Both `sync` and `generate` are disabled in manual mode.
These feature and icon settings apply to the bundled Android template; custom
templates define their own sources and resources.

## Android layout insets

`platforms.android.inset-handling` defaults to `"application"`. The bundled
Activity leaves the GPUI viewport at the window's full size and publishes
system-bar, display-cutout and keyboard geometry through `Window::insets()`.
The Rust application decides which content needs padding, using
`window.insets().effective()` in logical pixels.

Set `"inset-handling": "host"` for automatic avoidance. System bars remain
visible in both modes. Run `gpuiforge sync android` after changing this setting.
The [Android hosting guide](../../../crates/gpui_android/docs/hosting.md#safe-areas-and-the-keyboard)
shows Rust layout code and the equivalent custom Activity configuration.

## Recipes and templates

A recipe is an optional custom JSON platform definition. Android uses bundled
defaults when neither `recipe` nor `template` is configured. Application fields override recipe fields;
objects merge recursively and arrays replace the recipe's arrays. Recipes cannot
include another recipe. Unknown configuration fields are rejected.

Platform fields:

| Field | Meaning |
| --- | --- |
| `recipe` | Optional custom platform definition file |
| `management` | `managed` (default) or `manual` |
| `template` | Directory of project files |
| `project-dir` | Output directory, relative to the application; defaults to `target/gpuiforge/<platform>` |
| `variables` | String template values |
| `paths` | Paths exposed to templates and commands |
| `copies` | Array of `{ from, to }` copies, including binary files |
| `build`, `run` | Ordered process steps |
| `artifact` | Expected file relative to the platform project after building |
| `application-id`, `activity` | Android launch identifiers |
| `abis` | Enabled Android ABIs: `arm64-v8a`, `x86_64` |

Each step contains `program`, `args`, optional `cwd`, and optional `env`.
Commands receive an argument vector directly; there is no implicit shell.
The default working directory is the application directory. Steps stop on the
first failure. Configured programs execute with the user's permissions.
An optional `error-pattern` is a literal substring checked against both output
streams. These steps capture and print output when the command finishes and fail
when the substring appears, even if the process exits with zero. The default
Android launch step uses this to detect errors reported by `adb shell am start`.

Recipe `template`, `paths`, and copy `from` paths resolve relative to the recipe;
application overrides resolve relative to `gpuiforge.json`. `project-dir` always
resolves relative to the application. Copy destinations must remain inside the
generated project. Template/copy symlinks are rejected.

Project-owned recipes can use `"recipe": "platforms/android.json"` or
`gpuiforge init --android-recipe /path/to/android.json --application-id dev.example.app`.
Custom recipes replace the bundled platform definition. GPUiForge does not link
GPUI crates; the application selects its own framework dependencies.

Files ending in `.tmpl` are UTF-8 templates; generation removes that suffix.
Other files and explicit copies are copied verbatim, including executable
permissions. Variables use `{{name}}`; available names are `app.name`, `app_dir`,
`project_dir`, `tool_path` (the running GPUiForge executable), `profile` (`debug`/`release`), `cargo_profile` (`dev`/`release`),
`variant` (`Debug`/`Release`), `abis`, `application_id`, `activity`, `var.<key>`,
and `path.<key>`. `apk_suffix` is empty in debug and `-unsigned` in release for
the supplied Android recipe. `|xml` escapes XML values; `|kotlin` escapes Kotlin
string contents, including dollar signs. Unknown variables or filters fail
generation. Templates describe both build profiles; profile-specific values
are intended for process steps and artifact paths.
Android run steps also receive `adb` (the selected ADB executable), `device`
(the selected serial) and `artifact` (the absolute APK path).

The bundled Android template generates the Kotlin host, application, Gradle
wrapper, Manifest and build script in the application's output directory.
Generated paths refer to the Rust application; use a fresh output directory after
relocating the application.

## Ownership

`gpuiforge generate android` creates the managed project. `build` and Android
`run` regenerate it before building. The generator tracks its own files and
only updates unchanged generated content. It refuses to adopt a nonempty
unmarked directory, overwrite an untracked file, or replace edited generated
files. Use a single GPUiForge command per application at a time.

To take ownership:

```sh
gpuiforge platform eject android
```

This exports current sources, including edits and additions, into
`platforms/android`. Build caches and `local.properties` are excluded. It changes
`management` to `manual` and records `project-dir` in the application's JSON,
preserving other fields and the schema reference. An existing destination is never overwritten. The exported
project contains its host sources and retains references to its Rust application.

In manual mode, builds execute the configured steps without generating native
files. `generate` and `sync` are disabled. Kotlin, Manifest, Gradle and resources belong to
the application. Keep the configured launch identifiers and artifact path in
sync if you change them in the native project. Automatic return to managed mode
is not provided: choose a new empty project-dir to generate another project.
Rust packaging still uses GPUiForge. Direct Gradle invocations resolve
`gpuiforge` from PATH, or accept `-PgpuiForgeExecutable=/path/to/gpuiforge`.

## Build boundaries

The supplied Android debug recipe produces a signed development APK. Release
produces `app-release-unsigned.apk` without signing configuration, or
`app-release.apk` when signing is configured. Custom Gradle projects must keep
`artifact` consistent with their output. AAB packaging, store publication,
framework update automation and native source overlays are not built-in commands.

## Android permissions

Declare permission names on the Android platform:

```json
{
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "permissions": ["android.permission.INTERNET", "android.permission.RECORD_AUDIO"]
    }
  }
}
```

The bundled template writes these names as `uses-permission` entries in the
application Manifest. Duplicates are removed. A custom template can use
`{{android_permissions}}` for the generated XML. In manual mode, maintain the
Manifest yourself. Runtime permissions still require application authorization
requests; configuration does not show dialogs or grant access. GPUI's Android
backend provides `AndroidPermissions` for these requests.

## Android links

```json
{
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "url-schemes": ["myapp"]
    }
  }
}
```

The bundled template generates `VIEW`, `DEFAULT`, and `BROWSABLE` intent filters
for lowercase custom URI schemes. Each scheme has its own filter; duplicates
are removed. The Activity uses `singleTop`, so a link targeting the top Activity
arrives through `onNewIntent`. GPUI applications receive URLs through
`Application::on_open_urls`.

Custom templates can use `{{android_url_filters}}`. With manual management,
maintain the filters and Activity launch mode in your manifest. HTTP(S) App Links
need host-specific filters and website verification, so they are configured in
a custom manifest instead of `url-schemes`.

## Receiving Android shares

```json
{
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "features": ["sharing"],
      "share-mime-types": ["text/plain", "image/*", "application/pdf"]
    }
  }
}
```

The bundled template generates `SEND` and `SEND_MULTIPLE` filters with the
`DEFAULT` category. Types use lowercase `type/subtype` syntax; `image/*` and
`*/*` wildcards are supported. Declare only content your application can handle.
An empty or omitted list does not register a share target.

GPUI delivers incoming content through `Application::on_receive_share`.
Custom templates can use `{{android_share_filters}}`; manually managed hosts
maintain their own manifest filters and forward incoming intents.

## Android release signing

```json
{
  "platforms": {
    "android": {
      "signing": {
        "key-alias": "release",
        "key-password-env": "ANDROID_KEY_PASSWORD",
        "keystore": ".gpuiforge/signing/release.jks",
        "store-password-env": "ANDROID_STORE_PASSWORD"
      }
    }
  }
}
```

The keystore must already exist. Paths are relative to the configuration file
(or recipe when declared there); absolute paths are also supported. Keep the
keystore outside the generated Android directory. GPUiForge does not create,
copy, replace, or delete keys. Exclude `.gpuiforge/signing/` from version control
and keep a separate backup.

Provide the named password environment variables before `build android --release`
or `run android --release`. Missing files and unset or empty passwords fail before
the build. Debug builds do not require release credentials. Passwords are passed
to build child processes through the environment, not template variables or CLI
arguments. Generated Gradle files contain no password values. Build scripts run
with access to these credentials and must be trusted.

The bundled Gradle project and its ejected copy use `GPUIFORGE_SIGNING_ENABLED`,
`GPUIFORGE_SIGNING_KEYSTORE`, `GPUIFORGE_SIGNING_KEY_ALIAS`,
`GPUIFORGE_SIGNING_STORE_PASSWORD`, and `GPUIFORGE_SIGNING_KEY_PASSWORD`.
Custom build steps can consume the same environment. These names are reserved
for GPUiForge's build process. A configured release build sets the enabled flag
to `1`; other Android builds set it to `0`.
