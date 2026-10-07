use crate::IoExecutor;
use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

const CHUNK_SIZE: usize = 64 * 1024;

/// Backend for a read session. Return a nonempty chunk of at most `limit` bytes, or `None` at EOF.
pub trait PlatformReader: Send {
    fn read_chunk(&mut self, limit: usize) -> LocalBoxFuture<'_, Result<Option<Vec<u8>>>>;
    fn seek(&mut self, _position: SeekFrom) -> LocalBoxFuture<'_, Result<u64>> {
        Box::pin(async { Err(crate::unsupported("reader does not support seeking")) })
    }
}

pub struct FileReader(Box<dyn PlatformReader>);
impl FileReader {
    pub fn new(reader: impl PlatformReader + 'static) -> Self {
        Self(Box::new(reader))
    }
    pub fn from_blocking(reader: impl Read + Send + 'static, executor: IoExecutor) -> Self {
        Self::new(BlockingReader {
            resource: Resource::new(BlockingInput::Sequential(Box::new(reader)), executor),
        })
    }
    /// Adapt a blocking source that implements `Seek`; the underlying device may still reject it.
    pub fn from_seekable(reader: impl Read + Seek + Send + 'static, executor: IoExecutor) -> Self {
        Self::new(BlockingReader {
            resource: Resource::new(BlockingInput::Seekable(Box::new(reader)), executor),
        })
    }
    /// Move the byte cursor and return its position. Not all document providers support seeking.
    pub async fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        self.0.seek(position).await
    }
    /// Read up to 64 KiB, returning `None` at EOF.
    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>> {
        self.read_chunk_with_limit(CHUNK_SIZE).await
    }
    pub async fn read_chunk_with_limit(&mut self, limit: usize) -> Result<Option<Vec<u8>>> {
        ensure!(limit > 0, "read limit must be nonzero");
        let chunk = self.0.read_chunk(limit).await?;
        if let Some(bytes) = &chunk
            && (bytes.is_empty() || bytes.len() > limit)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "reader returned an invalid chunk size",
            )
            .into());
        }
        Ok(chunk)
    }
    /// Fill the buffer from the current cursor. Early EOF returns `UnexpectedEof`.
    /// Errors can leave the buffer partially filled and the cursor advanced.
    pub async fn read_exact(&mut self, mut buffer: &mut [u8]) -> Result<()> {
        while !buffer.is_empty() {
            let chunk = self
                .read_chunk_with_limit(buffer.len().min(CHUNK_SIZE))
                .await?
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "file ended before the buffer was filled",
                    )
                })?;
            let (filled, remaining) = buffer.split_at_mut(chunk.len());
            filled.copy_from_slice(&chunk);
            buffer = remaining;
        }
        Ok(())
    }
    /// Collect remaining bytes from the current cursor, failing with `FileTooLarge` on overflow.
    /// The overflow check consumes at most `max_bytes + 1` bytes; errors do not rewind the reader.
    pub async fn read_to_end_limited(&mut self, max_bytes: usize) -> Result<Vec<u8>> {
        let mut contents = Vec::new();
        loop {
            let remaining = max_bytes - contents.len();
            let limit = remaining.saturating_add(1).min(CHUNK_SIZE);
            let Some(chunk) = self.read_chunk_with_limit(limit).await? else {
                return Ok(contents);
            };
            if chunk.len() > remaining {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::FileTooLarge,
                    format!("file exceeds the read limit of {max_bytes} bytes"),
                )
                .into());
            }
            contents.extend(chunk);
        }
    }
}

/// Backend for an output session. Dropping an unfinished session must release its resources.
pub trait PlatformWriter: Send {
    fn write_all<'a>(&'a mut self, bytes: &'a [u8]) -> LocalBoxFuture<'a, Result<()>>;
    fn flush(&mut self) -> LocalBoxFuture<'_, Result<()>>;
    fn close(&mut self) -> LocalBoxFuture<'_, Result<()>>;
    fn abort(&mut self) -> LocalBoxFuture<'_, Result<()>>;
}

pub struct FileWriter(Box<dyn PlatformWriter>);
impl FileWriter {
    pub fn new(writer: impl PlatformWriter + 'static) -> Self {
        Self(Box::new(writer))
    }
    pub fn from_blocking(writer: impl BlockingWrite + 'static, executor: IoExecutor) -> Self {
        Self::new(BlockingWriter {
            resource: Resource::new(Box::new(writer), executor),
        })
    }
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.0.write_all(bytes).await
    }
    /// Flush pending bytes; does not guarantee durable storage or remote synchronization.
    pub async fn flush(&mut self) -> Result<()> {
        self.0.flush().await
    }
    /// Finish the session and report provider close errors. Close may publish staged provider data.
    pub async fn close(mut self) -> Result<()> {
        self.0.close().await
    }
    /// Release an unfinished output. Existing files may retain partial contents.
    pub async fn abort(mut self) -> Result<()> {
        self.0.abort().await
    }
}

