#[cfg(target_os = "linux")]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "synchronous test subprocess isolates desktop trash environment"
)]
fn trash_retains_contents_and_failures_do_not_delete_the_source() {
    use futures::executor::block_on;
    use gpui_io::{FileHandle, IoExecutor};
    use std::{fs, path::PathBuf};

    // Isolate process-global trash discovery from the user's actual desktop trash.
    let Some(root) = std::env::var_os("GPUI_TRASH_TEST_ROOT") else {
        let temp = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "trash_retains_contents_and_failures_do_not_delete_the_source",
                "--nocapture",
            ])
            .env("GPUI_TRASH_TEST_ROOT", temp.path())
            .env("XDG_DATA_HOME", temp.path().join("data"))
            .status()
            .unwrap();
        assert!(status.success());
        return;
    };
    block_on(async {
        let root = PathBuf::from(root);
        let path = root.join("回收.txt");
        fs::write(&path, b"recoverable").unwrap();
        let executor = IoExecutor::new(|work| {
            std::thread::spawn(work);
        });
        let readonly = FileHandle::from_path(&path, executor.clone(), false);
        assert!(!readonly.can_trash().await.unwrap());
        assert!(readonly.trash().await.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"recoverable");
        let file = FileHandle::from_path(&path, executor, true);
        assert!(file.can_trash().await.unwrap());
        file.trash().await.unwrap();
        assert!(!path.exists());
        let trash = root.join("data/Trash");
        let trashed = fs::read_dir(trash.join("files"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(fs::read(&trashed).unwrap(), b"recoverable");
        let info = fs::read_dir(trash.join("info"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(fs::read_to_string(&info).unwrap().contains("[Trash Info]"));
        fs::rename(&trashed, &path).unwrap();
        fs::remove_file(info).unwrap();
        fs::remove_dir(trash.join("files")).unwrap();
        fs::write(trash.join("files"), b"unavailable").unwrap();
        assert!(file.trash().await.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"recoverable");
    });
}
