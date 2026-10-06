# Configuration and workflows

Run GPUiForge in a directory containing `gpuiforge.toml`, or pass
`--config path/to/gpuiforge.toml`. Diagnostics and commands go to stderr;
generated directories and build artifact paths go to stdout. Child processes
inherit the terminal streams. CLI syntax errors exit with 2; execution errors
exit with 1.

## Application setup

`gpuiforge init` creates desktop configuration for an existing Cargo package.
To also configure Android, supply `--android`
and `--application-id dev.example.app`. Existing configuration is never replaced.
Initialization writes the default build and run steps into the configuration so
they can be edited directly.

```toml
[app]
name = "My App"

[[platforms.desktop.build]]
program = "cargo"
args = ["build", "--profile", "{{cargo_profile}}"]

[[platforms.desktop.run]]
program = "cargo"
args = ["run", "--profile", "{{cargo_profile}}"]

[platforms.android]
application-id = "dev.example.app"
abis = ["arm64-v8a", "x86_64"]

[[platforms.android.build]]
program = "bash"
args = ["gradlew", "--no-daemon", "-PgpuiAbis={{abis}}", "-PgpuiForgeExecutable={{tool_path}}", ":app:assemble{{variant}}"]
cwd = "{{project_dir}}"

[[platforms.android.run]]
program = "{{adb}}"
args = ["-s", "{{device}}", "install", "-r", "{{artifact}}"]

[[platforms.android.run]]
program = "{{adb}}"
args = ["-s", "{{device}}", "shell", "am", "start", "-W", "-n", "{{application_id}}/{{activity}}"]
error-pattern = "Error:"
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

```toml
[platforms.android.variables]
min_sdk = "26"
target_sdk = "36"
version_code = "1"
version_name = "0.1.0"
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

## Recipes and templates

A recipe is an optional custom TOML platform definition. Android uses bundled
defaults when neither `recipe` nor `template` is configured. Application fields override recipe fields;
tables merge recursively and arrays replace the recipe's arrays. Recipes cannot
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
application overrides resolve relative to `gpuiforge.toml`. `project-dir` always
resolves relative to the application. Copy destinations must remain inside the
generated project. Template/copy symlinks are rejected.

Project-owned recipes can use `recipe = "platforms/android.toml"` or
`gpuiforge init --android-recipe /path/to/android.toml --application-id dev.example.app`.
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
`management` to `manual` and records `project-dir` in the application's TOML,
preserving comments. An existing destination is never overwritten. The exported
project contains its host sources and retains references to its Rust application.

In manual mode, builds execute the configured steps without generating native
files. `generate` is disabled. Kotlin, Manifest, Gradle and resources belong to
the application. Keep the configured launch identifiers and artifact path in
sync if you change them in the native project. Automatic return to managed mode
is not provided: choose a new empty project-dir to generate another project.
Rust packaging still uses GPUiForge. Direct Gradle invocations resolve
`gpuiforge` from PATH, or accept `-PgpuiForgeExecutable=/path/to/gpuiforge`.

## Build boundaries

The supplied Android debug recipe produces a signed development APK. Its
release APK is unsigned; signing and store publication require application
configuration in a manually maintained Gradle project. Adjust `artifact` if
the signed APK has another filename. AAB packaging, signing configuration,
framework update automation and native source overlays are not built-in
commands. Customization can use a project-owned recipe/template or manual mode.
