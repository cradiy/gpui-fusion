use crate::{FileBookmark, FileHandle, IoExecutor, unsupported};
use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use std::{path::Path, sync::Arc};

/// Storage purpose. Availability and authorization are platform-dependent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemLocation {
    AppData,
    AppConfig,
    Cache,
    Downloads,
    Documents,
    Pictures,
    Music,
    Videos,
}

/// Metadata used when creating a file. Providers may require a concrete MIME type.
#[derive(Clone, Debug, Default)]
pub struct CreateOptions {
    pub mime_type: Option<String>,
}

/// A location can be a filesystem directory or a platform collection.
pub trait PlatformLocation: Send + Sync {
    fn path(&self) -> Option<&Path> {
        None
    }
    /// Resolve a named file without creating it; not all providers support name lookup.
    fn file(&self, _name: &str) -> Result<FileHandle> {
        Err(unsupported("location cannot resolve files by name"))
    }
    /// Create without overwriting existing files. Providers may choose a different display name.
    fn create_file(
        &self,
        name: String,
        options: CreateOptions,
    ) -> LocalBoxFuture<'static, Result<FileHandle>>;
}

#[derive(Clone)]
pub struct LocationHandle(Arc<dyn PlatformLocation>);
impl LocationHandle {
    pub fn new(location: impl PlatformLocation + 'static) -> Self {
        Self(Arc::new(location))
    }
    pub fn path(&self) -> Option<&Path> {
        self.0.path()
    }
    pub fn file(&self, name: &str) -> Result<FileHandle> {
        validate_name(name)?;
        self.0.file(name)
    }
    pub async fn create_file(
        &self,
        name: impl Into<String>,
        options: CreateOptions,
    ) -> Result<FileHandle> {
        let name = name.into();
        validate_name(&name)?;
        self.0.create_file(name, options).await
    }
    pub fn from_path(path: impl Into<std::path::PathBuf>, executor: impl Into<IoExecutor>) -> Self {
        crate::native::location(path.into(), executor.into())
    }
}

/// Location discovery performs no permission prompts or fallback to a different destination.
pub trait PlatformLocations: Send + Sync {
    fn restore_file(&self, _bookmark: FileBookmark) -> LocalBoxFuture<'static, Result<FileHandle>> {
        Box::pin(async { Err(unsupported("file bookmark provider is unavailable")) })
    }
    fn release_file(&self, _bookmark: FileBookmark) -> LocalBoxFuture<'static, Result<()>> {
        Box::pin(async { Err(unsupported("file bookmark provider is unavailable")) })
    }
    fn location(&self, kind: SystemLocation) -> LocalBoxFuture<'static, Result<LocationHandle>>;
}

#[derive(Clone)]
pub struct FileSystem(Arc<dyn PlatformLocations>);
impl FileSystem {
    /// Restore a file without opening a picker. Missing files or lost grants return errors.
    pub fn restore_file(
        &self,
        bookmark: &FileBookmark,
    ) -> LocalBoxFuture<'static, Result<FileHandle>> {
        self.0.restore_file(bookmark.clone())
    }
    /// Release the bookmark's persistent permissions without deleting the file.
    /// Grants can be shared by other handles or bookmarks within the application.
    pub fn release_file(&self, bookmark: &FileBookmark) -> LocalBoxFuture<'static, Result<()>> {
        self.0.release_file(bookmark.clone())
    }
    pub fn new(locations: impl PlatformLocations + 'static) -> Self {
        Self(Arc::new(locations))
    }
    pub async fn location(&self, kind: SystemLocation) -> Result<LocationHandle> {
        self.0.location(kind).await
    }

    /// Desktop directory discovery through `dirs`; `app_id` is a stable directory component.
    pub fn desktop(app_id: &str, executor: impl Into<IoExecutor>) -> Result<Self> {
        validate_app_id(app_id)?;
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        {
            Ok(Self::new(DesktopLocations {
                app_id: app_id.into(),
                executor: executor.into(),
            }))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = executor;
            Err(unsupported(
                "desktop locations unavailable on this platform",
            ))
        }
    }
}

/// Validate an app-specific directory component, independently of a display name.
pub fn validate_app_id(app_id: &str) -> Result<()> {
    validate_name(app_id)?;
    ensure!(
        app_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
        "invalid application identifier"
    );
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.ends_with(['.', ' '])
            && !name.contains(['/', '\\', '\0', ':']),
        "expected a filename, not a path"
    );
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
struct DesktopLocations {
    app_id: String,
    executor: IoExecutor,
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
impl PlatformLocations for DesktopLocations {
    fn location(&self, kind: SystemLocation) -> LocalBoxFuture<'static, Result<LocationHandle>> {
        let app_id = self.app_id.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let root = match kind {
                SystemLocation::AppData => dirs::data_local_dir(),
                SystemLocation::AppConfig => dirs::config_local_dir(),
                SystemLocation::Cache => dirs::cache_dir(),
                SystemLocation::Downloads => dirs::download_dir(),
                SystemLocation::Documents => dirs::document_dir(),
                SystemLocation::Pictures => dirs::picture_dir(),
                SystemLocation::Music => dirs::audio_dir(),
                SystemLocation::Videos => dirs::video_dir(),
            }
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "system location unavailable")
            })?;
            let path = match kind {
                SystemLocation::AppData | SystemLocation::AppConfig | SystemLocation::Cache => {
                    let path = root.join(&app_id);
                    // These platforms share one root for multiple storage purposes.
                    #[cfg(target_os = "windows")]
                    let path = path.join(match kind {
                        SystemLocation::AppData => "Data",
                        SystemLocation::AppConfig => "Config",
                        _ => "Cache",
                    });
                    #[cfg(target_os = "macos")]
                    let path = match kind {
                        SystemLocation::AppData => path.join("Data"),
                        SystemLocation::AppConfig => path.join("Config"),
                        _ => path,
                    };
                    path
                }
                _ => root,
            };
            Ok(LocationHandle::from_path(path, executor))
        })
    }
}
