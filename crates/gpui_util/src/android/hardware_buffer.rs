//! Shared RGBA allocations and immutable frames for Android GPU consumers.

use std::{
    ffi::c_void,
    os::fd::OwnedFd,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[repr(C)]
#[derive(Default)]
struct Description {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    reserved: u32,
    reserved_long: u64,
}

#[link(name = "android")]
unsafe extern "C" {
    fn AHardwareBuffer_allocate(description: *const Description, output: *mut *mut c_void) -> i32;
    fn AHardwareBuffer_release(buffer: *mut c_void);
}

/// An RGBA8 allocation usable as both a GPU render target and sampled image.
#[derive(Debug)]
pub struct HardwareBuffer {
    raw: NonNull<c_void>,
    width: u32,
    height: u32,
}

impl HardwareBuffer {
    pub fn allocate(width: u32, height: u32) -> anyhow::Result<Self> {
        anyhow::ensure!(width > 0 && height > 0, "empty Android hardware buffer");
        let description = Description {
            width,
            height,
            layers: 1,
            format: 1,
            usage: (1 << 8) | (1 << 9),
            ..Default::default()
        };
        let mut raw = std::ptr::null_mut();
        let result = unsafe { AHardwareBuffer_allocate(&description, &mut raw) };
        anyhow::ensure!(result == 0, "AHardwareBuffer_allocate failed: {result}");
        Ok(Self {
            raw: NonNull::new(raw).ok_or_else(|| anyhow::anyhow!("null hardware buffer"))?,
            width,
            height,
        })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}

// SAFETY: The allocation is reference counted by Arc. GPU access is synchronized
// by the frame's acquire fence and consumer completion before pool reuse.
unsafe impl Send for HardwareBuffer {}
unsafe impl Sync for HardwareBuffer {}

impl Drop for HardwareBuffer {
    fn drop(&mut self) {
        unsafe { AHardwareBuffer_release(self.raw.as_ptr()) };
    }
}

/// An immutable frame lease. Keep it alive until GPU reads have completed.
#[derive(Debug)]
pub struct HardwareBufferFrame {
    buffer: Arc<HardwareBuffer>,
    fence: OwnedFd,
    failed: Arc<AtomicBool>,
}

impl HardwareBufferFrame {
    /// # Safety
    /// The fence must cover all producer writes. The allocation must not be
    /// overwritten until this lease and every consumer reference are released.
    pub unsafe fn new(
        buffer: Arc<HardwareBuffer>,
        fence: OwnedFd,
        failed: Arc<AtomicBool>,
    ) -> Self {
        Self {
            buffer,
            fence,
            failed,
        }
    }

    pub fn buffer(&self) -> &HardwareBuffer {
        &self.buffer
    }
    pub fn acquire_fence(&self) -> &OwnedFd {
        &self.fence
    }
    pub fn report_import_failed(&self) {
        self.failed.store(true, Ordering::Release);
    }

    pub fn import_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
