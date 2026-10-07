use futures::executor::block_on;
use gpui_io::{
    BlockingWrite, CreateOptions, FileHandle, FileWriter, IoExecutor, LocationHandle, WriteOptions,
};
use std::{
    io::{self, Write},
    sync::mpsc,
    time::Duration,
};

fn executor() -> IoExecutor {
    IoExecutor::new(|work| {
        std::thread::spawn(work);
    })
}

#[test]
fn native_locations_preserve_files_and_reader_positions() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let executor = executor();
        let location = LocationHandle::from_path(temp.path().join("data"), executor.clone());
        assert!(location.file("../outside").is_err());
        assert!(location.file(".. ").is_err());
        let file = location
            .create_file("中文.bin", CreateOptions::default())
            .await
            .unwrap();
        file.write(b"abcdef".to_vec()).await.unwrap();
        assert!(
            location
                .create_file("中文.bin", CreateOptions::default())
                .await
                .is_err()
        );
        assert_eq!(file.read().await.unwrap(), b"abcdef");
        assert_eq!(file.metadata().await.unwrap().byte_len, Some(6));
        let mut first = file.open_read().await.unwrap();
        let mut second = file.clone().open_read().await.unwrap();
        assert_eq!(
            first.read_chunk_with_limit(2).await.unwrap().unwrap(),
            b"ab"
        );
        assert_eq!(
            first.read_chunk_with_limit(2).await.unwrap().unwrap(),
            b"cd"
        );
        assert_eq!(
            second.read_chunk_with_limit(2).await.unwrap().unwrap(),
            b"ab"
        );
        drop((first, second));
        let mut writer = file.open_write(WriteOptions::append()).await.unwrap();
        writer.write_all(b"gh").await.unwrap();
        writer.close().await.unwrap();
        assert_eq!(file.read().await.unwrap(), b"abcdefgh");
        file.open_write(WriteOptions::truncate())
            .await
            .unwrap()
            .close()
            .await
            .unwrap();
        assert!(file.read().await.unwrap().is_empty());
        let readonly = FileHandle::from_path(file.path().unwrap(), executor, false);
        assert!(readonly.open_write(WriteOptions::truncate()).await.is_err());
    });
}

struct Output {
    events: mpsc::Sender<&'static str>,
    fail_close: bool,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl BlockingWrite for Output {
    fn close(&mut self) -> anyhow::Result<()> {
        self.events.send("close").unwrap();
        if self.fail_close {
            anyhow::bail!("provider rejected close");
        }
        Ok(())
    }
    fn abort(&mut self) -> anyhow::Result<()> {
        self.events.send("abort").unwrap();
        Ok(())
    }
}

#[test]
fn outputs_close_explicitly_and_abort_on_failure_or_drop() {
    for (finish, fail_close, expected) in [
        (true, false, vec!["close"]),
        (true, true, vec!["close", "abort"]),
        (false, false, vec!["abort"]),
    ] {
        let (events, receiver) = mpsc::channel();
        let writer = FileWriter::from_blocking(Output { events, fail_close }, executor());
        if finish {
            assert_eq!(block_on(writer.close()).is_err(), fail_close);
        } else {
            drop(writer);
        }
        for event in expected {
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
                event
            );
        }
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
