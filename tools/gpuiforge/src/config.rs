use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default, rename = "$schema")]
    _schema: Option<String>,
    app: App,
    platforms: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct App {
    name: String,
}

#[derive(schemars::JsonSchema, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Management {
    /// GPUiForge generates native files during sync and before builds. Edited generated files are protected from overwriting.
    #[default]
    Managed,
    /// The application maintains native files. Build/run execute configured steps without regeneration; sync and generate are disabled. Use platform eject to export a managed project.
    Manual,
}

#[derive(schemars::JsonSchema, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Platform {
    /// Optional editor schema reference for a platform recipe. Relative references resolve from the JSON file. GPUiForge does not fetch schema URLs.
    #[serde(default, rename = "$schema")]
    pub _schema: Option<String>,
    /// Optional modules for the bundled Android host. Defaults to an empty list: window rendering, input, IME and lifecycle remain available. Disabled modules omit their Kotlin files, Manifest components and dedicated dependencies. Cargo features and Android permissions are configured separately. Android TalkBack semantics are not implemented.
    #[serde(default)]
    pub features: BTreeSet<AndroidFeature>,
    /// Application and launcher icon for the bundled Android host. Accepts PNG, WebP or Android drawable XML. Resolve relative paths from gpuiforge.json. Omit to leave the application icon unspecified; no copy entry is required.
    pub icon: Option<PathBuf>,
    /// Default small icon for general, media and data-sync notifications. Requires notifications, media-notifications or data-sync. Accepts PNG, WebP or Android drawable XML relative to gpuiforge.json; use a monochrome image with transparency. A per-send resource icon takes priority, followed by this icon, the application icon and Android's generic icon.
    pub notification_icon: Option<PathBuf>,
    /// Optional JSON platform recipe, relative to gpuiforge.json. Application fields override recipe fields: objects merge recursively and arrays replace the whole recipe array. Custom recipes replace bundled Android defaults and cannot include another recipe.
    #[serde(rename = "recipe")]
    pub _recipe: Option<PathBuf>,
    #[serde(skip)]
    pub bundled: bool,
    #[serde(skip)]
    pub default_android_build: bool,
    /// Native project ownership. Defaults to managed. Use gpuiforge platform eject android to export generated sources before taking manual ownership.
    #[serde(default)]
    pub management: Management,
    /// Custom native project template directory. Relative paths resolve from the JSON file declaring this field. Files ending in .tmpl expand {{variables}} and lose that suffix; other files are copied verbatim. Symlinks are rejected. Omit together with recipe to use bundled Android templates.
    pub template: Option<PathBuf>,
    /// Native project directory, always relative to the application's configuration directory, even when declared in a recipe. Defaults to target/gpuiforge/<platform>. Must be nonempty and contain no . or .. components. Managed generation never adopts a nonempty directory without matching ownership metadata.
    pub project_dir: Option<PathBuf>,
    /// Custom string values exposed as {{var.<name>}} in templates and process steps. Values are strings, including SDK levels and version codes. Application values override individual recipe keys. Bundled Android templates recognize the documented SDK, version and native_library keys; arbitrary additional keys are allowed.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Named filesystem paths exposed as {{path.<name>}}. Relative values resolve from the JSON file declaring them and must exist when variables are expanded. Template expansion receives absolute, canonical paths. Use this for shared assets or external template inputs.
    #[serde(default)]
    pub paths: BTreeMap<String, PathBuf>,
    /// Extra file or directory copies into the generated native project. Contents are copied verbatim, including binary assets, without template expansion. Duplicate destinations and symlinks are rejected. Application arrays replace recipe copies; use icon and notification-icon for ordinary Android icons.
    #[serde(default)]
    pub copies: Vec<CopySpec>,
    /// Ordered process steps for gpuiforge build. Android run executes these before its run steps. Each step must succeed before the next starts. Bundled Android defaults assemble the selected debug/release APK; a configured array replaces all default build steps.
    #[serde(default)]
    pub build: Vec<Step>,
    /// Ordered launch steps for gpuiforge run. Android builds first, then runs its configured install and launch steps. Desktop and Web execute only these steps, so they must build or serve the application as needed. Bundled Android defaults use ADB; a configured array replaces all defaults.
    #[serde(default)]
    pub run: Vec<Step>,
    /// Expected build output path relative to project-dir, with {{variables}} supported. When present, build fails if the file does not exist afterward. Android run exposes its absolute path as {{artifact}}. Bundled default: app/build/outputs/apk/{{profile}}/app-{{profile}}{{apk_suffix}}.apk.
    pub artifact: Option<String>,
    /// Android package identity, such as com.example.app. Required directly or through a recipe. Use at least two dot-separated segments starting with ASCII letters; remaining characters may be letters, digits or underscores. This is independent of the Kotlin host namespace and Rust package name.
    pub application_id: Option<String>,
    /// Android Activity class used by launch steps through {{activity}}. Bundled default: dev.gpuiforge.app.MainActivity. Changing this value does not rename generated Kotlin classes or Manifest entries; keep it aligned with a custom or manually maintained host.
    pub activity: Option<String>,
    /// Android architectures enabled for packaging. Bundled defaults: arm64-v8a and x86_64. Builds package all enabled ABIs unless --abi selects one; device runs choose a compatible enabled ABI. At least one is required, and 32-bit Android targets are not supported.
    #[serde(default)]
    pub abis: Vec<String>,
    /// Fully qualified Android permission names written as uses-permission entries in the bundled Manifest, for example android.permission.INTERNET. Defaults to empty; duplicates are removed. Modules do not grant or request runtime permissions automatically. Runtime authorization remains the application's responsibility.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Lowercase custom URI schemes delivered through Application::on_open_urls, for example myapp. Defaults to empty; duplicates are removed. Bundled templates generate VIEW/DEFAULT/BROWSABLE intent filters. HTTP(S) App Links and file/content URI handlers require a custom Manifest.
    #[serde(default)]
    pub url_schemes: Vec<String>,
    /// MIME types accepted from Android SEND and SEND_MULTIPLE intents, for example text/plain, image/* or */*. Defaults to empty, which does not register a share target. Use lowercase type/subtype syntax. The bundled host requires the sharing feature; the application handles received content itself.
    #[serde(default)]
    pub share_mime_types: Vec<String>,
    /// Optional Android release signing configuration. All four fields are required when present. Passwords come from environment variables during --release builds; debug builds use the development key. Without this object, the bundled release build produces an unsigned APK. Store the keystore outside the generated project.
    pub signing: Option<Signing>,
}

