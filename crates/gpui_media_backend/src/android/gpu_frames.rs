use gpui_util::android::hardware_buffer::{HardwareBuffer, HardwareBufferFrame};
use jni::{
    JNIEnv,
    objects::JClass,
    sys::{jint, jlong},
};
use std::{
    ffi::{CStr, c_char, c_void},
    os::fd::FromRawFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

type Pointer = *mut c_void;
type CreateImage = unsafe extern "C" fn(Pointer, Pointer, u32, Pointer, *const i32) -> Pointer;
type DestroyImage = unsafe extern "C" fn(Pointer, Pointer) -> u32;
type NativeClientBuffer = unsafe extern "C" fn(Pointer) -> Pointer;
type BindImage = unsafe extern "C" fn(u32, Pointer);
type CreateSync = unsafe extern "C" fn(Pointer, u32, *const i32) -> Pointer;
type DestroySync = unsafe extern "C" fn(Pointer, Pointer) -> u32;
type FenceFd = unsafe extern "C" fn(Pointer, Pointer) -> i32;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetProcAddress(name: *const c_char) -> Pointer;
    fn eglGetCurrentDisplay() -> Pointer;
    fn eglQueryString(display: Pointer, name: i32) -> *const c_char;
}
#[link(name = "GLESv2")]
unsafe extern "C" {
    fn glGenTextures(count: i32, textures: *mut u32);
    fn glDeleteTextures(count: i32, textures: *const u32);
    fn glBindTexture(target: u32, texture: u32);
    fn glTexParameteri(target: u32, parameter: u32, value: i32);
    fn glGetError() -> u32;
    fn glFlush();
    fn glFinish();
}

struct Slot {
    buffer: Arc<HardwareBuffer>,
    texture: u32,
    image: Pointer,
}

struct Pool {
    display: Pointer,
    create_image: CreateImage,
    destroy_image: DestroyImage,
    client_buffer: NativeClientBuffer,
    bind_image: BindImage,
    create_sync: CreateSync,
    destroy_sync: DestroySync,
    fence_fd: FenceFd,
    slots: Vec<Slot>,
    selected: Option<usize>,
    failed: Arc<AtomicBool>,
}

impl Pool {
    unsafe fn load<T: Copy>(name: &CStr) -> anyhow::Result<T> {
        let address = unsafe { eglGetProcAddress(name.as_ptr()) };
        anyhow::ensure!(!address.is_null(), "missing EGL entry point {name:?}");
        // All callers supply an EGL/GL function pointer with pointer-sized ABI.
        Ok(unsafe { std::mem::transmute_copy(&address) })
    }

    unsafe fn new() -> anyhow::Result<Self> {
        let display = unsafe { eglGetCurrentDisplay() };
        anyhow::ensure!(!display.is_null(), "no current EGL display");
        let extensions = unsafe { eglQueryString(display, 0x3055) };
        anyhow::ensure!(!extensions.is_null(), "missing EGL extensions");
        let extensions = unsafe { CStr::from_ptr(extensions) }.to_string_lossy();
        for required in [
            "EGL_ANDROID_image_native_buffer",
            "EGL_ANDROID_native_fence_sync",
            "EGL_KHR_image_base",
        ] {
            anyhow::ensure!(
                extensions.split_whitespace().any(|value| value == required),
                "missing {required}"
            );
        }
        Ok(Self {
            display,
            create_image: unsafe { Self::load(c"eglCreateImageKHR") }?,
            destroy_image: unsafe { Self::load(c"eglDestroyImageKHR") }?,
            client_buffer: unsafe { Self::load(c"eglGetNativeClientBufferANDROID") }?,
            bind_image: unsafe { Self::load(c"glEGLImageTargetTexture2DOES") }?,
            create_sync: unsafe { Self::load(c"eglCreateSyncKHR") }?,
            destroy_sync: unsafe { Self::load(c"eglDestroySyncKHR") }?,
            fence_fd: unsafe { Self::load(c"eglDupNativeFenceFDANDROID") }?,
            slots: Vec::new(),
            selected: None,
            failed: Arc::new(AtomicBool::new(false)),
        })
    }

