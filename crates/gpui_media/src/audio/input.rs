use std::{
    io::{self, IoSliceMut, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use symphonia::core::io::MediaSource;

/// Physical input position, independent of the demuxer's read-ahead buffer.
pub(super) struct InputState {
    length: Option<u64>,
    position: AtomicU64,
}
impl InputState {
    pub fn at_known_end(&self) -> bool {
        self.length
            .is_some_and(|length| self.position.load(Ordering::Acquire) >= length)
    }
}

pub(super) struct AudioInput {
    source: Box<dyn MediaSource>,
    state: Arc<InputState>,
}
impl AudioInput {
    pub fn new(source: Box<dyn MediaSource>) -> (Self, Arc<InputState>) {
        let state = Arc::new(InputState {
            length: source.byte_len(),
            position: AtomicU64::new(0),
        });
        (
            Self {
                source,
                state: state.clone(),
            },
            state,
        )
    }
}
impl Read for AudioInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = loop {
            match self.source.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        self.state
            .position
            .fetch_add(count as u64, Ordering::AcqRel);
        Ok(count)
    }
    fn read_vectored(&mut self, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let count = loop {
            match self.source.read_vectored(buffers) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        self.state
            .position
            .fetch_add(count as u64, Ordering::AcqRel);
        Ok(count)
    }
}
impl Seek for AudioInput {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let position = self.source.seek(position)?;
        self.state.position.store(position, Ordering::Release);
        Ok(position)
    }
}
impl MediaSource for AudioInput {
    fn is_seekable(&self) -> bool {
        self.source.is_seekable()
    }
    fn byte_len(&self) -> Option<u64> {
        self.state.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};

    struct InterruptedInput {
        bytes: Cursor<Vec<u8>>,
        interrupt: bool,
    }
    impl Read for InterruptedInput {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.interrupt = !self.interrupt;
            if self.interrupt {
                return Err(io::ErrorKind::Interrupted.into());
            }
            // Small reads force repeated refills through the parser's buffering layer.
            let length = buffer.len().min(7);
            self.bytes.read(&mut buffer[..length])
        }
    }
    impl Seek for InterruptedInput {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.bytes.seek(position)
        }
    }
    impl MediaSource for InterruptedInput {
        fn is_seekable(&self) -> bool {
            true
        }
        fn byte_len(&self) -> Option<u64> {
            Some(self.bytes.get_ref().len() as u64)
        }
    }
    #[test]
    fn interrupted_short_reads_preserve_demuxer_bytes_and_end_position() {
        let expected: Vec<u8> = (0..1024).map(|n| (n % 251) as u8).collect();
        let (input, state) = AudioInput::new(Box::new(InterruptedInput {
            bytes: Cursor::new(expected.clone()),
            interrupt: false,
        }));
        let mut stream =
            MediaSourceStream::new(Box::new(input), MediaSourceStreamOptions::default());
        let mut actual = Vec::new();
        stream.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
        assert!(state.at_known_end());
        stream.seek(SeekFrom::Start(0)).unwrap();
        assert!(!state.at_known_end());
        let mut replay = Vec::new();
        stream.read_to_end(&mut replay).unwrap();
        assert_eq!(replay, expected);
    }
}