#[derive(schemars::JsonSchema, Clone, Copy, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidFeature {
    /// Document open/save pickers, persistent document grants, private/public storage and the file provider. Does not require broad storage access permissions for document picker use.
    Files,
    /// Sending and receiving Android shares. Automatically includes files. Set share-mime-types separately to register the application as a share target.
    Sharing,
    /// Android-backed credential storage used by GPUI's credential API. Enable when storing or retrieving application credentials.
    Credentials,
    /// Media playback and frame decoding, including the Media3 dependency. System playback notifications are selected separately with media-notifications.
    Media,
    /// General system notifications, action buttons and inline replies. Declare POST_NOTIFICATIONS and request runtime authorization when required by Android.
    Notifications,
    /// Android system media session and playback notification controls. Independent of media and notifications, so an application may connect its own player. Does not create a foreground service or keep background work alive.
    MediaNotifications,
    /// Session-bound foreground execution for application-owned data transfers. Declare FOREGROUND_SERVICE and FOREGROUND_SERVICE_DATA_SYNC. Does not schedule work, survive session closure or restore tasks after process death.
    DataSync,
}
impl Platform {
    pub fn feature(&self, feature: AndroidFeature) -> bool {
        self.features.contains(&feature)
            || (feature == AndroidFeature::Files
                && self.features.contains(&AndroidFeature::Sharing))
    }
}

