use super::*;
use objc2_core_foundation::CFRetained;
use objc2_core_video::*;
type CVPixelBuffer = CFRetained<objc2_core_video::CVPixelBuffer>;
use crate::core_video::{
    CoreVideoTexture as CVMetalTexture, CoreVideoTextureCache as CVMetalTextureCache,
};

use gpui::{CoreVideoHandle, SurfaceColorInfo, SurfaceHandle, SurfaceSource};

#[derive(Default)]
pub(super) struct CoreVideoSurfaces {
    cache: Option<CVMetalTextureCache>,
    pub surfaces: HashMap<SurfaceId, CachedSurface>,
    legacy: HashMap<usize, Arc<SurfaceFrame>>,
}

// Immutable CoreVideo objects are reference counted. Keep both the buffer and its
// texture wrapper alive until WGPU releases the HAL texture after GPU completion.
struct TextureLease {
    _texture: CVMetalTexture,
    _buffer: CoreVideoHandle,
}
unsafe impl Send for TextureLease {}
unsafe impl Sync for TextureLease {}

impl CoreVideoSurfaces {
    pub fn prepare_legacy(&mut self, scene: &Scene) {
        self.legacy.clear();
        scene.visit(&mut |scene| {
            for surface in &scene.surfaces {
                if let SurfaceSource::Surface(buffer) = &surface.source {
                    let key = CFRetained::as_ptr(buffer).as_ptr() as usize;
                    if let std::collections::hash_map::Entry::Vacant(entry) = self.legacy.entry(key)
                    {
                        let (Ok(width), Ok(height)) = (
                            i32::try_from(CVPixelBufferGetWidth(&buffer)),
                            i32::try_from(CVPixelBufferGetHeight(&buffer)),
                        ) else {
                            log::error!("CoreVideo buffer dimensions exceed the supported range");
                            continue;
                        };
                        let size = gpui::size(DevicePixels(width), DevicePixels(height));
                        // Legacy surfaces publish decoder-owned buffers for read-only sampling.
                        let buffer = unsafe { CoreVideoHandle::new(buffer.clone()) };
                        match SurfaceFrame::from_core_video(
                            SurfaceHandle::new(),
                            0,
                            Bounds::new(gpui::point(DevicePixels(0), DevicePixels(0)), size),
                            size,
                            SurfaceFormat::Nv12,
                            buffer,
                            SurfaceColorInfo::default(),
                        ) {
                            Ok(frame) => {
                                entry.insert(Arc::new(frame));
                            }
                            Err(error) => log::error!("invalid legacy CoreVideo frame: {error}"),
                        }
                    }
                }
            }
        });
        self.surfaces.retain(|_, cached| cached.owner.is_alive());
    }

    pub fn frame<'a>(&'a self, source: &'a SurfaceSource) -> Option<&'a Arc<SurfaceFrame>> {
        source.frame().or_else(|| match source {
            SurfaceSource::Surface(buffer) => self
                .legacy
                .get(&(CFRetained::as_ptr(buffer).as_ptr() as usize)),
            _ => None,
        })
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        frame: &SurfaceFrame,
        buffer: &CoreVideoHandle,
    ) -> anyhow::Result<()> {
        let id = frame.handle().id();
        if self.surfaces.get(&id).is_some_and(|cached| {
            cached.sequence == frame.sequence()
                && cached.format == frame.format()
                && cached.size == frame.coded_size()
        }) {
            return Ok(());
        }
        self.surfaces.remove(&id);
        if self.cache.is_none() {
            let hal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }
                .ok_or_else(|| anyhow::anyhow!("CoreVideo requires a Metal device"))?;
            self.cache = Some(
                CVMetalTextureCache::new(hal.raw_device())
                    .map_err(|code| anyhow::anyhow!("CVMetalTextureCacheCreate: {code}"))?,
            );
        }
        let cache = self.cache.as_ref().unwrap();
        let image = unsafe { buffer.pixel_buffer() };
        let textures = match frame.format() {
            SurfaceFormat::Bgra8 | SurfaceFormat::Rgba8 => {
                let (metal_format, format) = if frame.format() == SurfaceFormat::Bgra8 {
                    (
                        objc2_metal::MTLPixelFormat::BGRA8Unorm,
                        wgpu::TextureFormat::Bgra8Unorm,
                    )
                } else {
                    (
                        objc2_metal::MTLPixelFormat::RGBA8Unorm,
                        wgpu::TextureFormat::Rgba8Unorm,
                    )
                };
                let texture = import_plane(
                    device,
                    cache,
                    image,
                    buffer,
                    metal_format,
                    format,
                    CVPixelBufferGetWidth(&image),
                    CVPixelBufferGetHeight(&image),
                    0,
                )?;
                CachedSurfaceTextures::Rgba {
                    view: texture.create_view(&Default::default()),
                    _texture: texture,
                }
            }
            SurfaceFormat::Nv12 => {
                anyhow::ensure!(
                    CVPixelBufferGetPlaneCount(&image) >= 2,
                    "NV12 buffer requires two planes"
                );
                let y = import_plane(
                    device,
                    cache,
                    image,
                    buffer,
                    objc2_metal::MTLPixelFormat::R8Unorm,
                    wgpu::TextureFormat::R8Unorm,
                    CVPixelBufferGetWidthOfPlane(&image, 0),
                    CVPixelBufferGetHeightOfPlane(&image, 0),
                    0,
                )?;
                let uv = import_plane(
                    device,
                    cache,
                    image,
                    buffer,
                    objc2_metal::MTLPixelFormat::RG8Unorm,
                    wgpu::TextureFormat::Rg8Unorm,
                    CVPixelBufferGetWidthOfPlane(&image, 1),
                    CVPixelBufferGetHeightOfPlane(&image, 1),
                    1,
                )?;
                CachedSurfaceTextures::Nv12 {
                    y_view: y.create_view(&Default::default()),
                    uv_view: uv.create_view(&Default::default()),
                    _y_texture: y,
                    _uv_texture: uv,
                }
            }
        };
        self.surfaces.insert(
            id,
            CachedSurface {
                sequence: frame.sequence(),
                format: frame.format(),
                size: frame.coded_size(),
                textures,
                owner: frame.handle().downgrade(),
            },
        );
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn import_plane(
    device: &wgpu::Device,
    cache: &CVMetalTextureCache,
    image: &CVPixelBuffer,
    buffer: &CoreVideoHandle,
    metal_format: objc2_metal::MTLPixelFormat,
    format: wgpu::TextureFormat,
    width: usize,
    height: usize,
    plane: usize,
) -> anyhow::Result<wgpu::Texture> {
    let texture = cache
        .import(image, metal_format, width, height, plane)
        .map_err(|code| anyhow::anyhow!("CoreVideo texture import: {code}"))?;
    let raw = texture.texture.clone();
    let lease = TextureLease {
        _texture: texture,
        _buffer: buffer.clone(),
    };
    let extent = wgpu::Extent3d {
        width: width as u32,
        height: height as u32,
        depth_or_array_layers: 1,
    };
    // The cache uses this exact device; CoreVideo initializes the immutable texture.
    let hal = unsafe {
        wgpu::hal::metal::Device::texture_from_raw(
            raw,
            format,
            objc2_metal::MTLTextureType::Type2D,
            1,
            1,
            extent.into(),
            Some(Box::new(move || drop(lease))),
        )
    };
    Ok(unsafe {
        device.create_texture_from_hal::<wgpu::hal::api::Metal>(
            hal,
            &wgpu::TextureDescriptor {
                label: Some("gpui.core_video"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::wgt::TextureUses::RESOURCE,
        )
    })
}