    fn target(&mut self, width: u32, height: u32) -> anyhow::Result<i32> {
        anyhow::ensure!(
            !self.failed.load(Ordering::Acquire),
            "GPU consumer rejected hardware buffers"
        );
        self.selected = None;
        let available = self
            .slots
            .iter()
            .position(|slot| Arc::strong_count(&slot.buffer) == 1);
        let index = if let Some(index) = available {
            if self.slots[index].buffer.width() != width
                || self.slots[index].buffer.height() != height
            {
                let slot = self.slots.swap_remove(index);
                unsafe {
                    glDeleteTextures(1, &slot.texture);
                    (self.destroy_image)(self.display, slot.image);
                }
                self.slots.push(self.allocate(width, height)?);
                self.slots.len() - 1
            } else {
                index
            }
        } else if self.slots.len() < 4 {
            self.slots.push(self.allocate(width, height)?);
            self.slots.len() - 1
        } else {
            return Ok(0);
        };
        self.selected = Some(index);
        Ok(self.slots[index].texture as i32)
    }

    fn allocate(&self, width: u32, height: u32) -> anyhow::Result<Slot> {
        let buffer = Arc::new(HardwareBuffer::allocate(width, height)?);
        let client = unsafe { (self.client_buffer)(buffer.as_ptr()) };
        anyhow::ensure!(!client.is_null(), "cannot create EGL client buffer");
        let image = unsafe {
            (self.create_image)(
                self.display,
                std::ptr::null_mut(),
                0x3140,
                client,
                [0x30D2, 1, 0x3038].as_ptr(),
            )
        };
        anyhow::ensure!(!image.is_null(), "cannot import hardware buffer into EGL");
        let mut texture = 0;
        unsafe {
            glGenTextures(1, &mut texture);
            glBindTexture(0x0DE1, texture);
            glTexParameteri(0x0DE1, 0x2801, 0x2601);
            glTexParameteri(0x0DE1, 0x2800, 0x2601);
            glTexParameteri(0x0DE1, 0x2802, 0x812F);
            glTexParameteri(0x0DE1, 0x2803, 0x812F);
            (self.bind_image)(0x0DE1, image);
            if glGetError() != 0 {
                glDeleteTextures(1, &texture);
                (self.destroy_image)(self.display, image);
                anyhow::bail!("cannot bind hardware buffer to GL texture");
            }
        }
        Ok(Slot {
            buffer,
            texture,
            image,
        })
    }

    fn publish(&mut self) -> anyhow::Result<Arc<HardwareBufferFrame>> {
        let index = self
            .selected
            .take()
            .ok_or_else(|| anyhow::anyhow!("no hardware frame target"))?;
        let sync = unsafe { (self.create_sync)(self.display, 0x3144, [0x3038].as_ptr()) };
        anyhow::ensure!(!sync.is_null(), "cannot create native GPU fence");
        unsafe { glFlush() };
        let fd = unsafe { (self.fence_fd)(self.display, sync) };
        unsafe { (self.destroy_sync)(self.display, sync) };
        anyhow::ensure!(fd >= 0, "cannot export native GPU fence");
        Ok(Arc::new(unsafe {
            HardwareBufferFrame::new(
                self.slots[index].buffer.clone(),
                std::os::fd::OwnedFd::from_raw_fd(fd),
                self.failed.clone(),
            )
        }))
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        // Only called by MediaFrames on the context's owning GL thread.
        unsafe {
            glFinish();
            for slot in &self.slots {
                glDeleteTextures(1, &slot.texture);
                (self.destroy_image)(self.display, slot.image);
            }
        }
    }
}

pub(super) extern "system" fn create(_: JNIEnv, _: JClass) -> jlong {
    match unsafe { Pool::new() } {
        Ok(pool) => Box::into_raw(Box::new(pool)) as jlong,
        Err(error) => {
            log::info!("Android video uses CPU frames: {error:#}");
            0
        }
    }
}

pub(super) extern "system" fn target(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    width: jint,
    height: jint,
) -> jint {
    let pool = unsafe { &mut *(handle as *mut Pool) };
    match pool.target(width as u32, height as u32) {
        Ok(texture) => texture,
        Err(error) => {
            log::warn!("Android video falls back to CPU frames: {error:#}");
            -1
        }
    }
}

pub(super) extern "system" fn publish(
    _: JNIEnv,
    _: JClass,
    handle: jlong,
    id: jlong,
    generation: jlong,
    timestamp: jlong,
) -> jint {
    let pool = unsafe { &mut *(handle as *mut Pool) };
    match pool.publish() {
        Ok(frame) => {
            super::hardware_frame(id, generation, frame, timestamp);
            1
        }
        Err(error) => {
            log::warn!("Android video fence failed: {error:#}");
            0
        }
    }
}

pub(super) extern "system" fn close(_: JNIEnv, _: JClass, handle: jlong) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as *mut Pool) });
    }
}