#[derive(schemars::JsonSchema, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Signing {
    /// Existing keystore file, relative to the JSON file declaring this field or an absolute path. Must remain outside project-dir so generated files cannot replace it. GPUiForge does not generate keys or package the keystore in the APK.
    pub keystore: PathBuf,
    /// Alias of the signing key inside the keystore, not the keystore filename. Must be nonempty and match the alias used when the key was created.
    pub key_alias: String,
    /// Name of the environment variable containing the keystore password, not the password itself. It must be set to a nonempty value for release builds. Example: ANDROID_STORE_PASSWORD.
    pub store_password_env: String,
    /// Name of the environment variable containing the private signing key's password. It may name the same variable as store-password-env when both passwords are identical. Example: ANDROID_KEY_PASSWORD.
    pub key_password_env: String,
}

#[derive(schemars::JsonSchema, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopySpec {
    /// Existing source file or directory. Relative paths resolve from the JSON file declaring this copy. Directory contents are copied recursively; symlinks are rejected and content is not template-expanded.
    pub from: PathBuf,
    /// Destination relative to the generated project directory. A file source requires its destination filename; a directory source supplies contents under this directory. Must be nonempty, contain no . or .. components, and not overlap another generated file.
    pub to: PathBuf,
}

#[derive(schemars::JsonSchema, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Step {
    /// Executable name resolved through PATH, or an explicit executable path. Supports {{variables}}. GPUiForge starts it directly without a shell; use an explicit shell program when shell syntax is required.
    pub program: String,
    /// Arguments passed individually to the executable, in order. Defaults to empty. Spaces do not split an argument; shell quoting, pipes, globbing and $VAR expansion are not performed by GPUiForge. Common placeholders: {{cargo_profile}} (dev/release), {{profile}} (debug/release), {{variant}} (Debug/Release), {{project_dir}} and {{abis}}. Android run steps also receive {{adb}}, {{device}} and {{artifact}}. Custom values use {{var.<name>}} or {{path.<name>}}.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory for this step. Defaults to the application configuration directory. Relative paths also resolve from that directory, including steps inherited from a recipe. Supports {{variables}}; use {{project_dir}} for Gradle commands in the native project.
    pub cwd: Option<String>,
    /// Additional environment variables for this process. Values are strings supporting {{variables}}; unlisted variables are inherited from GPUiForge's environment. Generated release-signing variables take precedence. Reference password variable names in signing instead of storing passwords here.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Optional case-sensitive literal substring checked in stdout and stderr. A match fails the step even if its exit code is zero. When set, output is captured and printed after the process finishes; otherwise streams are inherited. Use Error: to detect ADB launch errors. This is not a regular expression.
    pub error_pattern: Option<String>,
}

pub struct Project {
    pub config_path: PathBuf,
    pub root: PathBuf,
    pub name: String,
    pub platforms: BTreeMap<String, Platform>,
}

pub type Variables = BTreeMap<String, String>;

fn read(path: &Path) -> Result<serde_json::Value> {
    serde_json::from_str(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))
}

fn paths(value: &mut serde_json::Value, base: &Path) -> Result<()> {
    fn resolve(value: &mut serde_json::Value, base: &Path) -> Result<()> {
        let path = value.as_str().context("expected a path string")?;
        *value = serde_json::Value::String(base.join(path).to_string_lossy().into_owned());
        Ok(())
    }
    for field in ["icon", "notification-icon"] {
        if let Some(path) = value.get_mut(field) {
            resolve(path, base)?;
        }
    }
    if let Some(template) = value.get_mut("template") {
        resolve(template, base)?;
    }
    if let Some(signing) = value.get_mut("signing") {
        resolve(
            signing
                .get_mut("keystore")
                .context("signing.keystore is required")?,
            base,
        )?;
    }
    if let Some(table) = value
        .get_mut("paths")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (_, path) in table.iter_mut() {
            resolve(path, base)?;
        }
    }
    if let Some(copies) = value
        .get_mut("copies")
        .and_then(serde_json::Value::as_array_mut)
    {
        for copy in copies {
            resolve(copy.get_mut("from").context("copy is missing from")?, base)?;
        }
    }
    Ok(())
}

