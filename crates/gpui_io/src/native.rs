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
    fn can_rename(&self) -> LocalBoxFuture<'static, Result<bool>> {
        self.can_delete()
    }
    fn rename(&self, new_name: String) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let path = self.path.clone();
        let writable = self.writable;
        let executor = self.executor.clone();
        self.executor.run(move || {
            anyhow::ensure!(writable, "file handle is read-only");
            anyhow::ensure!(
                std::fs::symlink_metadata(&path)?.file_type().is_file(),
                "only regular files can be renamed"
            );
            let target = path.with_file_name(new_name);
            if target != path {
                rename_without_replacement(&path, &target)?;
            }
            Ok(file(target, executor, writable))
        })
    }
    fn can_delete(&self) -> LocalBoxFuture<'static, Result<bool>> {
        let path = self.path.clone();
        let writable = self.writable;
        self.executor
            .run(move || Ok(writable && std::fs::symlink_metadata(path)?.file_type().is_file()))
    }
    fn delete(&self) -> LocalBoxFuture<'static, Result<()>> {
        let path = self.path.clone();
        let writable = self.writable;
        self.executor.run(move || {
            anyhow::ensure!(writable, "file handle is read-only");
            anyhow::ensure!(
                std::fs::symlink_metadata(&path)?.file_type().is_file(),
                "only regular files can be deleted"
            );
            Ok(std::fs::remove_file(path)?)
        })
    }
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

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
fn rename_without_replacement(from: &Path, to: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn rename_without_replacement(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    // Canonical native prefixes keep long paths usable without process-wide manifest settings.
    let from = std::fs::canonicalize(from)?;
    let parent = to
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let to = std::fs::canonicalize(parent)?.join(to.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing destination filename")
    })?);
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both strings are NUL-terminated and live through the call. Flags omit replacement.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), 0)
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
)))]
fn rename_without_replacement(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "native rename is unavailable",
    ))
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
