use futures::executor::block_on;
use gpui_io::{CreateOptions, DirectoryEntryKind, IoExecutor, LocationHandle};

#[test]
fn listings_and_lookup_preserve_names_and_reject_missing_or_wrong_kind() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let root = LocationHandle::from_path(
            temp.path(),
            IoExecutor::new(|work| {
                std::thread::spawn(work);
            }),
        );
        let file = root
            .create_file("资料/note.txt", CreateOptions::default())
            .await
            .unwrap();
        file.write(b"existing".to_vec()).await.unwrap();
        let entries = root.read_dir("").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, DirectoryEntryKind::Directory);
        let entries = root.read_dir(&entries[0].name).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, DirectoryEntryKind::File);
        let reopened = root
            .open_file(std::path::Path::new("资料").join(&entries[0].name))
            .await
            .unwrap();
        assert_eq!(reopened.read().await.unwrap(), b"existing");
        for invalid in [
            "../escape",
            "/absolute",
            "资料//note.txt",
            "资料/./note.txt",
            "资料/../note.txt",
        ] {
            assert!(root.open_file(invalid).await.is_err());
            assert!(root.read_dir(invalid).await.is_err());
        }
        assert!(root.open_file("").await.is_err());
        assert!(root.open_file("资料").await.is_err());
        assert!(root.read_dir("资料/note.txt").await.is_err());
        assert!(root.open_file("missing").await.is_err());
        assert!(root.read_dir("missing").await.is_err());
        assert!(!temp.path().join("missing").exists());

        #[cfg(unix)]
        {
            use std::os::unix::{ffi::OsStringExt, fs::symlink};
            let name = std::ffi::OsString::from_vec(b"raw-\xff".to_vec());
            std::fs::write(temp.path().join(&name), b"raw").unwrap();
            symlink("资料", temp.path().join("link")).unwrap();
            let entries = root.read_dir("").await.unwrap();
            let raw = entries.iter().find(|entry| entry.name == name).unwrap();
            assert_eq!(
                root.open_file(&raw.name)
                    .await
                    .unwrap()
                    .read()
                    .await
                    .unwrap(),
                b"raw"
            );
            assert_eq!(
                entries
                    .iter()
                    .find(|entry| entry.name == "link")
                    .unwrap()
                    .kind,
                DirectoryEntryKind::Symlink
            );
            assert_eq!(root.read_dir("link").await.unwrap().len(), 1);
        }
    });
}
