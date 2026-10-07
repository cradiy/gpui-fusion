use crate::{FileReader, FileWriter, IoExecutor, unsupported};
use anyhow::Result;
use futures::{Stream, StreamExt, future::LocalBoxFuture, stream::BoxStream};
use std::{fmt::Debug, path::Path, sync::Arc, time::SystemTime};

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
pub trait PlatformFile: Debug + Send + Sync {
    fn name(&self) -> &str;
    fn path(&self) -> Option<&Path> {
        None
    }
    /// A browser object URL, kept valid by the handle or its open sessions.
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
    pub fn new(file: Arc<dyn PlatformFile>) -> Self {
        Self(file)
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
