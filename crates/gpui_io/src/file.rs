use crate::{FileReader, FileWriter, IoExecutor, unsupported};
use anyhow::Result;
use futures::{Stream, StreamExt, future::LocalBoxFuture, stream::BoxStream};
use std::{any::Any, fmt::Debug, path::Path, sync::Arc, time::SystemTime};

/// Serializable, provider-specific file reference. It is not a portable access token.
#[derive(Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileBookmark {
    provider: String,
    data: Vec<u8>,
}

impl FileBookmark {
    pub fn new(provider: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            provider: provider.into(),
            data,
        }
    }
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Debug for FileBookmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileBookmark")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

/// Ordered byte chunks; an error stops the write.
pub type FileWriteStream = BoxStream<'static, Result<Vec<u8>>>;

/// Metadata can be incomplete for remote providers.
#[derive(Clone, Debug, Default)]
pub struct FileMetadata {
    pub byte_len: Option<u64>,
    pub modified: Option<SystemTime>,
    pub mime_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WriteMode {
    /// Replace all contents, including when no bytes are written.
    #[default]
    Truncate,
    /// Append without replacing existing contents. Unsupported providers return an error.
    Append,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WriteOptions {
    pub mode: WriteMode,
}
impl WriteOptions {
    pub fn truncate() -> Self {
        Self {
            mode: WriteMode::Truncate,
        }
    }
    pub fn append() -> Self {
        Self {
            mode: WriteMode::Append,
        }
    }
}

/// Platform resource behind a file handle. Sessions own their resources independently.
pub trait PlatformFile: Any + Debug + Send + Sync {
    fn can_trash(&self) -> LocalBoxFuture<'static, Result<bool>> {
        Box::pin(async { Ok(false) })
    }
    fn trash(&self) -> LocalBoxFuture<'static, Result<()>> {
        Box::pin(async { Err(unsupported("file provider does not support trash")) })
    }
    fn can_rename(&self) -> LocalBoxFuture<'static, Result<bool>> {
        Box::pin(async { Ok(false) })
    }
    fn rename(&self, _new_name: String) -> LocalBoxFuture<'static, Result<FileHandle>> {
        Box::pin(async { Err(unsupported("file provider does not support renaming")) })
    }
    /// Query current support for permanent deletion, independently of content writing.
    fn can_delete(&self) -> LocalBoxFuture<'static, Result<bool>> {
        Box::pin(async { Ok(false) })
    }
    fn delete(&self) -> LocalBoxFuture<'static, Result<()>> {
        Box::pin(async { Err(unsupported("file provider does not support deletion")) })
    }
    fn persist(&self) -> LocalBoxFuture<'static, Result<FileBookmark>> {
        Box::pin(async {
            Err(unsupported(
                "file provider does not support persistent access",
            ))
        })
    }
    fn name(&self) -> &str;
    fn path(&self) -> Option<&Path> {
        None
    }
    /// A provider URL, such as an Android content URI or browser object URL.
    /// Keep the handle alive while using it; the URL does not grant access to other apps.
    fn url(&self) -> Option<&str> {
        None
    }
    fn can_write(&self) -> bool {
        false
    }
    fn metadata(&self) -> LocalBoxFuture<'static, Result<FileMetadata>>;
    fn open_read(&self) -> LocalBoxFuture<'static, Result<FileReader>>;
    fn open_write(&self, _options: WriteOptions) -> LocalBoxFuture<'static, Result<FileWriter>> {
        Box::pin(async { Err(unsupported("file handle does not support writing")) })
    }
}

/// A file reference and its access rights; cloning does not share a stream position.
#[derive(Clone, Debug)]
pub struct FileHandle(Arc<dyn PlatformFile>);

