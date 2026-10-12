use super::{FrameSize, PixelFormat};
use crate::{MediaError, MediaResult};
use objc2_core_foundation::CFRetained;
use objc2_core_video::*;
type CVPixelBuffer = CFRetained<objc2_core_video::CVPixelBuffer>;
use std::fmt;

/// Retained, immutable CoreVideo pixel buffer.
#[derive(Clone)]
pub struct CoreVideoHandle {
    pixel_buffer: CVPixelBuffer,
}

impl CoreVideoHandle {
    /// # Safety
    /// All producer writes must have finished. The buffer must not be mutated
    /// or reused while this handle or any retained pixel buffer exists.
    pub unsafe fn new(pixel_buffer: CVPixelBuffer) -> Self {
        Self { pixel_buffer }
    }

    /// # Safety
    /// The returned buffer must only be inspected, retained, or sampled.
    pub unsafe fn pixel_buffer(&self) -> &CVPixelBuffer {
        &self.pixel_buffer
    }

    pub(super) fn validate(&self, size: FrameSize, format: PixelFormat) -> MediaResult<()> {
        let code = CVPixelBufferGetPixelFormatType(&self.pixel_buffer);
        let matches_format = match format {
            PixelFormat::Bgra8 => code == u32::from_be_bytes(*b"BGRA"),
            PixelFormat::Rgba8 => code == u32::from_be_bytes(*b"RGBA"),
            PixelFormat::Nv12 => {
                (code == u32::from_be_bytes(*b"420v") || code == u32::from_be_bytes(*b"420f"))
                    && CVPixelBufferGetPlaneCount(&self.pixel_buffer) == 2
            }
        };
        if CVPixelBufferGetWidth(&self.pixel_buffer) != size.width as usize
            || CVPixelBufferGetHeight(&self.pixel_buffer) != size.height as usize
            || !matches_format
        {
            return Err(MediaError::invalid_input(
                "CoreVideo dimensions or pixel format mismatch",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for CoreVideoHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreVideoHandle")
            .field("width", &CVPixelBufferGetWidth(&self.pixel_buffer))
            .field("height", &CVPixelBufferGetHeight(&self.pixel_buffer))
            .finish_non_exhaustive()
    }
}

// SAFETY: CoreVideo supports reference counting and immutable sampling across
// threads; the constructor requires exclusive producer writes to have ended.
unsafe impl Send for CoreVideoHandle {}
// SAFETY: Shared access exposes only an immutable buffer under the same contract.
unsafe impl Sync for CoreVideoHandle {}
