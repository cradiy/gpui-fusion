use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CFRetained;
use objc2_core_video::{
    CVMetalTexture, CVMetalTextureCache, CVMetalTextureGetTexture, CVPixelBuffer,
};
use objc2_metal::{MTLDevice, MTLPixelFormat, MTLTexture};
use std::ptr::NonNull;

/// Imports immutable CoreVideo frames into the Metal device used by the renderer.
pub struct CoreVideoTextureCache(CFRetained<CVMetalTextureCache>);

impl CoreVideoTextureCache {
    pub fn new(device: &ProtocolObject<dyn MTLDevice>) -> Result<Self, i32> {
        let mut raw = std::ptr::null_mut();
        let status = unsafe {
            CVMetalTextureCache::create(None, None, device, None, NonNull::from(&mut raw))
        };
        if status != 0 {
            return Err(status);
        }
        Ok(Self(unsafe {
            CFRetained::from_raw(
                NonNull::new(raw).expect("CoreVideo returned a null texture cache"),
            )
        }))
    }

    /// The pixel buffer must remain immutable until GPU sampling has finished.
    pub fn import(
        &self,
        image: &CVPixelBuffer,
        format: MTLPixelFormat,
        width: usize,
        height: usize,
        plane: usize,
    ) -> Result<CoreVideoTexture, i32> {
        let mut raw = std::ptr::null_mut();
        let status = unsafe {
            CVMetalTextureCache::create_texture_from_image(
                None,
                &self.0,
                image,
                None,
                format,
                width,
                height,
                plane,
                NonNull::from(&mut raw),
            )
        };
        if status != 0 {
            return Err(status);
        }
        let backing = unsafe {
            CFRetained::from_raw(NonNull::new(raw).expect("CoreVideo returned a null texture"))
        };
        let texture = CVMetalTextureGetTexture(&backing).ok_or(-6660)?;
        Ok(CoreVideoTexture {
            texture,
            _backing: backing,
        })
    }
}

/// Retains both the Metal texture and its CoreVideo wrapper for GPU sampling.
pub struct CoreVideoTexture {
    pub texture: Retained<ProtocolObject<dyn MTLTexture>>,
    _backing: CFRetained<CVMetalTexture>,
}
