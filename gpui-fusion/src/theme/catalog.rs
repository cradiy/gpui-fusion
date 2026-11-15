use crate::theme::{StyleError, ThemeConfig};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

/// One JSON theme registered in a catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeEntry {
    pub name: String,
    pub file: PathBuf,
}

/// A catalog's index.toml. Selection persistence belongs to the calling application.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeIndex {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub themes: BTreeMap<String, ThemeEntry>,
}

impl ThemeIndex {
    pub fn from_toml(source: &str) -> Result<Self, StyleError> {
        let index: Self =
            toml::from_str(source).map_err(|e| StyleError::new("index.toml", e.to_string()))?;
        for (id, entry) in &index.themes {
            if id.trim().is_empty() || entry.name.trim().is_empty() {
                return Err(StyleError::new(id, "theme ID and name must not be empty"));
            }
            if entry.file.as_os_str().is_empty()
                || entry
                    .file
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
            {
                return Err(StyleError::new(
                    id,
                    "theme file must be a relative path without parent components",
                ));
            }
        }
        if let Some(id) = &index.default
            && !index.themes.contains_key(id)
        {
            return Err(StyleError::new(id, "default theme is not registered"));
        }
        Ok(index)
    }
}

/// A loaded index. Reloading the catalog observes changes to its registrations.
#[derive(Clone, Debug)]
pub struct ThemeCatalog {
    directory: PathBuf,
    index: ThemeIndex,
}

impl ThemeCatalog {
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, StyleError> {
        let directory = directory.into();
        Self::open_optional(directory.clone())?
            .ok_or_else(|| StyleError::new(directory.display().to_string(), "missing index.toml"))
    }

    fn open_optional(directory: PathBuf) -> Result<Option<Self>, StyleError> {
        let path = directory.join("index.toml");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(StyleError::new(
                    path.display().to_string(),
                    error.to_string(),
                ));
            }
        };
        let index = ThemeIndex::from_toml(&source)
            .map_err(|e| StyleError::new(path.display().to_string(), e.to_string()))?;
        Ok(Some(Self { directory, index }))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn index(&self) -> &ThemeIndex {
        &self.index
    }

    pub fn theme_path(&self, id: &str) -> Result<PathBuf, StyleError> {
        let entry = self
            .index
            .themes
            .get(id)
            .ok_or_else(|| StyleError::new(id, "theme is not registered"))?;
        Ok(self.directory.join(&entry.file))
    }

    pub fn load(&self, id: &str) -> Result<ThemeConfig, StyleError> {
        ThemeConfig::from_file(self.theme_path(id)?)
    }
}

/// A selection policy, independent of whether file changes are watched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    /// Use the application's catalog default, otherwise the global catalog default.
    Follow { app_id: String },
    /// Pin a theme in the application's catalog, ignoring global selection changes.
    Application { app_id: String, theme_id: String },
    /// Pin a theme in the global catalog, ignoring its default selection changes.
    Global { theme_id: String },
}

#[derive(Clone, Debug)]
pub struct ThemeRepository {
    root: PathBuf,
}

impl ThemeRepository {
    /// Uses GPUI_FUSION_THEME, otherwise the system configuration directory/org.gpui.fusion.
    pub fn discover() -> Result<Self, StyleError> {
        Self::from_root(root_directory(
            std::env::var_os("GPUI_FUSION_THEME"),
            dirs::config_dir,
        )?)
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Result<Self, StyleError> {
        let root = root.into();
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .map_err(|e| StyleError::new("theme directory", e.to_string()))?
                .join(root)
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn application_directory(&self, app_id: &str) -> Result<PathBuf, StyleError> {
        let mut parts = Path::new(app_id).components();
        if app_id.trim().is_empty()
            || app_id.contains(['/', '\\'])
            || !matches!(parts.next(), Some(Component::Normal(_)))
            || parts.next().is_some()
        {
            return Err(StyleError::new(
                "app_id",
                "expected one nonempty directory name",
            ));
        }
        Ok(self.root.join(app_id))
    }

    pub fn application_catalog(&self, app_id: &str) -> Result<ThemeCatalog, StyleError> {
        ThemeCatalog::open(self.application_directory(app_id)?)
    }

    pub fn global_catalog(&self) -> Result<ThemeCatalog, StyleError> {
        ThemeCatalog::open(self.root.clone())
    }

    pub fn load(&self, source: &ThemeSource) -> Result<ThemeConfig, StyleError> {
        let (catalog, id) = match source {
            ThemeSource::Application { app_id, theme_id } => {
                (self.application_catalog(app_id)?, theme_id.clone())
            }
            ThemeSource::Global { theme_id } => (self.global_catalog()?, theme_id.clone()),
            ThemeSource::Follow { app_id } => {
                let directory = self.application_directory(app_id)?;
                let application = ThemeCatalog::open_optional(directory)?;
                if let Some(catalog) = application.filter(|c| c.index.default.is_some()) {
                    let id = catalog.index.default.clone().unwrap();
                    (catalog, id)
                } else {
                    let global = ThemeCatalog::open_optional(self.root.clone())?;
                    if let Some(catalog) = global.filter(|c| c.index.default.is_some()) {
                        let id = catalog.index.default.clone().unwrap();
                        (catalog, id)
                    } else {
                        return Ok(ThemeConfig {
                            theme: crate::default_theme(),
                            mode: "light".into(),
                            style_set: "neutral".into(),
                        });
                    }
                }
            }
        };
        catalog.load(&id)
    }
}

fn root_directory(
    override_root: Option<std::ffi::OsString>,
    config_dir: impl FnOnce() -> Option<PathBuf>,
) -> Result<PathBuf, StyleError> {
    if let Some(path) = override_root.filter(|value| !value.is_empty()) {
        return Ok(path.into());
    }
    config_dir()
        .map(|path| path.join("org.gpui.fusion"))
        .ok_or_else(|| {
            StyleError::new(
                "theme directory",
                "system configuration directory is unavailable",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_override_does_not_require_a_system_config_directory() {
        let root = PathBuf::from("theme-root");
        assert_eq!(
            root_directory(Some(root.clone().into_os_string()), || None).unwrap(),
            root
        );
        assert_eq!(
            root_directory(Some("".into()), || Some(root.clone())).unwrap(),
            root.join("org.gpui.fusion")
        );
        assert!(root_directory(None, || None).is_err());
    }
}