fn merge(base: &mut serde_json::Value, overlay: serde_json::Value) {
    if let (Some(base), Some(overlay)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay {
            if let Some(existing) = base.get_mut(key) {
                merge(existing, value.clone());
            } else {
                base.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay;
    }
}

pub fn relative(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path.components().all(|c| matches!(c, Component::Normal(_))),
        "expected a nonempty relative path without '.' or '..': {}",
        path.display()
    );
    Ok(())
}

impl Project {
    pub fn load(path: &Path) -> Result<Self> {
        let config_path = path
            .canonicalize()
            .with_context(|| format!("configuration not found: {}", path.display()))?;
        let root = config_path.parent().unwrap().to_owned();
        let config: Config = serde_json::from_value(read(&config_path)?)?;
        ensure!(
            !config.app.name.trim().is_empty(),
            "app.name must not be empty"
        );
        let mut platforms = BTreeMap::new();
        for (name, mut value) in config.platforms {
            ensure!(
                matches!(name.as_str(), "desktop" | "android" | "web"),
                "unknown platform {name}"
            );
            let bundled = name == "android"
                && value.get("recipe").is_none()
                && value.get("template").is_none();
            let mut base = if let Some(recipe) = value.get("recipe") {
                let recipe = root
                    .join(recipe.as_str().context("recipe must be a path")?)
                    .canonicalize()?;
                let mut base = read(&recipe)?;
                ensure!(
                    base.get("recipe").is_none(),
                    "recipes cannot include another recipe"
                );
                paths(&mut base, recipe.parent().unwrap())?;
                base
            } else if bundled {
                serde_json::from_str(include_str!("../android.json"))?
            } else {
                serde_json::json!({})
            };
            paths(&mut value, &root)?;
            merge(&mut base, value);
            let mut platform: Platform =
                serde_json::from_value(base).with_context(|| format!("invalid platform {name}"))?;
            platform.bundled = bundled;
            platform.default_android_build = bundled
                && platform.build
                    == serde_json::from_str::<Platform>(include_str!("../android.json"))?.build;
            if bundled && !platform.variables.contains_key("native_library") {
                let manifest: toml::Value =
                    toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
                let package = manifest
                    .get("package")
                    .and_then(|p| p.get("name"))
                    .and_then(toml::Value::as_str)
                    .context("Cargo.toml must describe an application package")?;
                platform
                    .variables
                    .insert("native_library".into(), package.replace('-', "_"));
            }
            if let Some(dir) = &platform.project_dir {
                relative(dir)?;
            }
            for copy in &platform.copies {
                relative(&copy.to)?;
            }
            if name == "android" {
                ensure!(
                    bundled
                        || (platform.features.is_empty()
                            && platform.icon.is_none()
                            && platform.notification_icon.is_none()),
                    "features, icon and notification-icon require the bundled Android template"
                );
                ensure!(
                    !bundled
                        || platform.share_mime_types.is_empty()
                        || platform.feature(AndroidFeature::Sharing),
                    "share-mime-types requires the sharing feature"
                );
                ensure!(
                    platform.notification_icon.is_none()
                        || platform.feature(AndroidFeature::Notifications)
                        || platform.feature(AndroidFeature::MediaNotifications)
                        || platform.feature(AndroidFeature::DataSync),
                    "notification-icon requires notifications, media-notifications or data-sync"
                );
                for permission in &platform.permissions {
                    ensure!(
                        permission.contains('.')
                            && permission.split('.').all(|part| {
                                let mut chars = part.chars();
                                chars
                                    .next()
                                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                                    && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
                            }),
                        "invalid Android permission name: {permission}"
                    );
                }
                if platform.feature(AndroidFeature::DataSync) {
                    for required in [
                        "android.permission.FOREGROUND_SERVICE",
                        "android.permission.FOREGROUND_SERVICE_DATA_SYNC",
                    ] {
                        ensure!(
                            platform.permissions.iter().any(|value| value == required),
                            "data-sync requires {required} in permissions"
                        );
                    }
                }
                platform.permissions.sort();
                platform.permissions.dedup();
                for scheme in &platform.url_schemes {
                    let mut chars = scheme.chars();
                    ensure!(
                        chars.next().is_some_and(|c| c.is_ascii_lowercase())
                            && chars.all(|c| c.is_ascii_lowercase()
                                || c.is_ascii_digit()
                                || matches!(c, '+' | '-' | '.'))
                            && !matches!(scheme.as_str(), "http" | "https" | "file" | "content"),
                        "url-schemes requires lowercase custom URI schemes; configure web and file links in a custom manifest: {scheme}"
                    );
                }
                platform.url_schemes.sort();
                platform.url_schemes.dedup();
                for mime in &platform.share_mime_types {
                    let valid_part = |part: &str| {
                        !part.is_empty()
                            && part.bytes().all(|c| {
                                c.is_ascii_lowercase()
                                    || c.is_ascii_digit()
                                    || b"!#$%&'+-.^_`|~".contains(&c)
                            })
                    };
                    ensure!(
                        mime.split_once('/').is_some_and(|(major, minor)| {
                            (valid_part(major) && (valid_part(minor) || minor == "*"))
                                || (major == "*" && minor == "*")
                        }),
                        "invalid share MIME type (use lowercase type/subtype): {mime}"
                    );
                }
                platform.share_mime_types.sort();
                platform.share_mime_types.dedup();
                if let Some(signing) = &platform.signing {
                    ensure!(
                        !signing.key_alias.trim().is_empty(),
                        "signing.key-alias must not be empty"
                    );
                    for name in [&signing.store_password_env, &signing.key_password_env] {
                        let mut chars = name.chars();
                        ensure!(
                            chars
                                .next()
                                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_'),
                            "invalid signing environment variable name"
                        );
                    }
                }
                ensure!(
                    !platform.abis.is_empty()
                        && platform
                            .abis
                            .iter()
                            .all(|a| matches!(a.as_str(), "arm64-v8a" | "x86_64")),
                    "android.abis must contain arm64-v8a and/or x86_64"
                );
                let id = platform
                    .application_id
                    .as_deref()
                    .context("android.application-id is required")?;
                ensure!(
                    id.contains('.')
                        && id.split('.').all(|part| {
                            let mut chars = part.chars();
                            chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
                        }),
                    "invalid Android application-id"
                );
            } else {
                ensure!(
                    platform.permissions.is_empty()
                        && platform.features.is_empty()
                        && platform.icon.is_none()
                        && platform.notification_icon.is_none()
                        && platform.signing.is_none()
                        && platform.url_schemes.is_empty()
                        && platform.share_mime_types.is_empty(),
                    "features, icons, permissions, signing, url-schemes and share-mime-types are Android-only settings"
                );
            }
            platforms.insert(name, platform);
        }
        ensure!(!platforms.is_empty(), "configure at least one platform");
        Ok(Self {
            config_path,
            root,
            name: config.app.name,
            platforms,
        })
    }

    pub fn platform(&self, name: &str) -> Result<&Platform> {
        self.platforms
            .get(name)
            .with_context(|| format!("platform {name} is not configured"))
    }

    pub fn directory(&self, name: &str) -> Result<PathBuf> {
        let platform = self.platform(name)?;
        Ok(self.root.join(
            platform
                .project_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("target/gpuiforge/{name}"))),
        ))
    }

    pub fn variables(&self, name: &str, release: bool, abi: Option<&str>) -> Result<Variables> {
        let p = self.platform(name)?;
        let mut vars = BTreeMap::from([
            ("app.name".into(), self.name.clone()),
            ("app_dir".into(), self.root.display().to_string()),
            (
                "tool_path".into(),
                std::env::current_exe()?.display().to_string(),
            ),
            (
                "project_dir".into(),
                self.directory(name)?.display().to_string(),
            ),
            (
                "profile".into(),
                if release { "release" } else { "debug" }.into(),
            ),
            (
                "cargo_profile".into(),
                if release { "release" } else { "dev" }.into(),
            ),
            (
                "apk_suffix".into(),
                if release && p.signing.is_none() {
                    "-unsigned"
                } else {
                    ""
                }
                .into(),
            ),
            (
                "variant".into(),
                if release { "Release" } else { "Debug" }.into(),
            ),
            (
                "abis".into(),
                abi.map(str::to_owned).unwrap_or_else(|| p.abis.join(",")),
            ),
        ]);
        vars.insert(
            "android_permissions".into(),
            p.permissions
                .iter()
                .map(|name| format!("    <uses-permission android:name=\"{name}\" />\n"))
                .collect(),
        );
        vars.insert("android_icon".into(), if p.icon.is_some() { "android:icon=\"@drawable/gpui_app_icon\" android:roundIcon=\"@drawable/gpui_app_icon\"".into() } else { String::new() });
        if let Some(id) = &p.application_id {
            vars.insert("application_id".into(), id.clone());
        }
        vars.insert("android_url_filters".into(), p.url_schemes.iter().map(|scheme| format!(
            "            <intent-filter>\n                <action android:name=\"android.intent.action.VIEW\" />\n                <category android:name=\"android.intent.category.DEFAULT\" />\n                <category android:name=\"android.intent.category.BROWSABLE\" />\n                <data android:scheme=\"{scheme}\" />\n            </intent-filter>\n"
        )).collect());
        vars.insert("android_share_filters".into(), if p.share_mime_types.is_empty() {
            String::new()
        } else {
            let types: String = p.share_mime_types.iter().map(|mime| format!(
                "                <data android:mimeType=\"{}\" />\n", mime.replace('&', "&amp;").replace('\'', "&apos;")
            )).collect();
            ["SEND", "SEND_MULTIPLE"].iter().map(|action| format!(
                "            <intent-filter>\n                <action android:name=\"android.intent.action.{action}\" />\n                <category android:name=\"android.intent.category.DEFAULT\" />\n{types}            </intent-filter>\n"
            )).collect()
        });
        if let Some(activity) = &p.activity {
            vars.insert("activity".into(), activity.clone());
        }
        for (key, value) in &p.variables {
            vars.insert(format!("var.{key}"), value.clone());
        }
        for (key, path) in &p.paths {
            vars.insert(
                format!("path.{key}"),
                path.canonicalize()
                    .with_context(|| format!("missing path {key}: {}", path.display()))?
                    .display()
                    .to_string(),
            );
        }
        Ok(vars)
    }
}

pub fn expand(input: &str, vars: &Variables) -> Result<String> {
    let mut rest = input;
    let mut result = String::new();
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        let end = rest
            .find("}}")
            .context("unterminated template expression")?;
        let (name, filter) = rest[..end]
            .trim()
            .split_once('|')
            .unwrap_or((rest[..end].trim(), "raw"));
        let value = vars
            .get(name.trim())
            .with_context(|| format!("unknown template variable {}", name.trim()))?;
        let value = match filter.trim() {
            "raw" => value.clone(),
            "xml" => value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;"),
            "kotlin" => {
                let quoted = serde_json::to_string(value)?;
                quoted[1..quoted.len() - 1].replace('$', "\\$")
            }
            other => bail!("unknown template filter {other}"),
        };
        result.push_str(&value);
        rest = &rest[end + 2..];
    }
    result.push_str(rest);
    Ok(result)
}
