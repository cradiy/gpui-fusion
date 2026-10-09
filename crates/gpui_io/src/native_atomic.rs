use crate::BlockingWrite;
use anyhow::Result;
use std::{
    fs::Permissions,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

pub(super) struct AtomicWriter {
    staged: Option<NamedTempFile>,
    target: PathBuf,
}

impl AtomicWriter {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let target = std::path::absolute(path)?;
        target_permissions(&target)?;
        let parent = target.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory")
        })?;
        let staged = tempfile::Builder::new()
            .prefix(".gpui-write-")
            .tempfile_in(parent)?;
        Ok(Self {
            staged: Some(staged),
            target,
        })
    }

    fn staged(&mut self) -> io::Result<&mut NamedTempFile> {
        self.staged
            .as_mut()
            .ok_or_else(|| io::Error::other("writer closed"))
    }
}

impl Write for AtomicWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.staged()?.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.staged()?.flush()
    }
}

impl BlockingWrite for AtomicWriter {
    fn close(&mut self) -> Result<()> {
        if let Some(permissions) = target_permissions(&self.target)? {
            self.staged()?.as_file().set_permissions(permissions)?;
        }
        self.staged()?.as_file().sync_all()?;
        let staged = self
            .staged
            .take()
            .ok_or_else(|| io::Error::other("writer closed"))?;
        match staged.persist(&self.target) {
            Ok(_) => Ok(()),
            Err(error) => {
                self.staged = Some(error.file);
                Err(error.error.into())
            }
        }
    }

    fn abort(&mut self) -> Result<()> {
        if let Some(staged) = self.staged.take() {
            staged.close()?;
        }
        Ok(())
    }
}

fn target_permissions(path: &Path) -> io::Result<Option<Permissions>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "atomic replacement requires a regular file, not a directory or symbolic link",
                ));
            }
            let permissions = metadata.permissions();
            if permissions.readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "file is read-only",
                ));
            }
            Ok(Some(permissions))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
