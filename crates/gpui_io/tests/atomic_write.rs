#![cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
))]

use futures::executor::block_on;
use gpui_io::{FileHandle, IoExecutor, WriteOptions};
use std::fs;

fn executor() -> IoExecutor {
    // Complete queued work inline so drop cleanup is observable without timing assumptions.
    IoExecutor::new(|work| work())
}

#[test]
fn atomic_sessions_publish_only_on_close_and_clean_up_abandoned_writes() {
    block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, b"original").unwrap();
        let file = FileHandle::from_path(&path, executor(), true);
        assert!(file.can_write_atomically());

        for abort in [true, false] {
            let mut writer = file
                .open_write(WriteOptions::atomic_replace())
                .await
                .unwrap();
            writer.write_all(b"partial").await.unwrap();
            writer.flush().await.unwrap();
            assert_eq!(fs::read(&path).unwrap(), b"original");
            if abort {
                writer.abort().await.unwrap();
            } else {
                drop(writer);
            }
            assert_eq!(fs::read(&path).unwrap(), b"original");
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }

        let mut writer = file
            .open_write(WriteOptions::atomic_replace())
            .await
            .unwrap();
        writer.write_all(b"complete ").await.unwrap();
        writer.write_all(b"replacement").await.unwrap();
        assert_eq!(file.read().await.unwrap(), b"original");
        writer.close().await.unwrap();
        assert_eq!(file.read().await.unwrap(), b"complete replacement");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

        file.open_write(WriteOptions::atomic_replace())
            .await
            .unwrap()
            .close()
            .await
            .unwrap();
        assert!(file.read().await.unwrap().is_empty());
        fs::remove_file(&path).unwrap();
        let mut writer = file
            .open_write(WriteOptions::atomic_replace())
            .await
            .unwrap();
        writer.write_all(b"new").await.unwrap();
        assert!(!path.exists());
        writer.close().await.unwrap();
        assert_eq!(file.read().await.unwrap(), b"new");
    });
}

#[test]
fn failed_atomic_close_preserves_destination_and_removes_staging_file() {
    block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("data");
        fs::write(&path, b"original").unwrap();
        let file = FileHandle::from_path(&path, executor(), true);
        let mut writer = file
            .open_write(WriteOptions::atomic_replace())
            .await
            .unwrap();
        writer.write_all(b"replacement").await.unwrap();
        // Another actor replaces the destination while the session is open.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"untouched").unwrap();
        assert!(writer.close().await.is_err());
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

        let readonly = FileHandle::from_path(path.join("keep"), executor(), false);
        assert!(!readonly.can_write_atomically());
        assert!(
            readonly
                .open_write(WriteOptions::atomic_replace())
                .await
                .is_err()
        );
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"untouched");
    });
}

#[cfg(unix)]
#[test]
fn atomic_replacement_preserves_permissions_and_rejects_symlinks() {
    use std::{
        io,
        os::unix::fs::{PermissionsExt, symlink},
    };
    block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("data");
        fs::write(&path, b"original").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let link = directory.path().join("link");
        symlink(&path, &link).unwrap();
        let linked = FileHandle::from_path(&link, executor(), true);
        assert!(
            linked
                .open_write(WriteOptions::atomic_replace())
                .await
                .is_err()
        );
        assert!(link.is_symlink());
        assert_eq!(fs::read(&path).unwrap(), b"original");

        let file = FileHandle::from_path(&path, executor(), true);
        let mut writer = file
            .open_write(WriteOptions::atomic_replace())
            .await
            .unwrap();
        writer.write_all(b"new").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        let error = file
            .open_write(WriteOptions::atomic_replace())
            .await
            .err()
            .unwrap();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(&path).unwrap(), b"new");
    });
}