/// Blocking provider stream with fallible completion and cancellation cleanup.
pub trait BlockingWrite: Write + Send {
    fn close(&mut self) -> Result<()>;
    fn abort(&mut self) -> Result<()> {
        self.close()
    }
}

trait Cleanup: Send {
    fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}
impl Cleanup for BlockingInput {}
impl Cleanup for Box<dyn BlockingWrite> {
    fn cleanup(&mut self) -> Result<()> {
        self.abort()
    }
}

struct Resource<T: Cleanup + 'static> {
    state: Arc<Mutex<Option<T>>>,
    executor: IoExecutor,
}
impl<T: Cleanup + 'static> Resource<T> {
    fn new(value: T, executor: IoExecutor) -> Self {
        Self {
            state: Arc::new(Mutex::new(Some(value))),
            executor,
        }
    }
    fn run<R: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Option<T>) -> Result<R> + Send + 'static,
    ) -> LocalBoxFuture<'static, Result<R>> {
        let state = self.state.clone();
        self.executor.run(move || {
            work(
                &mut *state
                    .lock()
                    .map_err(|_| std::io::Error::other("I/O session poisoned"))?,
            )
        })
    }
}
impl<T: Cleanup + 'static> Drop for Resource<T> {
    fn drop(&mut self) {
        let state = self.state.clone();
        self.executor.dispatch(move || {
            if let Some(mut resource) = state.lock().unwrap_or_else(|e| e.into_inner()).take()
                && let Err(error) = resource.cleanup()
            {
                log::warn!("I/O cleanup failed: {error:#}");
            }
        });
    }
}

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

enum BlockingInput {
    Sequential(Box<dyn Read + Send>),
    Seekable(Box<dyn ReadSeek>),
}
impl Read for BlockingInput {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Sequential(reader) => reader.read(bytes),
            Self::Seekable(reader) => reader.read(bytes),
        }
    }
}

struct BlockingReader {
    resource: Resource<BlockingInput>,
}
impl PlatformReader for BlockingReader {
    fn seek(&mut self, position: SeekFrom) -> LocalBoxFuture<'_, Result<u64>> {
        self.resource.run(move |slot| match slot {
            Some(BlockingInput::Seekable(reader)) => Ok(reader.seek(position)?),
            Some(BlockingInput::Sequential(_)) => {
                Err(crate::unsupported("reader does not support seeking"))
            }
            None => Err(std::io::Error::other("reader closed").into()),
        })
    }
    fn read_chunk(&mut self, limit: usize) -> LocalBoxFuture<'_, Result<Option<Vec<u8>>>> {
        self.resource.run(move |slot| {
            let reader = slot
                .as_mut()
                .ok_or_else(|| std::io::Error::other("reader closed"))?;
            let mut bytes = vec![0; limit];
            let count = loop {
                match reader.read(&mut bytes) {
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    result => break result?,
                }
            };
            bytes.truncate(count);
            Ok((count != 0).then_some(bytes))
        })
    }
}

struct BlockingWriter {
    resource: Resource<Box<dyn BlockingWrite>>,
}
impl PlatformWriter for BlockingWriter {
    fn write_all<'a>(&'a mut self, bytes: &'a [u8]) -> LocalBoxFuture<'a, Result<()>> {
        Box::pin(async move {
            for chunk in bytes.chunks(CHUNK_SIZE) {
                let chunk = chunk.to_vec();
                self.resource
                    .run(move |slot| {
                        let writer = slot
                            .as_mut()
                            .ok_or_else(|| std::io::Error::other("writer closed"))?;
                        if let Err(error) = writer.write_all(&chunk) {
                            if let Some(mut writer) = slot.take()
                                && let Err(cleanup) = writer.abort()
                            {
                                return Err(anyhow::Error::from(error)
                                    .context(format!("output cleanup failed: {cleanup:#}")));
                            }
                            return Err(error.into());
                        }
                        Ok(())
                    })
                    .await?;
            }
            Ok(())
        })
    }
    fn flush(&mut self) -> LocalBoxFuture<'_, Result<()>> {
        self.resource.run(|slot| {
            let writer = slot
                .as_mut()
                .ok_or_else(|| std::io::Error::other("writer closed"))?;
            Ok(writer.flush()?)
        })
    }
    fn close(&mut self) -> LocalBoxFuture<'_, Result<()>> {
        self.resource.run(|slot| {
            let mut writer = slot
                .take()
                .ok_or_else(|| std::io::Error::other("writer closed"))?;
            let result = writer
                .flush()
                .map_err(anyhow::Error::from)
                .and_then(|_| writer.close());
            if let Err(error) = result {
                if let Err(cleanup) = writer.abort() {
                    return Err(error.context(format!("output cleanup failed: {cleanup:#}")));
                }
                return Err(error);
            }
            Ok(())
        })
    }
    fn abort(&mut self) -> LocalBoxFuture<'_, Result<()>> {
        self.resource.run(|slot| {
            if let Some(mut writer) = slot.take() {
                writer.abort()?;
            }
            Ok(())
        })
    }
}
