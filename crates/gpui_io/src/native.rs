use crate::{
    BlockingWrite, CreateOptions, FileHandle, FileMetadata, FileReader, FileWriter, IoExecutor,
    LocationHandle, PlatformFile, PlatformLocation, WriteMode, WriteOptions,
};
use anyhow::Result;
use futures::future::LocalBoxFuture;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) fn file(path: PathBuf, executor: IoExecutor, writable: bool) -> FileHandle {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    FileHandle::new(Arc::new(NativeFile {
        path,
        name,
        executor,
        writable,
    }))
}
struct NativeFile {
    path: PathBuf,
    name: String,
    executor: IoExecutor,
    writable: bool,
}
impl std::fmt::Debug for NativeFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("NativeFile").field(&self.path).finish()
    }
}
impl PlatformFile for NativeFile {
    fn name(&self) -> &str {
        &self.name
    }
    fn path(&self) -> Option<&Path> {
        Some(&self.path)
    }
    fn can_write(&self) -> bool {
        self.writable
    }
    fn metadata(&self) -> LocalBoxFuture<'static, Result<FileMetadata>> {
        let path = self.path.clone();
        self.executor.run(move || {
            let metadata = std::fs::metadata(path)?;
            Ok(FileMetadata {
                byte_len: Some(metadata.len()),
                modified: metadata.modified().ok(),
                mime_type: None,
            })
        })
    }
    fn open_read(&self) -> LocalBoxFuture<'static, Result<FileReader>> {
        let path = self.path.clone();
        let executor = self.executor.clone();
        self.executor
            .run(move || Ok(FileReader::from_seekable(File::open(path)?, executor)))
    }
    fn open_write(&self, options: WriteOptions) -> LocalBoxFuture<'static, Result<FileWriter>> {
        if !self.writable {
            return Box::pin(async {
                Err(
                    io::Error::new(io::ErrorKind::PermissionDenied, "file handle is read-only")
                        .into(),
                )
            });
        }
        let path = self.path.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(options.mode == WriteMode::Truncate)
                .append(options.mode == WriteMode::Append)
                .open(path)?;
            Ok(FileWriter::from_blocking(NativeWriter(file), executor))
        })
    }
}

struct NativeWriter(File);
impl Write for NativeWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl BlockingWrite for NativeWriter {
    fn close(&mut self) -> Result<()> {
        Ok(self.0.flush()?)
    }
}

pub(crate) fn location(path: PathBuf, executor: IoExecutor) -> LocationHandle {
    LocationHandle::new(NativeLocation { path, executor })
}
struct NativeLocation {
    path: PathBuf,
    executor: IoExecutor,
}
impl PlatformLocation for NativeLocation {
    fn persist(&self) -> LocalBoxFuture<'static, Result<crate::LocationBookmark>> {
        let path = self.path.clone();
        self.executor.run(move || {
            anyhow::ensure!(path.is_dir(), "directory is missing");
            Ok(crate::LocationBookmark::from_path(std::path::absolute(
                path,
            )?))
        })
    }
    fn path(&self) -> Option<&Path> {
        Some(&self.path)
    }
    fn file(&self, relative_path: &str) -> Result<FileHandle> {
        Ok(file(
            self.path.join(relative_path),
            self.executor.clone(),
            true,
        ))
    }
    fn create_file(
        &self,
        relative_path: String,
        _options: CreateOptions,
    ) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let root = self.path.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let (parent, name) = relative_path
                .rsplit_once('/')
                .unwrap_or(("", &relative_path));
            let root = root.join(parent);
            std::fs::create_dir_all(&root)?;
            let path = root.join(name);
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            Ok(file(path, executor, true))
        })
    }
}