impl FileHandle {
    /// Query trash support, independently of permanent deletion. Later access can still fail.
    pub fn can_trash(&self) -> LocalBoxFuture<'static, Result<bool>> {
        self.0.can_trash()
    }
    /// Move to the system/provider trash. Never falls back to permanent deletion.
    /// Retention and restoration belong to the system/provider. Close active I/O first.
    pub fn trash(&self) -> LocalBoxFuture<'static, Result<()>> {
        self.0.trash()
    }
    pub fn can_rename(&self) -> LocalBoxFuture<'static, Result<bool>> {
        self.0.can_rename()
    }
    /// Rename within the same directory. Use the returned handle, whose name or URI may differ.
    /// Native paths reject an existing target; document providers determine name conflicts.
    pub async fn rename(&self, new_name: impl Into<String>) -> Result<FileHandle> {
        let new_name = new_name.into();
        crate::location::validate_name(&new_name)?;
        self.0.rename(new_name).await
    }
    /// Query deletion support. A later delete can still fail if access changes.
    pub fn can_delete(&self) -> LocalBoxFuture<'static, Result<bool>> {
        self.0.can_delete()
    }
    /// Permanently delete this file. Does not recursively delete directories or use trash.
    pub fn delete(&self) -> LocalBoxFuture<'static, Result<()>> {
        self.0.delete()
    }
    /// Retain access explicitly. The application must store the returned bookmark.
    /// Dropping a handle or bookmark does not release persistent permissions.
    pub fn persist(&self) -> LocalBoxFuture<'static, Result<FileBookmark>> {
        self.0.persist()
    }
    pub fn new(file: Arc<dyn PlatformFile>) -> Self {
        Self(file)
    }
    /// Access the provider for platform integration without exposing its resource identifier.
    pub fn downcast_ref<T: PlatformFile>(&self) -> Option<&T> {
        (self.0.as_ref() as &dyn Any).downcast_ref()
    }
    pub fn name(&self) -> &str {
        self.0.name()
    }
    pub fn path(&self) -> Option<&Path> {
        self.0.path()
    }
    pub fn url(&self) -> Option<&str> {
        self.0.url()
    }
    pub fn can_write(&self) -> bool {
        self.0.can_write()
    }
    pub fn metadata(&self) -> LocalBoxFuture<'static, Result<FileMetadata>> {
        self.0.metadata()
    }
    pub fn open_read(&self) -> LocalBoxFuture<'static, Result<FileReader>> {
        self.0.open_read()
    }
    pub fn open_write(&self, options: WriteOptions) -> LocalBoxFuture<'static, Result<FileWriter>> {
        self.0.open_write(options)
    }

    /// Read the complete file. Use a reader session for bounded-memory access.
    pub fn read(&self) -> LocalBoxFuture<'static, Result<Vec<u8>>> {
        let open = self.open_read();
        Box::pin(async move {
            let mut reader = open.await?;
            let mut contents = Vec::new();
            while let Some(chunk) = reader.read_chunk().await? {
                contents.extend(chunk);
            }
            Ok(contents)
        })
    }
    /// Read the complete file, failing if its contents exceed `max_bytes`.
    /// Reads at most one extra byte to detect overflow; does not trust provider metadata.
    pub fn read_limited(&self, max_bytes: usize) -> LocalBoxFuture<'static, Result<Vec<u8>>> {
        let open = self.open_read();
        Box::pin(async move { open.await?.read_to_end_limited(max_bytes).await })
    }
    pub fn write(&self, contents: Vec<u8>) -> LocalBoxFuture<'static, Result<()>> {
        self.write_stream(futures::stream::once(async { Ok(contents) }))
    }
    /// Replace contents incrementally; errors or cancellation may leave partial contents.
    /// Await completion before starting another save to the same file.
    pub fn write_stream(
        &self,
        contents: impl Stream<Item = Result<Vec<u8>>> + Send + 'static,
    ) -> LocalBoxFuture<'static, Result<()>> {
        let open = self.open_write(WriteOptions::truncate());
        Box::pin(async move {
            let mut writer = open.await?;
            let mut contents = Box::pin(contents);
            let result: Result<()> = async {
                while let Some(chunk) = contents.next().await {
                    writer.write_all(&chunk?).await?;
                }
                Ok(())
            }
            .await;
            match result {
                Ok(()) => writer.close().await,
                Err(error) => {
                    if let Err(cleanup) = writer.abort().await {
                        return Err(error.context(format!("output cleanup failed: {cleanup:#}")));
                    }
                    Err(error)
                }
            }
        })
    }

    /// Reference a native path without opening it. Writes may create the file, but not its parent.
    pub fn from_path(
        path: impl Into<std::path::PathBuf>,
        executor: impl Into<IoExecutor>,
        writable: bool,
    ) -> Self {
        crate::native::file(path.into(), executor.into(), writable)
    }
}
