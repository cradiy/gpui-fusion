use futures::executor::block_on;
use gpui_io::{
    BlockingWrite, CreateOptions, FileHandle, FileMetadata, FileReader, FileWriter, IoExecutor,
    LocationHandle, PlatformFile, PlatformReader, WriteOptions,
};
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
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
        assert_eq!(file.read_limited(6).await.unwrap(), b"abcdef");
        assert!(file.read_limited(5).await.is_err());
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
        assert_eq!(first.seek(SeekFrom::End(-2)).await.unwrap(), 4);
        assert_eq!(first.read_chunk().await.unwrap().unwrap(), b"ef");
        assert_eq!(first.seek(SeekFrom::Current(-6)).await.unwrap(), 0);
        assert!(first.seek(SeekFrom::Current(-1)).await.is_err());
        assert_eq!(
            first.read_chunk_with_limit(2).await.unwrap().unwrap(),
            b"ab"
        );
        assert_eq!(first.seek(SeekFrom::Start(100)).await.unwrap(), 100);
        assert!(first.read_chunk().await.unwrap().is_none());
        assert_eq!(first.seek(SeekFrom::Start(1)).await.unwrap(), 1);
        assert_eq!(first.read_chunk().await.unwrap().unwrap(), b"bcdef");
        assert_eq!(second.read_chunk().await.unwrap().unwrap(), b"cdef");
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
        assert!(file.read_limited(0).await.unwrap().is_empty());
        let readonly = FileHandle::from_path(file.path().unwrap(), executor, false);
        assert!(readonly.open_write(WriteOptions::truncate()).await.is_err());
    });
}

#[derive(Debug)]
struct UnboundedFile(Arc<AtomicUsize>);
impl PlatformFile for UnboundedFile {
    fn name(&self) -> &str {
        "stream"
    }
    fn metadata(&self) -> futures::future::LocalBoxFuture<'static, anyhow::Result<FileMetadata>> {
        panic!("bounded reads must not rely on provider metadata");
    }
    fn open_read(&self) -> futures::future::LocalBoxFuture<'static, anyhow::Result<FileReader>> {
        let reader = CountingRead(self.0.clone());
        Box::pin(async move { Ok(FileReader::from_blocking(reader, executor())) })
    }
}
struct CountingRead(Arc<AtomicUsize>);
impl Read for CountingRead {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        bytes.fill(0);
        self.0.fetch_add(bytes.len(), Ordering::Relaxed);
        Ok(bytes.len())
    }
}

#[test]
fn bounded_reads_stop_an_unknown_length_stream_and_preserve_sequential_access() {
    block_on(async {
        let bytes = Arc::new(AtomicUsize::new(0));
        let file = FileHandle::new(Arc::new(UnboundedFile(bytes.clone())));
        for limit in [0, 128 * 1024] {
            bytes.store(0, Ordering::Relaxed);
            let error = file.read_limited(limit).await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::FileTooLarge
            );
            assert_eq!(bytes.load(Ordering::Relaxed), limit + 1);
        }
        let mut reader = file.open_read().await.unwrap();
        let error = reader.seek(SeekFrom::Start(0)).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            reader.read_chunk_with_limit(3).await.unwrap().unwrap(),
            [0; 3]
        );
    });
}

struct ShortRead(io::Cursor<Vec<u8>>);
impl Read for ShortRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = buffer.len().min(3);
        self.0.read(&mut buffer[..limit])
    }
}
impl Seek for ShortRead {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

#[test]
fn reader_helpers_follow_the_cursor_across_short_reads_and_errors() {
    block_on(async {
        let mut reader = FileReader::from_seekable(
            ShortRead(io::Cursor::new(b"0123456789abcdef".to_vec())),
            executor(),
        );
        reader.seek(SeekFrom::Start(2)).await.unwrap();
        reader.read_exact(&mut []).await.unwrap();
        let mut header = [0; 5];
        reader.read_exact(&mut header).await.unwrap();
        assert_eq!(&header, b"23456");
        assert_eq!(reader.read_to_end_limited(9).await.unwrap(), b"789abcdef");
        assert_eq!(reader.seek(SeekFrom::Current(0)).await.unwrap(), 16);

        reader.seek(SeekFrom::Start(2)).await.unwrap();
        let error = reader.read_to_end_limited(4).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::FileTooLarge
        );
        let mut next = [0; 3];
        reader.read_exact(&mut next).await.unwrap();
        assert_eq!(&next, b"789");

        reader.seek(SeekFrom::End(-2)).await.unwrap();
        let mut incomplete = [0; 4];
        let error = reader.read_exact(&mut incomplete).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert_eq!(&incomplete[..2], b"ef");
        assert!(reader.read_to_end_limited(0).await.unwrap().is_empty());

        reader.seek(SeekFrom::Start(0)).await.unwrap();
        assert!(reader.read_to_end_limited(0).await.is_err());
        let mut byte = [0];
        reader.read_exact(&mut byte).await.unwrap();
        assert_eq!(&byte, b"1");
    });
}

struct InvalidChunks(Vec<u8>);
impl PlatformReader for InvalidChunks {
    fn read_chunk(
        &mut self,
        _: usize,
    ) -> futures::future::LocalBoxFuture<'_, anyhow::Result<Option<Vec<u8>>>> {
        Box::pin(async { Ok(Some(self.0.clone())) })
    }
}

#[test]
fn reader_helpers_reject_empty_and_oversized_backend_chunks() {
    block_on(async {
        for bytes in [vec![], vec![1, 2, 3]] {
            let mut reader = FileReader::new(InvalidChunks(bytes));
            let error = reader.read_exact(&mut [0]).await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::InvalidData
            );
            let error = reader.read_to_end_limited(1).await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::InvalidData
            );
        }
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
