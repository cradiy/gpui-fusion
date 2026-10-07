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
    app: App,
    platforms: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct App {
    name: String,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Management {
    #[default]
    Managed,
    Manual,
}

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Platform {
    #[serde(default)]
    pub features: BTreeSet<AndroidFeature>,
    pub icon: Option<PathBuf>,
    pub notification_icon: Option<PathBuf>,
    #[serde(rename = "recipe")]
    pub _recipe: Option<PathBuf>,
    #[serde(skip)]
    pub bundled: bool,
    #[serde(skip)]
    pub default_android_build: bool,
    #[serde(default)]
    pub management: Management,
    pub template: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub paths: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub copies: Vec<CopySpec>,
    #[serde(default)]
    pub build: Vec<Step>,
    #[serde(default)]
    pub run: Vec<Step>,
    pub artifact: Option<String>,
    pub application_id: Option<String>,
    pub activity: Option<String>,
    #[serde(default)]
    pub abis: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub url_schemes: Vec<String>,
    #[serde(default)]
    pub share_mime_types: Vec<String>,
    pub signing: Option<Signing>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidFeature {
    Files,
    Sharing,
    Credentials,
    Media,
    Notifications,
    MediaNotifications,
}
impl Platform {
    pub fn feature(&self, feature: AndroidFeature) -> bool {
        self.features.contains(&feature)
            || (feature == AndroidFeature::Files
                && self.features.contains(&AndroidFeature::Sharing))
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Signing {
    pub keystore: PathBuf,
    pub key_alias: String,
    pub store_password_env: String,
    pub key_password_env: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopySpec {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Step {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub error_pattern: Option<String>,
}

pub struct Project {
    pub config_path: PathBuf,
    pub root: PathBuf,
    pub name: String,
    pub platforms: BTreeMap<String, Platform>,
}

pub type Variables = BTreeMap<String, String>;

fn read(path: &Path) -> Result<toml::Value> {
    toml::from_str(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))
}

fn paths(value: &mut toml::Value, base: &Path) -> Result<()> {
    fn resolve(value: &mut toml::Value, base: &Path) -> Result<()> {
        let path = value.as_str().context("expected a path string")?;
        *value = toml::Value::String(base.join(path).to_string_lossy().into_owned());
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
    if let Some(table) = value.get_mut("paths").and_then(toml::Value::as_table_mut) {
        for (_, path) in table.iter_mut() {
            resolve(path, base)?;
        }
    }
    if let Some(copies) = value.get_mut("copies").and_then(toml::Value::as_array_mut) {
        for copy in copies {
            resolve(copy.get_mut("from").context("copy is missing from")?, base)?;
        }
    }
    Ok(())
}

fn merge(base: &mut toml::Value, overlay: toml::Value) {
    if let (Some(base), Some(overlay)) = (base.as_table_mut(), overlay.as_table()) {
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
        let config: Config = read(&config_path)?.try_into()?;
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
                toml::from_str(include_str!("../android.toml"))?
            } else {
                toml::Value::Table(toml::Table::new())
            };
            paths(&mut value, &root)?;
            merge(&mut base, value);
            let mut platform: Platform = base
                .try_into()
                .with_context(|| format!("invalid platform {name}"))?;
            platform.bundled = bundled;
            platform.default_android_build = bundled
                && platform.build
                    == toml::from_str::<Platform>(include_str!("../android.toml"))?.build;
            if bundled && !platform.variables.contains_key("native_library") {
                let manifest = read(&root.join("Cargo.toml"))?;
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
                        || platform.feature(AndroidFeature::MediaNotifications),
                    "notification-icon requires notifications or media-notifications"
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
