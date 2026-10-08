use futures::{executor::block_on, future::LocalBoxFuture};
use gpui_io::{
    CreateOptions, FileHandle, FileMetadata, FileReader, IoExecutor, LocationHandle, PlatformFile,
    PlatformReader, TransferStage,
};
use std::{io, sync::Arc};

fn executor() -> IoExecutor {
    IoExecutor::new(|work| {
        std::thread::spawn(work);
    })
}

#[test]
fn transfers_preserve_contents_and_never_replace_existing_files() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let root = LocationHandle::from_path(temp.path(), executor());
        let source = root
            .create_file("source.bin", CreateOptions::default())
            .await
            .unwrap();
        let contents = (0..200_000).map(|n| (n % 251) as u8).collect::<Vec<_>>();
        source.write(contents.clone()).await.unwrap();
        let copy = source
            .copy_to(&root, "中文/sub/copy.bin", CreateOptions::default())
            .await
            .unwrap();
        assert_eq!(copy.read().await.unwrap(), contents);
        assert_eq!(source.read().await.unwrap(), contents);
        assert!(
            source
                .copy_to(&root, "source.bin", CreateOptions::default())
                .await
                .is_err()
        );
        assert!(
            source
                .move_to(&root, "中文/sub/copy.bin", CreateOptions::default())
                .await
                .is_err()
        );
        assert_eq!(source.read().await.unwrap(), contents);
        assert_eq!(copy.read().await.unwrap(), contents);
        let readonly = FileHandle::from_path(source.path().unwrap(), executor(), false);
        assert!(!readonly.can_delete().await.unwrap());
        assert!(
            readonly
                .move_to(&root, "forbidden.bin", CreateOptions::default())
                .await
                .is_err()
        );
        assert!(!temp.path().join("forbidden.bin").exists());
        let moved = source
            .move_to(&root, "moved/file.bin", CreateOptions::default())
            .await
            .unwrap();
        assert!(!temp.path().join("source.bin").exists());
        assert_eq!(moved.read().await.unwrap(), contents);
        assert!(moved.can_rename().await.unwrap());
        let occupied = root
            .create_file("moved/existing.bin", CreateOptions::default())
            .await
            .unwrap();
        occupied.write(b"keep".to_vec()).await.unwrap();
        assert!(moved.rename("existing.bin").await.is_err());
        assert!(moved.rename("../escape.bin").await.is_err());
        assert_eq!(occupied.read().await.unwrap(), b"keep");
        let renamed = moved.rename("重命名.bin").await.unwrap();
        assert_eq!(renamed.name(), "重命名.bin");
        assert_eq!(renamed.read().await.unwrap(), contents);
        renamed.delete().await.unwrap();
        assert!(!temp.path().join("moved/file.bin").exists());
        assert_eq!(copy.read().await.unwrap(), contents);
    });
}

#[derive(Debug)]
struct FaultySource {
    file: FileHandle,
    fail_read: bool,
}

impl PlatformFile for FaultySource {
    fn name(&self) -> &str {
        "source"
    }
    fn metadata(&self) -> LocalBoxFuture<'static, anyhow::Result<FileMetadata>> {
        self.file.metadata()
    }
    fn can_delete(&self) -> LocalBoxFuture<'static, anyhow::Result<bool>> {
        Box::pin(async { Ok(true) })
    }
    fn delete(&self) -> LocalBoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "access revoked").into())
        })
    }
    fn open_read(&self) -> LocalBoxFuture<'static, anyhow::Result<FileReader>> {
        if self.fail_read {
            Box::pin(async { Ok(FileReader::new(FailingReader(false))) })
        } else {
            self.file.open_read()
        }
    }
}

struct FailingReader(bool);
impl PlatformReader for FailingReader {
    fn read_chunk(&mut self, _: usize) -> LocalBoxFuture<'_, anyhow::Result<Option<Vec<u8>>>> {
        Box::pin(async move {
            if std::mem::replace(&mut self.0, true) {
                Err(io::Error::other("source disconnected").into())
            } else {
                Ok(Some(b"partial".to_vec()))
            }
        })
    }
}

#[test]
fn failed_copy_removes_partial_output_but_failed_source_delete_keeps_complete_copy() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let root = LocationHandle::from_path(temp.path(), executor());
        let original = root
            .create_file("source", CreateOptions::default())
            .await
            .unwrap();
        original.write(b"complete".to_vec()).await.unwrap();
        let broken = FileHandle::new(Arc::new(FaultySource {
            file: original.clone(),
            fail_read: true,
        }));
        let error = broken
            .move_to(&root, "partial", CreateOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.stage, TransferStage::Copy);
        assert!(error.destination.is_none());
        assert!(!temp.path().join("partial").exists());
        assert_eq!(original.read().await.unwrap(), b"complete");
        let revoked = FileHandle::new(Arc::new(FaultySource {
            file: original.clone(),
            fail_read: false,
        }));
        let error = revoked
            .move_to(&root, "complete-copy", CreateOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.stage, TransferStage::DeleteSource);
        assert_eq!(
            error.destination.unwrap().read().await.unwrap(),
            b"complete"
        );
        assert_eq!(original.read().await.unwrap(), b"complete");
    });
}
