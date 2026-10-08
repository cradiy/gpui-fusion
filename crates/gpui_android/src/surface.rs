use anyhow::{Result, anyhow};
use jni::{JNIEnv, objects::JObject};
use raw_window_handle::*;
use std::{ffi::c_void, ptr::NonNull, sync::Arc};

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
}

/// Owns the NDK reference acquired from a Java Surface.
#[derive(Debug, Clone)]
pub(crate) struct NativeWindow(Arc<NativeWindowRef>);

#[derive(Debug)]
struct NativeWindowRef(NonNull<c_void>);

// ANativeWindow uses thread-safe reference counting. Drawing and detachment
// remain serialized on the GPUI thread; these impls only permit WGPU ownership.
unsafe impl Send for NativeWindowRef {}
unsafe impl Sync for NativeWindowRef {}

impl Drop for NativeWindowRef {
    fn drop(&mut self) {
        // SAFETY: fromSurface acquired exactly one NDK reference owned by this Arc.
        unsafe { ANativeWindow_release(self.0.as_ptr()) };
    }
}

impl NativeWindow {
    pub fn is_same_window(&self, other: &Self) -> bool {
        self.0.0 == other.0.0
    }

    pub fn from_surface(env: &JNIEnv, surface: &JObject) -> Result<Self> {
        // SAFETY: called with a live Java Surface on the JNI callback thread.
        let pointer = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
        Ok(Self(Arc::new(NativeWindowRef(
            NonNull::new(pointer).ok_or_else(|| anyhow!("Surface has no native window"))?,
        ))))
    }
}

impl HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = AndroidNdkWindowHandle::new(self.0.0).into();
        // SAFETY: the native window is retained for the borrow's lifetime.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for NativeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}
