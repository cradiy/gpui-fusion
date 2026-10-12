use crate::metal::{self, *};
use crate::metal_atlas::MetalAtlas;
use crate::shader_programs::{ShaderLibrary, ShaderProgram};
use anyhow::Result;
use block2::RcBlock;
use gpui::{
    AtlasTextureId, BackdropBlur, BackdropShader, Bounds, ColorRange, DevicePixels, EffectQuad,
    EffectShader, PaintSurface, Path, PathSprite, PolychromeSprite, PrimitiveBatch, ScaledPixels,
    Scene, Shadow, Size, SurfaceColorInfo, SurfaceFormat, SurfaceFrame, SurfaceFrameBacking,
    SurfaceId, Underline, WeakSurfaceHandle, YuvMatrix, point, size,
};
use gpui::{BackdropInstance, EffectInstance};
use objc2::rc::Retained;
use objc2_foundation::NSString;

type Quad = gpui::Quad<gpui::GpuBackground, gpui::GpuBorderGradient>;
type MonochromeSprite = gpui::MonochromeSprite<gpui::GpuBackground>;
use gpui_render::SurfaceParams;
use image::RgbaImage;

use objc2_core_foundation::CFRetained;
use objc2_core_video::*;
type CVPixelBuffer = CFRetained<objc2_core_video::CVPixelBuffer>;
use gpui_wgpu::core_video::{
    CoreVideoTexture as CVMetalTexture, CoreVideoTextureCache as CVMetalTextureCache,
};

use crate::metal::{
    CAMetalLayer, CommandQueue, MTLGPUFamily, MTLPixelFormat, MTLResourceOptions,
    MTLSamplerAddressMode, MTLSamplerMinMagFilter, NSRange, RenderPassColorAttachmentDescriptorRef,
    SamplerDescriptor,
};
use objc2::msg_send;
use objc2_quartz_core::CAAutoresizingMask;
use parking_lot::Mutex;

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    mem, ptr,
    sync::Arc,
};

#[cfg(not(feature = "runtime_shaders"))]
include!(concat!(env!("OUT_DIR"), "/shader_libraries.rs"));
// Use 4x MSAA, all devices support it.
// https://developer.apple.com/documentation/metal/mtldevice/1433355-supportstexturesamplecount
const PATH_SAMPLE_COUNT: u32 = 4;

#[derive(Clone)]
enum CachedSurfaceTextures {
    Rgba(metal::Texture),
    Nv12 {
        y: metal::Texture,
        uv: metal::Texture,
    },
}

struct CachedSurface {
    sequence: u64,
    format: SurfaceFormat,
    size: Size<DevicePixels>,
    textures: CachedSurfaceTextures,
    owner: WeakSurfaceHandle,
}

enum CoreVideoTextures {
    Rgba(CVMetalTexture),
    Nv12 {
        y: CVMetalTexture,
        uv: CVMetalTexture,
    },
}

pub(crate) type Context = Arc<Mutex<InstanceBufferPool>>;
pub(crate) type Renderer = MetalRenderer;

pub(crate) unsafe fn new_renderer(
    context: self::Context,
    _native_window: *mut c_void,
    _native_view: *mut c_void,
    _bounds: gpui::Size<f32>,
    transparent: bool,
) -> Renderer {
    MetalRenderer::new(context, transparent)
}

pub(crate) struct InstanceBufferPool {
    buffer_size: usize,
    buffers: Vec<InstanceBuffer>,
    scene_context: Option<gpui_wgpu::WgpuContext>,
}

impl Default for InstanceBufferPool {
    fn default() -> Self {
        Self {
            buffer_size: 2 * 1024 * 1024,
            buffers: Vec::new(),
            scene_context: None,
        }
    }
}

pub(crate) struct InstanceBuffer {
    metal_buffer: metal::Buffer,
    size: usize,
    gradients: Option<metal::Buffer>,
    gradient_data: Vec<gpui::GpuGradientStop>,
}

// Metal buffers can move between threads. Each instance is exclusively owned while
// encoding and returns to the synchronized pool only after GPU completion.
unsafe impl Send for InstanceBuffer {}

impl InstanceBufferPool {
    fn scene_context(&mut self) -> Result<gpui_wgpu::WgpuContext> {
        if self.scene_context.is_none() {
            self.scene_context = Some(gpui_wgpu::WgpuContext::new_headless()?);
        }
        Ok(self.scene_context.as_ref().unwrap().clone())
    }

    pub(crate) fn reset(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
        self.buffers.clear();
    }

    pub(crate) fn acquire(
        &mut self,
        device: &metal::Device,
        unified_memory: bool,
    ) -> InstanceBuffer {
        if let Some(buffer) = self.buffers.pop() {
            return buffer;
        }
        let buffer = {
            let options = if unified_memory {
                MTLResourceOptions::StorageModeShared
                    // Buffers are write only which can benefit from the combined cache
                    // https://developer.apple.com/documentation/metal/mtlresourceoptions/cpucachemodewritecombined
                    | MTLResourceOptions::CPUCacheModeWriteCombined
            } else {
                MTLResourceOptions::StorageModeManaged
            };

            device
                .newBufferWithLength_options(self.buffer_size, options)
                .expect("Metal newBufferWithLength_options failed")
        };
        InstanceBuffer {
            metal_buffer: buffer,
            size: self.buffer_size,
            gradients: None,
            gradient_data: Vec::new(),
        }
    }

    pub(crate) fn release(&mut self, buffer: InstanceBuffer) {
        if buffer.size == self.buffer_size {
            self.buffers.push(buffer)
        }
    }
}

pub(crate) struct MetalRenderer {
    scene_renderer: Option<crate::metal_scene::MetalSceneRenderer>,
    subtree_pipeline_state: metal::RenderPipelineState,
    device: metal::Device,
    layer: Option<metal::MetalLayer>,
    is_apple_gpu: bool,
    is_unified_memory: bool,
    presents_with_transaction: bool,
    /// For headless rendering, tracks whether output should be opaque
    opaque: bool,
    command_queue: CommandQueue,
    paths_rasterization_pipeline_state: metal::RenderPipelineState,
    path_sprites_pipeline_state: metal::RenderPipelineState,
    shadows_pipeline_state: metal::RenderPipelineState,
    quads_pipeline_state: metal::RenderPipelineState,
    effect_pipeline_states: HashMap<u64, metal::RenderPipelineState>,
    failed_effect_pipeline_states: HashSet<u64>,
    backdrop_effect_pipeline_states: HashMap<u64, metal::RenderPipelineState>,
    failed_backdrop_effect_pipeline_states: HashSet<u64>,
    effect_sampler: metal::SamplerState,
    underlines_pipeline_state: metal::RenderPipelineState,
    monochrome_sprites_pipeline_state: metal::RenderPipelineState,
    polychrome_sprites_pipeline_state: metal::RenderPipelineState,
    surfaces_rgba_pipeline_state: metal::RenderPipelineState,
    surfaces_nv12_pipeline_state: metal::RenderPipelineState,
    surfaces: HashMap<SurfaceId, CachedSurface>,
    #[allow(clippy::arc_with_non_send_sync)]
    instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
    sprite_atlas: Arc<MetalAtlas>,
    core_video_texture_cache: CVMetalTextureCache,
    path_intermediate_texture: Option<metal::Texture>,
    path_intermediate_msaa_texture: Option<metal::Texture>,
    path_sample_count: u32,
    unused_path_frames: u16,
    backdrop_source_texture: Option<metal::Texture>,
    backdrop_blurred_texture: Option<metal::Texture>,
    /// Offscreen render target reused across `render_scene` calls when
    /// rendering headlessly without reading pixels back.
    #[cfg(any(test, feature = "test-support"))]
    headless_render_target: Option<metal::Texture>,
}

type EffectGlobalParams = gpui_render::PrimitiveGlobals;

const DEFAULT_BACKDROP_EFFECT: &str = r#"
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    return sample_blurred_backdrop(input, vec2<f32>(0.0));
}
"#;

impl MetalRenderer {
    /// Creates a new MetalRenderer with a CAMetalLayer for window-based rendering.
    pub fn new(instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>, transparent: bool) -> Self {
        let device = Self::create_device();

        let layer = metal::CAMetalLayer::new();
        layer.setDevice(Some(&device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        // Support direct-to-display rendering if the window is not transparent
        // https://developer.apple.com/documentation/metal/managing-your-game-window-for-metal-in-macos
        layer.setOpaque(!transparent);
        layer.setMaximumDrawableCount(3);
        // Backdrop effects sample pixels already rendered into the drawable.
        layer.setFramebufferOnly(false);
        unsafe {
            let _: () = msg_send![&*layer, setAllowsNextDrawableTimeout: false];
            let _: () = msg_send![&*layer, setNeedsDisplayOnBoundsChange: true];
            let _: () = msg_send![
                &*layer,
                setAutoresizingMask: (CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable).bits()
            ];
        }

        Self::new_internal(device, Some(layer), !transparent, instance_buffer_pool)
    }

    /// Creates a new headless MetalRenderer for offscreen rendering without a window.
    ///
    /// This renderer can render scenes to images without requiring a CAMetalLayer,
    /// window, or AppKit. Use `render_scene_to_image()` to render scenes.
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_headless(instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>) -> Self {
        let device = Self::create_device();
        Self::new_internal(device, None, true, instance_buffer_pool)
    }

    pub(crate) fn new_auxiliary(&self) -> Self {
        let mut renderer = Self::new_internal(
            self.device.clone(),
            None,
            false,
            self.instance_buffer_pool.clone(),
        );
        renderer.sprite_atlas = self.sprite_atlas.clone();
        renderer
    }

    fn create_device() -> metal::Device {
        // Prefer low‐power integrated GPUs on Intel Mac. On Apple
        // Silicon, there is only ever one GPU, so this is equivalent to
        // `metal::MTLCreateSystemDefaultDevice()`.
        if let Some(d) = metal::MTLCopyAllDevices()
            .to_vec()
            .into_iter()
            .min_by_key(|d| (d.isRemovable(), !d.isLowPower()))
        {
            d
        } else {
            // For some reason `all()` can return an empty list, see https://github.com/zed-industries/zed/issues/37689
            // In that case, we fall back to the system default device.
            log::error!(
                "Unable to enumerate Metal devices; attempting to use system default device"
            );
            metal::MTLCreateSystemDefaultDevice().unwrap_or_else(|| {
                log::error!("unable to access a compatible graphics device");
                std::process::exit(1);
            })
        }
    }

    fn new_internal(
        device: metal::Device,
        layer: Option<metal::MetalLayer>,
        opaque: bool,
        instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
    ) -> Self {
        let scene_renderer = match instance_buffer_pool
            .lock()
            .scene_context()
            .and_then(crate::metal_scene::MetalSceneRenderer::new)
        {
            Ok(renderer) => Some(renderer),
            Err(error) => {
                log::error!("failed to initialize Metal 3D renderer: {error:#}");
                None
            }
        };
        let device = scene_renderer
            .as_ref()
            .map_or(device, |renderer| renderer.device());
        if let Some(layer) = &layer {
            layer.setDevice(Some(&device));
        }
        // Shared memory can be used only if CPU and GPU share the same memory space.
        // https://developer.apple.com/documentation/metal/setting-resource-storage-modes
        let is_unified_memory = device.hasUnifiedMemory();
        // Apple GPU families support memoryless textures, which can significantly reduce
        // memory usage by keeping render targets in on-chip tile memory instead of
        // allocating backing store in system memory.
        // https://developer.apple.com/documentation/metal/mtlgpufamily
        let is_apple_gpu = device.supportsFamily(MTLGPUFamily::Apple1);

        let libraries: HashMap<_, _> = ShaderLibrary::ALL
            .iter()
            .map(|&shader| {
                #[cfg(feature = "runtime_shaders")]
                let library = metal::library_from_source(
                    &device,
                    shader.source(),
                    &metal::CompileOptions::new(),
                );
                #[cfg(not(feature = "runtime_shaders"))]
                let library = metal::library_from_data(&device, compiled_library_bytes(shader));
                let library = library.unwrap_or_else(|error| {
                    panic!("error loading Metal library {}: {error}", shader.name())
                });
                (shader, library)
            })
            .collect();
        let path_sprites_pipeline_state = build_path_sprite_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::PathSprite,
            MTLPixelFormat::BGRA8Unorm,
        );
        let shadows_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::Shadow,
            MTLPixelFormat::BGRA8Unorm,
        );
        let paths_rasterization_pipeline_state = build_path_rasterization_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::PathRasterization,
            MTLPixelFormat::BGRA8Unorm,
            PATH_SAMPLE_COUNT,
        );
        let quads_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::Quad,
            MTLPixelFormat::BGRA8Unorm,
        );
        let underlines_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::Underline,
            MTLPixelFormat::BGRA8Unorm,
        );
        let monochrome_sprites_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::MonochromeSprite,
            MTLPixelFormat::BGRA8Unorm,
        );
        let polychrome_sprites_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::PolychromeSprite,
            MTLPixelFormat::BGRA8Unorm,
        );
        let surfaces_rgba_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::SurfaceRgba,
            MTLPixelFormat::BGRA8Unorm,
        );
        let surfaces_nv12_pipeline_state = build_pipeline_state(
            &device,
            &libraries,
            ShaderProgram::SurfaceNv12,
            MTLPixelFormat::BGRA8Unorm,
        );

        let subtree_pipeline_state = crate::metal_scene::composite_pipeline(
            &device,
            &libraries[&ShaderProgram::Subtree.program().library],
        );
        let command_queue = scene_renderer.as_ref().map_or_else(
            || {
                device
                    .newCommandQueue()
                    .expect("Metal newCommandQueue failed")
            },
            |renderer| renderer.command_queue(),
        );
        let sprite_atlas = Arc::new(if let Some(renderer) = &scene_renderer {
            MetalAtlas::with_shared(
                device.clone(),
                is_apple_gpu,
                renderer.renderer.sprite_atlas().clone(),
            )
        } else {
            MetalAtlas::new(device.clone(), is_apple_gpu)
        });
        let core_video_texture_cache = CVMetalTextureCache::new(&device).unwrap();
        let effect_sampler_descriptor = SamplerDescriptor::new();
        effect_sampler_descriptor.setMinFilter(MTLSamplerMinMagFilter::Linear);
        effect_sampler_descriptor.setMagFilter(MTLSamplerMinMagFilter::Linear);
        effect_sampler_descriptor.setSAddressMode(MTLSamplerAddressMode::ClampToEdge);
        effect_sampler_descriptor.setTAddressMode(MTLSamplerAddressMode::ClampToEdge);
        let effect_sampler = device
            .newSamplerStateWithDescriptor(&effect_sampler_descriptor)
            .expect("Metal newSamplerStateWithDescriptor failed");

        Self {
            scene_renderer,
            subtree_pipeline_state,
            device,
            layer,
            presents_with_transaction: false,
            is_apple_gpu,
            is_unified_memory,
            opaque,
            command_queue,
            paths_rasterization_pipeline_state,
            path_sprites_pipeline_state,
            shadows_pipeline_state,
            quads_pipeline_state,
            effect_pipeline_states: HashMap::default(),
            failed_effect_pipeline_states: HashSet::default(),
            backdrop_effect_pipeline_states: HashMap::default(),
            failed_backdrop_effect_pipeline_states: HashSet::default(),
            effect_sampler,
            underlines_pipeline_state,
            monochrome_sprites_pipeline_state,
            polychrome_sprites_pipeline_state,
            surfaces_rgba_pipeline_state,
            surfaces_nv12_pipeline_state,
            surfaces: HashMap::default(),
            instance_buffer_pool,
            sprite_atlas,
            core_video_texture_cache,
            path_intermediate_texture: None,
            path_intermediate_msaa_texture: None,
            path_sample_count: PATH_SAMPLE_COUNT,
            unused_path_frames: 0,
            backdrop_source_texture: None,
            backdrop_blurred_texture: None,
            #[cfg(any(test, feature = "test-support"))]
            headless_render_target: None,
        }
    }

    pub fn scene3d_support(&self) -> gpui::Scene3dSupport {
        self.scene_renderer.as_ref().map_or(
            gpui::Scene3dSupport::Unsupported(gpui::Scene3dUnsupportedReason::RendererUnavailable),
            |renderer| renderer.renderer.scene3d_support(),
        )
    }

    pub fn supports_subtree_effects(&self) -> bool {
        self.scene_renderer
            .as_ref()
            .is_some_and(|renderer| !renderer.context.device_lost())
    }

    pub fn gpu_specs(&self) -> gpui::GpuSpecs {
        gpui::GpuSpecs {
            device_name: self.device.name().to_string(),
            driver_name: "Metal".to_owned(),
            ..Default::default()
        }
    }

    pub fn clear_scene3d_caches(&mut self) {
        if let Some(renderer) = &mut self.scene_renderer {
            renderer.clear_caches();
        }
    }

    pub fn scene3d_output_cache_stats(&self) -> Option<gpui::Scene3dOutputCacheStats> {
        self.scene_renderer
            .as_ref()
            .map(|renderer| renderer.renderer.scene3d_output_cache_stats())
    }

    pub fn set_scene3d_output_cache_budget(&mut self, bytes: u64) {
        if let Some(renderer) = &mut self.scene_renderer {
            renderer.renderer.set_scene3d_output_cache_budget(bytes);
        }
    }

    pub fn layer(&self) -> Option<&metal::MetalLayerRef> {
        self.layer.as_ref().map(|l| l.as_ref())
    }

    pub fn layer_ptr(&self) -> *mut CAMetalLayer {
        self.layer
            .as_ref()
            .map(|l| Retained::as_ptr(l).cast_mut())
            .unwrap_or(ptr::null_mut())
    }

    pub fn sprite_atlas(&self) -> &Arc<MetalAtlas> {
        &self.sprite_atlas
    }

    pub fn set_presents_with_transaction(&mut self, presents_with_transaction: bool) {
        self.presents_with_transaction = presents_with_transaction;
        if let Some(layer) = &self.layer {
            layer.setPresentsWithTransaction(presents_with_transaction);
        }
    }

    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        if let Some(layer) = &self.layer {
            layer.setDrawableSize(objc2_core_foundation::CGSize::new(
                size.width.0 as f64,
                size.height.0 as f64,
            ));
        }
        if self
            .path_intermediate_texture
            .as_ref()
            .is_some_and(|texture| {
                texture.width() != size.width.0.max(0) as usize
                    || texture.height() != size.height.0.max(0) as usize
            })
        {
            self.path_intermediate_texture = None;
            self.path_intermediate_msaa_texture = None;
        }
    }

    fn update_path_intermediate_textures(&mut self, size: Size<DevicePixels>) {
        // We are uncertain when this happens, but sometimes size can be 0 here. Most likely before
        // the layout pass on window creation. Zero-sized texture creation causes SIGABRT.
        // https://github.com/zed-industries/zed/issues/36229
        if size.width.0 <= 0 || size.height.0 <= 0 {
            self.path_intermediate_texture = None;
            self.path_intermediate_msaa_texture = None;
            return;
        }

        if self
            .path_intermediate_texture
            .as_ref()
            .is_some_and(|texture| {
                texture.width() == size.width.0 as usize
                    && texture.height() == size.height.0 as usize
            })
        {
            return;
        }

        let texture_descriptor = metal::TextureDescriptor::new();
        unsafe { texture_descriptor.setWidth(size.width.0 as usize) };
        unsafe { texture_descriptor.setHeight(size.height.0 as usize) };
        texture_descriptor.setPixelFormat(metal::MTLPixelFormat::BGRA8Unorm);
        texture_descriptor.setStorageMode(metal::MTLStorageMode::Private);
        texture_descriptor
            .setUsage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        self.path_intermediate_texture = Some(
            self.device
                .newTextureWithDescriptor(&texture_descriptor)
                .expect("Metal newTextureWithDescriptor failed"),
        );

        if self.path_sample_count > 1 {
            // https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus
            // Rendering MSAA textures are done in a single pass, so we can use memory-less storage on Apple Silicon
            let storage_mode = if self.is_apple_gpu {
                metal::MTLStorageMode::Memoryless
            } else {
                metal::MTLStorageMode::Private
            };

            let msaa_descriptor = texture_descriptor;
            msaa_descriptor.setTextureType(metal::MTLTextureType::Type2DMultisample);
            msaa_descriptor.setStorageMode(storage_mode);
            unsafe { msaa_descriptor.setSampleCount(self.path_sample_count as _) };
            self.path_intermediate_msaa_texture = Some(
                self.device
                    .newTextureWithDescriptor(&msaa_descriptor)
                    .expect("Metal newTextureWithDescriptor failed"),
            );
        } else {
            self.path_intermediate_msaa_texture = None;
        }
    }

    pub fn update_transparency(&mut self, transparent: bool) {
        self.opaque = !transparent;
        if let Some(layer) = &self.layer {
            layer.setOpaque(!transparent);
        }
    }

    pub fn destroy(&self) {
        // nothing to do
    }

    pub fn draw(&mut self, scene: &Scene) {
        let layer = match &self.layer {
            Some(l) => l.clone(),
            None => {
                log::error!(
                    "draw() called on headless renderer - use render_scene_to_image() instead"
                );
                return;
            }
        };
        let viewport_size = layer.drawableSize();
        let viewport_size: Size<DevicePixels> = size(
            (viewport_size.width.ceil() as i32).into(),
            (viewport_size.height.ceil() as i32).into(),
        );
        let drawable = if let Some(drawable) = layer.nextDrawable() {
            drawable
        } else {
            log::error!(
                "failed to retrieve next drawable, drawable size: {:?}",
                viewport_size
            );
            return;
        };

        loop {
            let mut instance_buffer = self
                .instance_buffer_pool
                .lock()
                .acquire(&self.device, self.is_unified_memory);

            let command_buffer =
                self.draw_primitives(scene, &mut instance_buffer, &drawable, viewport_size);

            match command_buffer {
                Ok(command_buffer) => {
                    let instance_buffer_pool = self.instance_buffer_pool.clone();
                    let instance_buffer = Cell::new(Some(instance_buffer));
                    let block =
                        RcBlock::new(move |_: std::ptr::NonNull<metal::CommandBufferRef>| {
                            if let Some(instance_buffer) = instance_buffer.take() {
                                instance_buffer_pool.lock().release(instance_buffer);
                            }
                        });

                    unsafe { command_buffer.addCompletedHandler(RcBlock::as_ptr(&block)) };

                    if self.presents_with_transaction {
                        command_buffer.commit();
                        command_buffer.waitUntilScheduled();
                        drawable.present();
                    } else {
                        command_buffer.presentDrawable((&*drawable).as_ref());
                        command_buffer.commit();
                    }
                    return;
                }
                Err(err) => {
                    log::error!(
                        "failed to render: {}. retrying with larger instance buffer size",
                        err
                    );
                    let mut instance_buffer_pool = self.instance_buffer_pool.lock();
                    let buffer_size = instance_buffer_pool.buffer_size;
                    if buffer_size >= 256 * 1024 * 1024 {
                        log::error!("instance buffer size grew too large: {}", buffer_size);
                        break;
                    }
                    instance_buffer_pool.reset(buffer_size * 2);
                    log::info!(
                        "increased instance buffer size to {}",
                        instance_buffer_pool.buffer_size
                    );
                }
            }
        }
    }

    /// Renders the scene to a texture and returns the pixel data as an RGBA image.
    /// This does not present the frame to screen - useful for visual testing
    /// where we want to capture what would be rendered without displaying it.
    ///
    /// Note: This requires a layer-backed renderer. For headless rendering,
    /// use `render_scene_to_image()` instead.
    #[cfg(any(test, feature = "test-support"))]
    pub fn render_to_image(&mut self, scene: &Scene) -> Result<RgbaImage> {
        let layer = self
            .layer
            .clone()
            .ok_or_else(|| anyhow::anyhow!("render_to_image requires a layer-backed renderer"))?;
        let viewport_size = layer.drawableSize();
        let viewport_size: Size<DevicePixels> = size(
            (viewport_size.width.ceil() as i32).into(),
            (viewport_size.height.ceil() as i32).into(),
        );
        let drawable = layer
            .nextDrawable()
            .ok_or_else(|| anyhow::anyhow!("Failed to get drawable for render_to_image"))?;

        loop {
            let mut instance_buffer = self
                .instance_buffer_pool
                .lock()
                .acquire(&self.device, self.is_unified_memory);

            let command_buffer =
                self.draw_primitives(scene, &mut instance_buffer, &drawable, viewport_size);

            match command_buffer {
                Ok(command_buffer) => {
                    let instance_buffer_pool = self.instance_buffer_pool.clone();
                    let instance_buffer = Cell::new(Some(instance_buffer));
                    let block =
                        RcBlock::new(move |_: std::ptr::NonNull<metal::CommandBufferRef>| {
                            if let Some(instance_buffer) = instance_buffer.take() {
                                instance_buffer_pool.lock().release(instance_buffer);
                            }
                        });

                    unsafe {
                        command_buffer.addCompletedHandler(RcBlock::as_ptr(&block));
                    }

                    // Commit and wait for completion without presenting
                    command_buffer.commit();
                    command_buffer.waitUntilCompleted();

                    // Read pixels from the texture
                    let texture = drawable.texture();
                    let width = texture.width() as u32;
                    let height = texture.height() as u32;
                    let bytes_per_row = width as usize * 4;
                    let buffer_size = height as usize * bytes_per_row;

                    let mut pixels = vec![0u8; buffer_size];

                    let region = metal::MTLRegion {
                        origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
                        size: metal::MTLSize {
                            width: width as usize,
                            height: height as usize,
                            depth: 1,
                        },
                    };

                    unsafe {
                        texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                            std::ptr::NonNull::new(pixels.as_mut_ptr().cast()).unwrap(),
                            bytes_per_row,
                            region,
                            0,
                        );
                    }

                    // Convert BGRA to RGBA (swap B and R channels)
                    for chunk in pixels.chunks_exact_mut(4) {
                        chunk.swap(0, 2);
                    }

                    return RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
                        anyhow::anyhow!("Failed to create RgbaImage from pixel data")
                    });
                }
                Err(err) => {
                    log::error!(
                        "failed to render: {}. retrying with larger instance buffer size",
                        err
                    );
                    let mut instance_buffer_pool = self.instance_buffer_pool.lock();
                    let buffer_size = instance_buffer_pool.buffer_size;
                    if buffer_size >= 256 * 1024 * 1024 {
                        anyhow::bail!("instance buffer size grew too large: {}", buffer_size);
                    }
                    instance_buffer_pool.reset(buffer_size * 2);
                    log::info!(
                        "increased instance buffer size to {}",
                        instance_buffer_pool.buffer_size
                    );
                }
            }
        }
    }

    /// Renders a scene to an image without requiring a window or CAMetalLayer.
    ///
    /// This is the primary method for headless rendering. It creates an offscreen
    /// texture, renders the scene to it, and returns the pixel data as an RGBA image.
    pub fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        if size.width.0 <= 0 || size.height.0 <= 0 {
            anyhow::bail!("Invalid size for render_scene_to_image: {:?}", size);
        }

        // Create an offscreen texture as render target
        let texture_descriptor = metal::TextureDescriptor::new();
        unsafe { texture_descriptor.setWidth(size.width.0 as usize) };
        unsafe { texture_descriptor.setHeight(size.height.0 as usize) };
        texture_descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        texture_descriptor
            .setUsage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        texture_descriptor.setStorageMode(metal::MTLStorageMode::Managed);
        let target_texture = self
            .device
            .newTextureWithDescriptor(&texture_descriptor)
            .expect("Metal newTextureWithDescriptor failed");

        loop {
            let mut instance_buffer = self
                .instance_buffer_pool
                .lock()
                .acquire(&self.device, self.is_unified_memory);

            let command_buffer =
                self.draw_primitives_to_texture(scene, &mut instance_buffer, &target_texture, size);

            match command_buffer {
                Ok(command_buffer) => {
                    let instance_buffer_pool = self.instance_buffer_pool.clone();
                    let instance_buffer = Cell::new(Some(instance_buffer));
                    let block =
                        RcBlock::new(move |_: std::ptr::NonNull<metal::CommandBufferRef>| {
                            if let Some(instance_buffer) = instance_buffer.take() {
                                instance_buffer_pool.lock().release(instance_buffer);
                            }
                        });

                    unsafe { command_buffer.addCompletedHandler(RcBlock::as_ptr(&block)) };

                    // On discrete GPUs (non-unified memory), Managed textures
                    // require an explicit blit synchronize before the CPU can
                    // read back the rendered data. Without this, get_bytes
                    // returns stale zeros.
                    if !self.is_unified_memory {
                        let blit = command_buffer
                            .blitCommandEncoder()
                            .expect("Metal blitCommandEncoder failed");
                        blit.synchronizeResource(target_texture.as_ref());
                        blit.endEncoding();
                    }

                    // Commit and wait for completion
                    command_buffer.commit();
                    command_buffer.waitUntilCompleted();

                    // Read pixels from the texture
                    let width = size.width.0 as u32;
                    let height = size.height.0 as u32;
                    let bytes_per_row = width as usize * 4;
                    let buffer_size = height as usize * bytes_per_row;

                    let mut pixels = vec![0u8; buffer_size];

                    let region = metal::MTLRegion {
                        origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
                        size: metal::MTLSize {
                            width: width as usize,
                            height: height as usize,
                            depth: 1,
                        },
                    };

                    unsafe {
                        target_texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                            std::ptr::NonNull::new(pixels.as_mut_ptr().cast()).unwrap(),
                            bytes_per_row,
                            region,
                            0,
                        )
                    };

                    // Convert BGRA to RGBA (swap B and R channels)
                    for chunk in pixels.chunks_exact_mut(4) {
                        chunk.swap(0, 2);
                    }

                    return RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
                        anyhow::anyhow!("Failed to create RgbaImage from pixel data")
                    });
                }
                Err(err) => {
                    log::error!(
                        "failed to render: {}. retrying with larger instance buffer size",
                        err
                    );
                    let mut instance_buffer_pool = self.instance_buffer_pool.lock();
                    let buffer_size = instance_buffer_pool.buffer_size;
                    if buffer_size >= 256 * 1024 * 1024 {
                        anyhow::bail!("instance buffer size grew too large: {}", buffer_size);
                    }
                    instance_buffer_pool.reset(buffer_size * 2);
                    log::info!(
                        "increased instance buffer size to {}",
                        instance_buffer_pool.buffer_size
                    );
                }
            }
        }
    }

    /// Renders a scene to a reused offscreen texture without reading pixels
    /// back or blocking on GPU completion.
    ///
    /// This mirrors the CPU cost of presenting a frame to a window (scene
    /// encoding, instance buffer writes, command submission) and is used by
    /// headless benchmark rendering, where the produced pixels are never
    /// inspected.
    #[cfg(any(test, feature = "test-support"))]
    pub fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        if size.width.0 <= 0 || size.height.0 <= 0 {
            anyhow::bail!("Invalid size for render_scene: {:?}", size);
        }

        let needs_new_target = self.headless_render_target.as_ref().is_none_or(|texture| {
            texture.width() != size.width.0 as usize || texture.height() != size.height.0 as usize
        });
        if needs_new_target {
            let texture_descriptor = metal::TextureDescriptor::new();
            unsafe { texture_descriptor.setWidth(size.width.0 as usize) };
            unsafe { texture_descriptor.setHeight(size.height.0 as usize) };
            texture_descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            texture_descriptor.setUsage(
                metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead,
            );
            texture_descriptor.setStorageMode(metal::MTLStorageMode::Private);
            self.headless_render_target = Some(
                self.device
                    .newTextureWithDescriptor(&texture_descriptor)
                    .expect("Metal newTextureWithDescriptor failed"),
            );
        }
        let target_texture = self
            .headless_render_target
            .clone()
            .expect("just ensured the render target exists");

        loop {
            let mut instance_buffer = self
                .instance_buffer_pool
                .lock()
                .acquire(&self.device, self.is_unified_memory);

            let command_buffer =
                self.draw_primitives_to_texture(scene, &mut instance_buffer, &target_texture, size);

            match command_buffer {
                Ok(command_buffer) => {
                    let instance_buffer_pool = self.instance_buffer_pool.clone();
                    let instance_buffer = Cell::new(Some(instance_buffer));
                    let block =
                        RcBlock::new(move |_: std::ptr::NonNull<metal::CommandBufferRef>| {
                            if let Some(instance_buffer) = instance_buffer.take() {
                                instance_buffer_pool.lock().release(instance_buffer);
                            }
                        });

                    unsafe { command_buffer.addCompletedHandler(RcBlock::as_ptr(&block)) };

                    // Commit without waiting, mirroring presentation to a real
                    // window where the CPU doesn't block on the GPU.
                    command_buffer.commit();
                    return Ok(());
                }
                Err(err) => {
                    log::error!(
                        "failed to render: {}. retrying with larger instance buffer size",
                        err
                    );
                    let mut instance_buffer_pool = self.instance_buffer_pool.lock();
                    let buffer_size = instance_buffer_pool.buffer_size;
                    if buffer_size >= 256 * 1024 * 1024 {
                        anyhow::bail!("instance buffer size grew too large: {}", buffer_size);
                    }
                    instance_buffer_pool.reset(buffer_size * 2);
                    log::info!(
                        "increased instance buffer size to {}",
                        instance_buffer_pool.buffer_size
                    );
                }
            }
        }
    }

    fn draw_primitives(
        &mut self,
        scene: &Scene,
        instance_buffer: &mut InstanceBuffer,
        drawable: &metal::MetalDrawableRef,
        viewport_size: Size<DevicePixels>,
    ) -> Result<metal::CommandBuffer> {
        self.draw_primitives_to_texture(scene, instance_buffer, &drawable.texture(), viewport_size)
    }

    fn draw_primitives_to_texture(
        &mut self,
        scene: &Scene,
        instance_buffer: &mut InstanceBuffer,
        texture: &metal::TextureRef,
        viewport_size: Size<DevicePixels>,
    ) -> Result<metal::CommandBuffer> {
        let data = scene.gradients.stops();
        let bytes = std::mem::size_of_val(data).max(std::mem::size_of::<gpui::GpuGradientStop>());
        let limit = self.device.maxBufferLength().min(u32::MAX as usize);
        anyhow::ensure!(
            bytes <= limit,
            "gradient stop buffer requires {bytes} bytes, device limit is {limit} bytes"
        );
        if instance_buffer
            .gradients
            .as_ref()
            .is_none_or(|buffer| buffer.length() < bytes as usize)
        {
            let capacity = bytes
                .checked_next_power_of_two()
                .unwrap_or(bytes)
                .min(limit);
            instance_buffer.gradients = Some(
                self.device
                    .newBufferWithLength_options(
                        capacity as usize,
                        MTLResourceOptions::StorageModeShared,
                    )
                    .expect("Metal newBufferWithLength_options failed"),
            );
            instance_buffer.gradient_data.clear();
        }
        if let Some(range) = gpui::gradient_changed_range(&instance_buffer.gradient_data, data) {
            let target = instance_buffer
                .gradients
                .as_ref()
                .unwrap()
                .contents()
                .as_ptr() as *mut gpui::GpuGradientStop;
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr().add(range.start),
                    target.add(range.start),
                    range.len(),
                );
            }
        }
        instance_buffer.gradient_data.clear();
        instance_buffer.gradient_data.extend_from_slice(data);
        if scene.paths.is_empty() {
            self.unused_path_frames = self.unused_path_frames.saturating_add(1);
            if self.unused_path_frames >= 120 {
                self.path_intermediate_texture = None;
                self.path_intermediate_msaa_texture = None;
            }
        } else {
            self.unused_path_frames = 0;
            self.update_path_intermediate_textures(viewport_size);
        }
        let subtree_textures = if let Some(renderer) = &mut self.scene_renderer {
            renderer.prepare(scene, viewport_size)?
        } else {
            anyhow::ensure!(
                scene.subtree_layers.is_empty()
                    && scene.particles.is_empty()
                    && scene.fluids.is_empty(),
                "Metal subtree renderer is unavailable"
            );
            Vec::new()
        };
        self.ensure_effect_pipelines(scene);
        self.ensure_backdrop_effect_pipelines(scene);
        let command_queue = self.command_queue.clone();
        let command_buffer = command_queue
            .commandBuffer()
            .expect("Metal commandBuffer failed");
        let alpha = if self.opaque { 1. } else { 0. };
        let mut instance_offset = 0;

        let mut command_encoder = new_command_encoder_for_texture(
            &command_buffer,
            texture,
            viewport_size,
            |color_attachment| {
                color_attachment.setLoadAction(metal::MTLLoadAction::Clear);
                color_attachment.setClearColor(metal::clear_color(0., 0., 0., alpha));
            },
        );

        for batch in scene.batches() {
            let ok = match batch {
                PrimitiveBatch::SubtreeLayers(range) => {
                    command_encoder.setRenderPipelineState(&self.subtree_pipeline_state);
                    for texture in &subtree_textures[range] {
                        unsafe {
                            command_encoder.setFragmentTexture_atIndex(Some(texture), 0_usize)
                        };
                        unsafe {
                            command_encoder.drawPrimitives_vertexStart_vertexCount(
                                metal::MTLPrimitiveType::Triangle,
                                0,
                                3,
                            )
                        };
                    }
                    true
                }
                PrimitiveBatch::BackdropBlurs(range) => {
                    command_encoder.endEncoding();
                    let did_draw = self.draw_backdrop_blurs(
                        &scene.backdrop_blurs[range],
                        instance_buffer,
                        &mut instance_offset,
                        viewport_size,
                        &command_buffer,
                        texture,
                    );
                    command_encoder = new_command_encoder_for_texture(
                        &command_buffer,
                        texture,
                        viewport_size,
                        |color_attachment| {
                            color_attachment.setLoadAction(metal::MTLLoadAction::Load);
                        },
                    );
                    did_draw
                }
                PrimitiveBatch::Shadows(range) => self.draw_shadows(
                    &scene.shadows[range],
                    instance_buffer,
                    &mut instance_offset,
                    viewport_size,
                    &command_encoder,
                ),
                PrimitiveBatch::Quads(range) => self.draw_quads(
                    &scene.quads[range],
                    instance_buffer,
                    &mut instance_offset,
                    viewport_size,
                    &command_encoder,
                ),
                PrimitiveBatch::Effects(range) => self.draw_effects(
                    &scene.effects[range],
                    instance_buffer,
                    &mut instance_offset,
                    viewport_size,
                    &command_encoder,
                ),
                PrimitiveBatch::Particles(range) => {
                    command_encoder.setRenderPipelineState(&self.subtree_pipeline_state);
                    let base = scene.subtree_layers.len();
                    for index in range {
                        unsafe {
                            command_encoder.setFragmentTexture_atIndex(
                                Some(&subtree_textures[base + index]),
                                0_usize,
                            )
                        };
                        unsafe {
                            command_encoder.drawPrimitives_vertexStart_vertexCount(
                                metal::MTLPrimitiveType::Triangle,
                                0,
                                3,
                            )
                        };
                    }
                    true
                }
                PrimitiveBatch::Fluids(range) => {
                    command_encoder.setRenderPipelineState(&self.subtree_pipeline_state);
                    let base = scene.subtree_layers.len() + scene.particles.len();
                    for index in range {
                        unsafe {
                            command_encoder.setFragmentTexture_atIndex(
                                Some(&subtree_textures[base + index]),
                                0_usize,
                            )
                        };
                        unsafe {
                            command_encoder.drawPrimitives_vertexStart_vertexCount(
                                metal::MTLPrimitiveType::Triangle,
                                0,
                                3,
                            )
                        };
                    }
                    true
                }
                PrimitiveBatch::Paths(range) => {
                    let paths = &scene.paths[range];
                    command_encoder.endEncoding();

                    let did_draw = self.draw_paths_to_intermediate(
                        paths,
                        instance_buffer,
                        &mut instance_offset,
                        viewport_size,
                        &command_buffer,
                    );

                    command_encoder = new_command_encoder_for_texture(
                        &command_buffer,
                        texture,
                        viewport_size,
                        |color_attachment| {
                            color_attachment.setLoadAction(metal::MTLLoadAction::Load);
                        },
                    );

                    if did_draw {
                        self.draw_paths_from_intermediate(
                            paths,
                            instance_buffer,
                            &mut instance_offset,
                            viewport_size,
                            &command_encoder,
                        )
                    } else {
                        false
                    }
                }
                PrimitiveBatch::Underlines(range) => self.draw_underlines(
                    &scene.underlines[range],
                    instance_buffer,
                    &mut instance_offset,
                    viewport_size,
                    &command_encoder,
                ),
                PrimitiveBatch::MonochromeSprites { texture_id, range } => self
                    .draw_monochrome_sprites(
                        texture_id,
                        &scene.monochrome_sprites[range],
                        instance_buffer,
                        &mut instance_offset,
                        viewport_size,
                        &command_encoder,
                    ),
                PrimitiveBatch::PolychromeSprites { texture_id, range } => self
                    .draw_polychrome_sprites(
                        texture_id,
                        &scene.polychrome_sprites[range],
                        instance_buffer,
                        &mut instance_offset,
                        viewport_size,
                        &command_encoder,
                    ),
                PrimitiveBatch::Surfaces(range) => self.draw_surfaces(
                    &scene.surfaces[range],
                    instance_buffer,
                    &mut instance_offset,
                    viewport_size,
                    &command_encoder,
                ),
                PrimitiveBatch::SubpixelSprites { .. } => unreachable!(),
            };
            if !ok {
                command_encoder.endEncoding();
                anyhow::bail!(
                    "scene too large: {} paths, {} shadows, {} quads, {} underlines, {} mono, {} poly, {} surfaces",
                    scene.paths.len(),
                    scene.shadows.len(),
                    scene.quads.len(),
                    scene.underlines.len(),
                    scene.monochrome_sprites.len(),
                    scene.polychrome_sprites.len(),
                    scene.surfaces.len(),
                );
            }
        }

        command_encoder.endEncoding();

        if !self.is_unified_memory {
            // Sync the instance buffer to the GPU
            instance_buffer.metal_buffer.didModifyRange(NSRange {
                location: 0,
                length: instance_offset,
            });
        }

        Ok(command_buffer.to_owned())
    }

    fn draw_paths_to_intermediate(
        &self,
        paths: &[Path<ScaledPixels, gpui::GpuBackground>],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_buffer: &metal::CommandBufferRef,
    ) -> bool {
        if paths.is_empty() {
            return true;
        }
        let Some(intermediate_texture) = &self.path_intermediate_texture else {
            return false;
        };

        let render_pass_descriptor = metal::RenderPassDescriptor::new();
        let color_attachment = unsafe {
            render_pass_descriptor
                .colorAttachments()
                .objectAtIndexedSubscript(0)
        };
        color_attachment.setLoadAction(metal::MTLLoadAction::Clear);
        color_attachment.setClearColor(metal::clear_color(0., 0., 0., 0.));

        if let Some(msaa_texture) = &self.path_intermediate_msaa_texture {
            color_attachment.setTexture(Some(msaa_texture));
            color_attachment.setResolveTexture(Some(intermediate_texture));
            color_attachment.setStoreAction(metal::MTLStoreAction::MultisampleResolve);
        } else {
            color_attachment.setTexture(Some(intermediate_texture));
            color_attachment.setStoreAction(metal::MTLStoreAction::Store);
        }

        let command_encoder = command_buffer
            .renderCommandEncoderWithDescriptor(&render_pass_descriptor)
            .expect("Metal renderCommandEncoderWithDescriptor failed");
        command_encoder.setRenderPipelineState(&self.paths_rasterization_pipeline_state);

        align_offset(instance_offset);
        let mut vertices = Vec::new();
        for path in paths {
            vertices.extend(path.rasterization_vertices());
        }
        let vertices_bytes_len = mem::size_of_val(vertices.as_slice());
        let next_offset = *instance_offset + vertices_bytes_len;
        if next_offset > instance_buffer.size {
            command_encoder.endEncoding();
            return false;
        }
        Self::bind_shared_primitives(
            instance_buffer,
            true,
            *instance_offset,
            vertices_bytes_len,
            viewport_size,
            &command_encoder,
        );
        let buffer_contents = unsafe {
            (instance_buffer.metal_buffer.contents().as_ptr() as *mut u8).add(*instance_offset)
        };
        unsafe {
            ptr::copy_nonoverlapping(
                vertices.as_ptr() as *const u8,
                buffer_contents,
                vertices_bytes_len,
            );
        }
        unsafe {
            command_encoder.drawPrimitives_vertexStart_vertexCount(
                metal::MTLPrimitiveType::Triangle,
                0,
                vertices.len(),
            )
        };
        *instance_offset = next_offset;

        command_encoder.endEncoding();
        true
    }

    fn draw_shadows(
        &self,
        primitives: &[Shadow],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        self.draw_shared_primitives(
            primitives,
            &self.shadows_pipeline_state,
            false,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn draw_quads(
        &self,
        primitives: &[Quad],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        self.draw_shared_primitives(
            primitives,
            &self.quads_pipeline_state,
            true,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn bind_shared_primitives(
        instance_buffer: &InstanceBuffer,
        uses_gradients: bool,
        instance_offset: usize,
        bytes_len: usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) {
        unsafe {
            command_encoder.setFragmentBuffer_offset_atIndex(
                instance_buffer.gradients.as_deref(),
                0,
                gpui_render::METAL_GRADIENTS_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&instance_buffer.metal_buffer),
                instance_offset,
                gpui_render::METAL_INSTANCES_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&instance_buffer.metal_buffer),
                instance_offset,
                gpui_render::METAL_INSTANCES_SLOT as usize,
            )
        };

        let globals = gpui_render::PrimitiveGlobals {
            viewport_size: [
                i32::from(viewport_size.width) as f32,
                i32::from(viewport_size.height) as f32,
            ],
            ..Default::default()
        };
        let primitive_size =
            u32::try_from(bytes_len).expect("primitive buffer exceeds Metal address space");
        let gradient_size = instance_buffer
            .gradients
            .as_ref()
            .map_or(0, |buffer| buffer.length() as u32);
        let sizes = if uses_gradients {
            [gradient_size, primitive_size]
        } else {
            [primitive_size, 0]
        };
        unsafe {
            command_encoder.setVertexBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((&globals as *const _) as *mut c_void).unwrap(),
                mem::size_of_val(&globals) as usize,
                gpui_render::METAL_GLOBALS_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((&globals as *const _) as *mut c_void).unwrap(),
                mem::size_of_val(&globals) as usize,
                gpui_render::METAL_GLOBALS_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setVertexBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((sizes.as_ptr()) as *mut c_void).unwrap(),
                mem::size_of_val(&sizes) as usize,
                gpui_render::METAL_SIZES_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((sizes.as_ptr()) as *mut c_void).unwrap(),
                mem::size_of_val(&sizes) as usize,
                gpui_render::METAL_SIZES_SLOT as usize,
            )
        };
    }

    fn draw_shared_primitives<T>(
        &self,
        primitives: &[T],
        pipeline: &metal::RenderPipelineStateRef,
        uses_gradients: bool,
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        if primitives.is_empty() {
            return true;
        }
        align_offset(instance_offset);

        let bytes_len = mem::size_of_val(primitives);
        let next_offset = *instance_offset + bytes_len;
        if next_offset > instance_buffer.size {
            return false;
        }

        command_encoder.setRenderPipelineState(pipeline);
        Self::bind_shared_primitives(
            instance_buffer,
            uses_gradients,
            *instance_offset,
            bytes_len,
            viewport_size,
            command_encoder,
        );
        let buffer_contents = unsafe {
            (instance_buffer.metal_buffer.contents().as_ptr() as *mut u8).add(*instance_offset)
        };

        unsafe {
            ptr::copy_nonoverlapping(primitives.as_ptr() as *const u8, buffer_contents, bytes_len);
        }

        unsafe {
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                metal::MTLPrimitiveType::TriangleStrip,
                0,
                4,
                primitives.len(),
            )
        };
        *instance_offset = next_offset;
        true
    }

    fn ensure_effect_pipelines(&mut self, scene: &Scene) {
        let shaders = scene
            .effects
            .iter()
            .map(|effect| effect.shader.clone())
            .collect::<Vec<_>>();

        for shader in shaders {
            let key = shader.id().as_u64();
            if self.effect_pipeline_states.contains_key(&key)
                || self.failed_effect_pipeline_states.contains(&key)
            {
                continue;
            }

            let source = match shader.msl_source() {
                Some(source) => Ok(source.to_owned()),
                None => translate_effect_to_msl(&shader),
            };
            let result = source.and_then(|source| {
                let library = metal::library_from_source(
                    &self.device,
                    &source,
                    &metal::CompileOptions::new(),
                )
                .map_err(|error| anyhow::anyhow!("MSL compile error: {error}"))?;
                try_build_effect_pipeline_state(
                    &self.device,
                    &library,
                    "gpui_effect",
                    "vs_effect",
                    "fs_effect",
                    MTLPixelFormat::BGRA8Unorm,
                )
            });

            match result {
                Ok(pipeline) => {
                    self.effect_pipeline_states.insert(key, pipeline);
                }
                Err(error) => {
                    log::error!("failed to compile GPUI Metal effect {key:016x}: {error:#}");
                    self.failed_effect_pipeline_states.insert(key);
                }
            }
        }
    }

    fn ensure_backdrop_effect_pipelines(&mut self, scene: &Scene) {
        let default_shader = BackdropShader::wgsl(DEFAULT_BACKDROP_EFFECT);
        let shaders = std::iter::once(default_shader).chain(
            scene
                .backdrop_blurs
                .iter()
                .filter_map(|backdrop| backdrop.shader.clone()),
        );
        for shader in shaders {
            let key = shader.id().as_u64();
            if self.backdrop_effect_pipeline_states.contains_key(&key)
                || self.failed_backdrop_effect_pipeline_states.contains(&key)
            {
                continue;
            }
            let result = translate_backdrop_to_msl(&shader).and_then(|source| {
                let library = metal::library_from_source(
                    &self.device,
                    &source,
                    &metal::CompileOptions::new(),
                )
                .map_err(|error| anyhow::anyhow!("MSL compile error: {error}"))?;
                try_build_effect_pipeline_state(
                    &self.device,
                    &library,
                    "gpui_backdrop_effect",
                    "vs_backdrop",
                    "fs_backdrop",
                    MTLPixelFormat::BGRA8Unorm,
                )
            });
            match result {
                Ok(pipeline) => {
                    self.backdrop_effect_pipeline_states.insert(key, pipeline);
                }
                Err(error) => {
                    log::error!(
                        "failed to compile GPUI Metal backdrop effect {key:016x}: {error:#}"
                    );
                    self.failed_backdrop_effect_pipeline_states.insert(key);
                }
            }
        }
    }

    fn update_backdrop_textures(&mut self, viewport_size: Size<DevicePixels>) {
        let width = viewport_size.width.0.max(1) as usize;
        let height = viewport_size.height.0.max(1) as usize;
        let needs_update = self
            .backdrop_source_texture
            .as_ref()
            .is_none_or(|texture| texture.width() != width || texture.height() != height);
        if !needs_update {
            return;
        }
        let descriptor = metal::TextureDescriptor::new();
        unsafe { descriptor.setWidth(width) };
        unsafe { descriptor.setHeight(height) };
        descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        descriptor.setUsage(
            metal::MTLTextureUsage::ShaderRead
                | metal::MTLTextureUsage::ShaderWrite
                | metal::MTLTextureUsage::RenderTarget,
        );
        descriptor.setStorageMode(metal::MTLStorageMode::Private);
        self.backdrop_source_texture = Some(
            self.device
                .newTextureWithDescriptor(&descriptor)
                .expect("Metal newTextureWithDescriptor failed"),
        );
        self.backdrop_blurred_texture = Some(
            self.device
                .newTextureWithDescriptor(&descriptor)
                .expect("Metal newTextureWithDescriptor failed"),
        );
    }

    fn draw_backdrop_blurs(
        &mut self,
        backdrops: &[BackdropBlur],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_buffer: &metal::CommandBufferRef,
        target: &metal::TextureRef,
    ) -> bool {
        self.update_backdrop_textures(viewport_size);
        let (Some(source), Some(blurred)) = (
            self.backdrop_source_texture.as_ref(),
            self.backdrop_blurred_texture.as_ref(),
        ) else {
            return true;
        };
        let default_shader = BackdropShader::wgsl(DEFAULT_BACKDROP_EFFECT);
        for backdrop in backdrops {
            let blit = command_buffer
                .blitCommandEncoder()
                .expect("Metal blitCommandEncoder failed");
            let size = metal::MTLSize {
                width: target.width(),
                height: target.height(),
                depth: 1,
            };
            let origin = metal::MTLOrigin { x: 0, y: 0, z: 0 };
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(target, 0, 0, origin, size, source, 0, 0, origin)
            };
            blit.endEncoding();

            // MPSImageGaussianBlur is available on every macOS version supported by GPUI.
            // Calling it dynamically keeps MetalPerformanceShaders out of the public API.
            unsafe {
                let Some(class) = objc2::runtime::AnyClass::get(c"MPSImageGaussianBlur") else {
                    return true;
                };
                let filter: *mut objc2::runtime::AnyObject = msg_send![class, alloc];
                let filter: *mut objc2::runtime::AnyObject = msg_send![
                    filter,
                    initWithDevice: &*self.device,
                    sigma: backdrop.blur_radius.0.max(0.01)
                ];
                let _: () = msg_send![
                    filter,
                    encodeToCommandBuffer: command_buffer,
                    sourceTexture: &**source,
                    destinationTexture: &**blurred
                ];
                let _: () = msg_send![filter, release];
            }

            let shader = backdrop.shader.as_ref().unwrap_or(&default_shader);
            let Some(pipeline) = self
                .backdrop_effect_pipeline_states
                .get(&shader.id().as_u64())
            else {
                continue;
            };
            let instance = BackdropInstance::from(backdrop);
            align_offset(instance_offset);
            let bytes_len = mem::size_of::<BackdropInstance>();
            let next_offset = *instance_offset + bytes_len;
            if next_offset > instance_buffer.size {
                return false;
            }
            unsafe {
                ptr::copy_nonoverlapping(
                    &instance as *const BackdropInstance as *const u8,
                    (instance_buffer.metal_buffer.contents().as_ptr() as *mut u8)
                        .add(*instance_offset),
                    bytes_len,
                );
            }
            let globals = EffectGlobalParams {
                viewport_size: [viewport_size.width.0 as f32, viewport_size.height.0 as f32],
                premultiplied_alpha: 0,
                pad: 0,
                viewport_origin: [0.; 2],
                origin_pad: [0; 2],
            };
            let buffer_sizes = [bytes_len as u32];
            let encoder = new_command_encoder_for_texture(
                command_buffer,
                target,
                viewport_size,
                |attachment| attachment.setLoadAction(metal::MTLLoadAction::Load),
            );
            encoder.setRenderPipelineState(pipeline);
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (&globals as *const _ as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&globals) as usize,
                    0_usize,
                )
            };
            unsafe {
                encoder.setFragmentBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (&globals as *const _ as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&globals) as usize,
                    0_usize,
                )
            };
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(
                    Some(&instance_buffer.metal_buffer),
                    *instance_offset,
                    1_usize,
                )
            };
            unsafe {
                encoder.setFragmentBuffer_offset_atIndex(
                    Some(&instance_buffer.metal_buffer),
                    *instance_offset,
                    1_usize,
                )
            };
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (buffer_sizes.as_ptr() as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&buffer_sizes) as usize,
                    2_usize,
                )
            };
            unsafe {
                encoder.setFragmentBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (buffer_sizes.as_ptr() as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&buffer_sizes) as usize,
                    2_usize,
                )
            };
            unsafe { encoder.setFragmentTexture_atIndex(Some(source), 0_usize) };
            unsafe { encoder.setFragmentTexture_atIndex(Some(blurred), 1_usize) };
            unsafe { encoder.setFragmentSamplerState_atIndex(Some(&self.effect_sampler), 0_usize) };
            unsafe {
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                    metal::MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                    1,
                )
            };
            encoder.endEncoding();
            *instance_offset = next_offset;
        }
        true
    }

    fn draw_effects(
        &self,
        effects: &[EffectQuad],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        let mut start = 0;
        while start < effects.len() {
            let shader_id = effects[start].shader.id().as_u64();
            let texture_id = effects[start].image_tile.map(|tile| tile.texture_id);
            let second_texture_id = effects[start].second_image_tile.map(|tile| tile.texture_id);
            let third_texture_id = effects[start].third_image_tile.map(|tile| tile.texture_id);
            let fourth_texture_id = effects[start].fourth_image_tile.map(|tile| tile.texture_id);
            let mut end = start + 1;
            while end < effects.len()
                && effects[end].shader.id().as_u64() == shader_id
                && effects[end].image_tile.map(|tile| tile.texture_id) == texture_id
                && effects[end].second_image_tile.map(|tile| tile.texture_id) == second_texture_id
                && effects[end].third_image_tile.map(|tile| tile.texture_id) == third_texture_id
                && effects[end].fourth_image_tile.map(|tile| tile.texture_id) == fourth_texture_id
            {
                end += 1;
            }

            let Some(pipeline) = self.effect_pipeline_states.get(&shader_id) else {
                start = end;
                continue;
            };
            let instances = effects[start..end]
                .iter()
                .map(EffectInstance::from)
                .collect::<Vec<_>>();
            align_offset(instance_offset);
            let bytes_len = mem::size_of_val(instances.as_slice());
            let next_offset = *instance_offset + bytes_len;
            if next_offset > instance_buffer.size {
                return false;
            }

            let buffer_contents = unsafe {
                (instance_buffer.metal_buffer.contents().as_ptr() as *mut u8).add(*instance_offset)
            };
            unsafe {
                ptr::copy_nonoverlapping(
                    instances.as_ptr() as *const u8,
                    buffer_contents,
                    bytes_len,
                );
            }

            let globals = EffectGlobalParams {
                viewport_size: [viewport_size.width.0 as f32, viewport_size.height.0 as f32],
                premultiplied_alpha: 0,
                pad: 0,
                viewport_origin: [0.; 2],
                origin_pad: [0; 2],
            };
            let buffer_sizes = [bytes_len as u32];
            command_encoder.setRenderPipelineState(pipeline);
            unsafe {
                command_encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (&globals as *const EffectGlobalParams as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&globals) as usize,
                    0_usize,
                )
            };
            unsafe {
                command_encoder.setFragmentBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (&globals as *const EffectGlobalParams as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&globals) as usize,
                    0_usize,
                )
            };
            unsafe {
                command_encoder.setVertexBuffer_offset_atIndex(
                    Some(&instance_buffer.metal_buffer),
                    *instance_offset,
                    1_usize,
                )
            };
            unsafe {
                command_encoder.setFragmentBuffer_offset_atIndex(
                    Some(&instance_buffer.metal_buffer),
                    *instance_offset,
                    1_usize,
                )
            };
            unsafe {
                command_encoder.setVertexBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (buffer_sizes.as_ptr() as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&buffer_sizes) as usize,
                    2_usize,
                )
            };
            unsafe {
                command_encoder.setFragmentBytes_length_atIndex(
                    std::ptr::NonNull::<c_void>::new(
                        (buffer_sizes.as_ptr() as *const _) as *mut c_void,
                    )
                    .unwrap(),
                    mem::size_of_val(&buffer_sizes) as usize,
                    2_usize,
                )
            };
            if effects[start].shader.uses_image() {
                let Some(texture_id) = texture_id else {
                    start = end;
                    continue;
                };
                let texture = self.sprite_atlas.metal_texture(texture_id);
                unsafe { command_encoder.setFragmentTexture_atIndex(Some(&texture), 0_usize) };
                unsafe {
                    command_encoder
                        .setFragmentSamplerState_atIndex(Some(&self.effect_sampler), 0_usize)
                };
            }
            if effects[start].shader.image_count() >= 2 {
                let Some(second_texture_id) = second_texture_id else {
                    start = end;
                    continue;
                };
                let texture = self.sprite_atlas.metal_texture(second_texture_id);
                unsafe { command_encoder.setFragmentTexture_atIndex(Some(&texture), 1_usize) };
            }
            if effects[start].shader.image_count() >= 4 {
                let (Some(third_texture_id), Some(fourth_texture_id)) =
                    (third_texture_id, fourth_texture_id)
                else {
                    start = end;
                    continue;
                };
                let third_texture = self.sprite_atlas.metal_texture(third_texture_id);
                let fourth_texture = self.sprite_atlas.metal_texture(fourth_texture_id);
                unsafe {
                    command_encoder.setFragmentTexture_atIndex(Some(&third_texture), 2_usize)
                };
                unsafe {
                    command_encoder.setFragmentTexture_atIndex(Some(&fourth_texture), 3_usize)
                };
            }
            unsafe {
                command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                    metal::MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                    instances.len(),
                )
            };
            *instance_offset = next_offset;
            start = end;
        }
        true
    }

    fn draw_paths_from_intermediate(
        &self,
        paths: &[Path<ScaledPixels, gpui::GpuBackground>],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        let Some(first_path) = paths.first() else {
            return true;
        };

        let Some(ref intermediate_texture) = self.path_intermediate_texture else {
            return false;
        };

        unsafe {
            command_encoder.setFragmentTexture_atIndex(
                Some(intermediate_texture),
                gpui_render::METAL_TEXTURE_SLOT as usize,
            )
        };

        // When copying paths from the intermediate texture to the drawable,
        // each pixel must only be copied once, in case of transparent paths.
        //
        // If all paths have the same draw order, then their bounds are all
        // disjoint, so we can copy each path's bounds individually. If this
        // batch combines different draw orders, we perform a single copy
        // for a minimal spanning rect.
        let sprites;
        if paths.last().unwrap().order == first_path.order {
            sprites = paths
                .iter()
                .map(|path| PathSprite {
                    bounds: path.clipped_bounds(),
                })
                .collect();
        } else {
            let mut bounds = first_path.clipped_bounds();
            for path in paths.iter().skip(1) {
                bounds = bounds.union(&path.clipped_bounds());
            }
            sprites = vec![PathSprite { bounds }];
        }

        self.draw_shared_primitives(
            &sprites,
            &self.path_sprites_pipeline_state,
            false,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn draw_underlines(
        &self,
        primitives: &[Underline],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        self.draw_shared_primitives(
            primitives,
            &self.underlines_pipeline_state,
            false,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn draw_monochrome_sprites(
        &self,
        texture_id: AtlasTextureId,
        sprites: &[MonochromeSprite],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        let texture = self.sprite_atlas.metal_texture(texture_id);
        unsafe {
            command_encoder
                .setVertexTexture_atIndex(Some(&texture), gpui_render::METAL_TEXTURE_SLOT as usize)
        };
        unsafe {
            command_encoder.setFragmentTexture_atIndex(
                Some(&texture),
                gpui_render::METAL_TEXTURE_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentSamplerState_atIndex(
                Some(&self.effect_sampler),
                gpui_render::METAL_SAMPLER_SLOT as usize,
            )
        };
        let gamma = gpui_render::GammaParams::default();
        unsafe {
            command_encoder.setFragmentBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((&gamma as *const _ as *const _) as *mut c_void)
                    .unwrap(),
                mem::size_of_val(&gamma) as usize,
                gpui_render::METAL_GAMMA_SLOT as usize,
            )
        };
        self.draw_shared_primitives(
            sprites,
            &self.monochrome_sprites_pipeline_state,
            true,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn draw_polychrome_sprites(
        &self,
        texture_id: AtlasTextureId,
        sprites: &[PolychromeSprite],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        let texture = self.sprite_atlas.metal_texture(texture_id);
        unsafe {
            command_encoder
                .setVertexTexture_atIndex(Some(&texture), gpui_render::METAL_TEXTURE_SLOT as usize)
        };
        unsafe {
            command_encoder.setFragmentTexture_atIndex(
                Some(&texture),
                gpui_render::METAL_TEXTURE_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentSamplerState_atIndex(
                Some(&self.effect_sampler),
                gpui_render::METAL_SAMPLER_SLOT as usize,
            )
        };
        self.draw_shared_primitives(
            sprites,
            &self.polychrome_sprites_pipeline_state,
            false,
            instance_buffer,
            instance_offset,
            viewport_size,
            command_encoder,
        )
    }

    fn draw_surfaces(
        &mut self,
        surfaces: &[PaintSurface],
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        viewport_size: Size<DevicePixels>,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        self.surfaces.retain(|_, cached| cached.owner.is_alive());
        let globals = gpui_render::PrimitiveGlobals {
            viewport_size: [viewport_size.width.0 as f32, viewport_size.height.0 as f32],
            ..Default::default()
        };
        unsafe {
            command_encoder.setVertexBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((&globals as *const _ as *const _) as *mut c_void)
                    .unwrap(),
                mem::size_of_val(&globals) as usize,
                gpui_render::METAL_GLOBALS_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentBytes_length_atIndex(
                std::ptr::NonNull::<c_void>::new((&globals as *const _ as *const _) as *mut c_void)
                    .unwrap(),
                mem::size_of_val(&globals) as usize,
                gpui_render::METAL_GLOBALS_SLOT as usize,
            )
        };

        for surface in surfaces {
            match &surface.source {
                gpui::SurfaceSource::Frame(frame) => {
                    let params = surface_bounds(surface, Some(frame));
                    match frame.backing() {
                        SurfaceFrameBacking::Cpu(_) => {
                            let Some(textures) = self.cpu_surface_textures(frame) else {
                                continue;
                            };
                            let rendered = match &textures {
                                CachedSurfaceTextures::Rgba(texture) => self.draw_surface_textures(
                                    params,
                                    texture,
                                    texture,
                                    false,
                                    instance_buffer,
                                    instance_offset,
                                    command_encoder,
                                ),
                                CachedSurfaceTextures::Nv12 { y, uv } => self
                                    .draw_surface_textures(
                                        params,
                                        y,
                                        uv,
                                        true,
                                        instance_buffer,
                                        instance_offset,
                                        command_encoder,
                                    ),
                            };
                            if !rendered {
                                return false;
                            }
                        }
                        SurfaceFrameBacking::CoreVideo(core_video) => {
                            // SAFETY: The CoreVideoHandle contract freezes the
                            // published buffer; Metal only samples it here.
                            let pixel_buffer = unsafe { core_video.pixel_buffer() };
                            let textures = match self
                                .core_video_textures(pixel_buffer, frame.format())
                            {
                                Ok(textures) => textures,
                                Err(error) => {
                                    log::error!("failed to import CoreVideo surface: {error:#}");
                                    continue;
                                }
                            };
                            let rendered = self.draw_core_video_surface(
                                params,
                                &textures,
                                instance_buffer,
                                instance_offset,
                                command_encoder,
                            );
                            match rendered {
                                Ok(true) => {}
                                Ok(false) => return false,
                                Err(error) => {
                                    log::error!(
                                        "failed to access CoreVideo Metal texture: {error:#}"
                                    );
                                }
                            }
                        }
                    }
                }
                gpui::SurfaceSource::Surface(image_buffer) => {
                    let params = surface_bounds(surface, None);
                    let textures = match self.core_video_textures(image_buffer, SurfaceFormat::Nv12)
                    {
                        Ok(textures) => textures,
                        Err(error) => {
                            log::error!("failed to import legacy CoreVideo surface: {error:#}");
                            continue;
                        }
                    };
                    let rendered = self.draw_core_video_surface(
                        params,
                        &textures,
                        instance_buffer,
                        instance_offset,
                        command_encoder,
                    );
                    match rendered {
                        Ok(true) => {}
                        Ok(false) => return false,
                        Err(error) => {
                            log::error!(
                                "failed to access legacy CoreVideo Metal texture: {error:#}"
                            );
                        }
                    }
                }
            }
        }
        true
    }

    fn cpu_surface_textures(&mut self, frame: &SurfaceFrame) -> Option<CachedSurfaceTextures> {
        let id = frame.handle().id();
        let recreate = self.surfaces.get(&id).is_none_or(|cached| {
            cached.format != frame.format() || cached.size != frame.coded_size()
        });
        if recreate {
            let textures = self.create_surface_textures(frame);
            upload_surface(&textures, frame);
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
            return self.surfaces.get(&id).map(|cached| cached.textures.clone());
        }

        let cached = self.surfaces.get_mut(&id)?;
        if cached.sequence != frame.sequence() {
            upload_surface(&cached.textures, frame);
            cached.sequence = frame.sequence();
        }
        Some(cached.textures.clone())
    }

    fn create_surface_textures(&self, frame: &SurfaceFrame) -> CachedSurfaceTextures {
        let size = frame.coded_size();
        let width = size.width.0 as usize;
        let height = size.height.0 as usize;
        let create = |pixel_format, width, height| {
            let descriptor = metal::TextureDescriptor::new();
            unsafe { descriptor.setWidth(width) };
            unsafe { descriptor.setHeight(height) };
            descriptor.setPixelFormat(pixel_format);
            descriptor.setUsage(metal::MTLTextureUsage::ShaderRead);
            descriptor.setStorageMode(if self.is_apple_gpu {
                metal::MTLStorageMode::Shared
            } else {
                metal::MTLStorageMode::Managed
            });
            self.device
                .newTextureWithDescriptor(&descriptor)
                .expect("Metal newTextureWithDescriptor failed")
        };

        match frame.format() {
            SurfaceFormat::Bgra8 => {
                CachedSurfaceTextures::Rgba(create(MTLPixelFormat::BGRA8Unorm, width, height))
            }
            SurfaceFormat::Rgba8 => {
                CachedSurfaceTextures::Rgba(create(MTLPixelFormat::RGBA8Unorm, width, height))
            }
            SurfaceFormat::Nv12 => CachedSurfaceTextures::Nv12 {
                y: create(MTLPixelFormat::R8Unorm, width, height),
                uv: create(
                    MTLPixelFormat::RG8Unorm,
                    width.div_ceil(2),
                    height.div_ceil(2),
                ),
            },
        }
    }

    fn core_video_textures(
        &self,
        image_buffer: &CVPixelBuffer,
        format: SurfaceFormat,
    ) -> Result<CoreVideoTextures> {
        let image = image_buffer;
        match format {
            SurfaceFormat::Bgra8 | SurfaceFormat::Rgba8 => {
                let pixel_format = match format {
                    SurfaceFormat::Bgra8 => MTLPixelFormat::BGRA8Unorm,
                    SurfaceFormat::Rgba8 => MTLPixelFormat::RGBA8Unorm,
                    SurfaceFormat::Nv12 => unreachable!(),
                };
                Ok(CoreVideoTextures::Rgba(
                    self.core_video_texture_cache
                        .import(
                            image,
                            pixel_format,
                            CVPixelBufferGetWidth(&image_buffer),
                            CVPixelBufferGetHeight(&image_buffer),
                            0,
                        )
                        .map_err(|code| anyhow::anyhow!("CVMetalTextureCache returned {code}"))?,
                ))
            }
            SurfaceFormat::Nv12 => {
                anyhow::ensure!(
                    CVPixelBufferGetPlaneCount(&image_buffer) >= 2,
                    "NV12 CoreVideo buffer has fewer than two planes"
                );
                let y = self
                    .core_video_texture_cache
                    .import(
                        image,
                        MTLPixelFormat::R8Unorm,
                        CVPixelBufferGetWidthOfPlane(&image_buffer, 0),
                        CVPixelBufferGetHeightOfPlane(&image_buffer, 0),
                        0,
                    )
                    .map_err(|code| anyhow::anyhow!("CVMetalTextureCache returned {code}"))?;
                let uv = self
                    .core_video_texture_cache
                    .import(
                        image,
                        MTLPixelFormat::RG8Unorm,
                        CVPixelBufferGetWidthOfPlane(&image_buffer, 1),
                        CVPixelBufferGetHeightOfPlane(&image_buffer, 1),
                        1,
                    )
                    .map_err(|code| anyhow::anyhow!("CVMetalTextureCache returned {code}"))?;
                Ok(CoreVideoTextures::Nv12 { y, uv })
            }
        }
    }

    fn draw_core_video_surface(
        &self,
        params: SurfaceParams,
        textures: &CoreVideoTextures,
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> Result<bool> {
        match textures {
            CoreVideoTextures::Rgba(texture) => {
                let texture = core_video_texture_ref(texture)?;
                Ok(self.draw_surface_textures(
                    params,
                    texture,
                    texture,
                    false,
                    instance_buffer,
                    instance_offset,
                    command_encoder,
                ))
            }
            CoreVideoTextures::Nv12 { y, uv } => {
                let y = core_video_texture_ref(y)?;
                let uv = core_video_texture_ref(uv)?;
                Ok(self.draw_surface_textures(
                    params,
                    y,
                    uv,
                    true,
                    instance_buffer,
                    instance_offset,
                    command_encoder,
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_surface_textures(
        &self,
        params: SurfaceParams,
        first_texture: &metal::TextureRef,
        second_texture: &metal::TextureRef,
        nv12: bool,
        instance_buffer: &mut InstanceBuffer,
        instance_offset: &mut usize,
        command_encoder: &metal::RenderCommandEncoderRef,
    ) -> bool {
        align_offset(instance_offset);
        let next_offset = *instance_offset + mem::size_of::<SurfaceParams>();
        if next_offset > instance_buffer.size {
            return false;
        }

        command_encoder.setRenderPipelineState(if nv12 {
            &self.surfaces_nv12_pipeline_state
        } else {
            &self.surfaces_rgba_pipeline_state
        });
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&instance_buffer.metal_buffer),
                *instance_offset,
                gpui_render::METAL_INSTANCES_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&instance_buffer.metal_buffer),
                *instance_offset,
                gpui_render::METAL_INSTANCES_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentTexture_atIndex(
                Some(first_texture),
                gpui_render::METAL_TEXTURE_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentTexture_atIndex(
                Some(second_texture),
                gpui_render::METAL_CHROMA_TEXTURE_SLOT as usize,
            )
        };
        unsafe {
            command_encoder.setFragmentSamplerState_atIndex(
                Some(&self.effect_sampler),
                gpui_render::METAL_SAMPLER_SLOT as usize,
            )
        };

        unsafe {
            let buffer_contents = (instance_buffer.metal_buffer.contents().as_ptr() as *mut u8)
                .add(*instance_offset) as *mut SurfaceParams;
            ptr::write(buffer_contents, params);
        }

        unsafe {
            command_encoder.drawPrimitives_vertexStart_vertexCount(
                metal::MTLPrimitiveType::TriangleStrip,
                0,
                4,
            )
        };
        *instance_offset = next_offset;
        true
    }
}

fn core_video_texture_ref(texture: &CVMetalTexture) -> Result<&metal::TextureRef> {
    Ok(&texture.texture)
}

fn upload_surface(textures: &CachedSurfaceTextures, frame: &SurfaceFrame) {
    let size = frame.coded_size();
    let width = size.width.0 as usize;
    let height = size.height.0 as usize;
    let planes = frame
        .cpu_planes()
        .expect("CPU surface upload requires CPU planes");
    let upload =
        |texture: &metal::TextureRef, plane: &gpui::SurfacePlane, width: usize, height: usize| {
            let bytes = unsafe { plane.bytes().as_ptr().add(plane.offset()) };
            unsafe {
                texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                    metal::region_2d(0, 0, width, height),
                    0,
                    std::ptr::NonNull::new(bytes.cast_mut().cast()).unwrap(),
                    plane.stride() as usize,
                )
            };
        };

    match textures {
        CachedSurfaceTextures::Rgba(texture) => upload(texture, &planes[0], width, height),
        CachedSurfaceTextures::Nv12 { y, uv } => {
            upload(y, &planes[0], width, height);
            upload(uv, &planes[1], width.div_ceil(2), height.div_ceil(2));
        }
    }
}

fn surface_bounds(surface: &PaintSurface, frame: Option<&SurfaceFrame>) -> SurfaceParams {
    let (uv, color) = frame.map_or(
        (
            Bounds::new(point(0.0, 0.0), size(1.0, 1.0)),
            SurfaceColorInfo {
                matrix: YuvMatrix::Bt601,
                range: ColorRange::Full,
            },
        ),
        |frame| (frame.normalized_visible_rect(), frame.color()),
    );

    let rect = |bounds: Bounds<ScaledPixels>| {
        [
            bounds.origin.x.0,
            bounds.origin.y.0,
            bounds.size.width.0,
            bounds.size.height.0,
        ]
    };
    SurfaceParams {
        bounds: rect(surface.bounds),
        clip_bounds: rect(surface.clip_bounds),
        content_mask: rect(surface.content_mask.bounds),
        corner_radii: [
            surface.corner_radii.top_left.0,
            surface.corner_radii.top_right.0,
            surface.corner_radii.bottom_right.0,
            surface.corner_radii.bottom_left.0,
        ],
        uv_bounds: [uv.origin.x, uv.origin.y, uv.size.width, uv.size.height],
        color_rows: color.yuv_to_rgb_matrix(),
        opacity: surface.opacity,
        _pad: [0.0; 3],
    }
}

fn new_command_encoder_for_texture<'a>(
    command_buffer: &'a metal::CommandBufferRef,
    texture: &'a metal::TextureRef,
    viewport_size: Size<DevicePixels>,
    configure_color_attachment: impl Fn(&RenderPassColorAttachmentDescriptorRef),
) -> metal::RenderCommandEncoder {
    let render_pass_descriptor = metal::RenderPassDescriptor::new();
    let color_attachment = unsafe {
        render_pass_descriptor
            .colorAttachments()
            .objectAtIndexedSubscript(0)
    };
    color_attachment.setTexture(Some(texture));
    color_attachment.setStoreAction(metal::MTLStoreAction::Store);
    configure_color_attachment(&color_attachment);

    let command_encoder = command_buffer
        .renderCommandEncoderWithDescriptor(&render_pass_descriptor)
        .expect("Metal renderCommandEncoderWithDescriptor failed");
    command_encoder.setViewport(metal::MTLViewport {
        originX: 0.0,
        originY: 0.0,
        width: i32::from(viewport_size.width) as f64,
        height: i32::from(viewport_size.height) as f64,
        znear: 0.0,
        zfar: 1.0,
    });
    command_encoder
}

fn translate_effect_to_msl(shader: &EffectShader) -> Result<String> {
    gpui_render::native::to_msl(
        &gpui::compose_effect_shader_wgsl(shader),
        gpui_render::native::ShaderKind::Effect {
            image_count: shader.image_count(),
        },
    )
}

fn translate_backdrop_to_msl(shader: &BackdropShader) -> Result<String> {
    gpui_render::native::to_msl(
        &gpui::compose_backdrop_shader_wgsl(shader),
        gpui_render::native::ShaderKind::Backdrop,
    )
}

fn try_build_effect_pipeline_state(
    device: &metal::DeviceRef,
    library: &metal::LibraryRef,
    label: &str,
    vertex_fn_name: &str,
    fragment_fn_name: &str,
    pixel_format: metal::MTLPixelFormat,
) -> Result<metal::RenderPipelineState> {
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str(vertex_fn_name))
        .ok_or_else(|| anyhow::anyhow!("missing vertex function"))?;
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str(fragment_fn_name))
        .ok_or_else(|| anyhow::anyhow!("missing fragment function"))?;

    let descriptor = metal::RenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(label)));
    descriptor.setVertexFunction(Some(vertex_fn.as_ref()));
    descriptor.setFragmentFunction(Some(fragment_fn.as_ref()));
    let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    color_attachment.setPixelFormat(pixel_format);
    color_attachment.setBlendingEnabled(true);
    color_attachment.setRgbBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setAlphaBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setSourceRGBBlendFactor(metal::MTLBlendFactor::SourceAlpha);
    color_attachment.setSourceAlphaBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setDestinationRGBBlendFactor(metal::MTLBlendFactor::OneMinusSourceAlpha);
    color_attachment.setDestinationAlphaBlendFactor(metal::MTLBlendFactor::One);

    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(|error| anyhow::anyhow!("Metal pipeline error: {error}"))
}

fn build_pipeline_state(
    device: &metal::DeviceRef,
    libraries: &HashMap<ShaderLibrary, metal::Library>,
    program: ShaderProgram,
    pixel_format: metal::MTLPixelFormat,
) -> metal::RenderPipelineState {
    let program = program.program();
    let library = &libraries[&program.library];
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str(program.vertex))
        .expect("error locating vertex function");
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str(program.fragment))
        .expect("error locating fragment function");

    let descriptor = metal::RenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(program.label)));
    descriptor.setVertexFunction(Some(vertex_fn.as_ref()));
    descriptor.setFragmentFunction(Some(fragment_fn.as_ref()));
    let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    color_attachment.setPixelFormat(pixel_format);
    color_attachment.setBlendingEnabled(true);
    color_attachment.setRgbBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setAlphaBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setSourceRGBBlendFactor(metal::MTLBlendFactor::SourceAlpha);
    color_attachment.setSourceAlphaBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setDestinationRGBBlendFactor(metal::MTLBlendFactor::OneMinusSourceAlpha);
    color_attachment.setDestinationAlphaBlendFactor(metal::MTLBlendFactor::One);

    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .expect("could not create render pipeline state")
}

fn build_path_sprite_pipeline_state(
    device: &metal::DeviceRef,
    libraries: &HashMap<ShaderLibrary, metal::Library>,
    program: ShaderProgram,
    pixel_format: metal::MTLPixelFormat,
) -> metal::RenderPipelineState {
    let program = program.program();
    let library = &libraries[&program.library];
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str(program.vertex))
        .expect("error locating vertex function");
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str(program.fragment))
        .expect("error locating fragment function");

    let descriptor = metal::RenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(program.label)));
    descriptor.setVertexFunction(Some(vertex_fn.as_ref()));
    descriptor.setFragmentFunction(Some(fragment_fn.as_ref()));
    let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    color_attachment.setPixelFormat(pixel_format);
    color_attachment.setBlendingEnabled(true);
    color_attachment.setRgbBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setAlphaBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setSourceRGBBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setSourceAlphaBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setDestinationRGBBlendFactor(metal::MTLBlendFactor::OneMinusSourceAlpha);
    color_attachment.setDestinationAlphaBlendFactor(metal::MTLBlendFactor::One);

    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .expect("could not create render pipeline state")
}

fn build_path_rasterization_pipeline_state(
    device: &metal::DeviceRef,
    libraries: &HashMap<ShaderLibrary, metal::Library>,
    program: ShaderProgram,
    pixel_format: metal::MTLPixelFormat,
    path_sample_count: u32,
) -> metal::RenderPipelineState {
    let program = program.program();
    let library = &libraries[&program.library];
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str(program.vertex))
        .expect("error locating vertex function");
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str(program.fragment))
        .expect("error locating fragment function");

    let descriptor = metal::RenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(program.label)));
    descriptor.setVertexFunction(Some(vertex_fn.as_ref()));
    descriptor.setFragmentFunction(Some(fragment_fn.as_ref()));
    if path_sample_count > 1 {
        descriptor.setRasterSampleCount(path_sample_count as _);
        descriptor.setAlphaToCoverageEnabled(false);
    }
    let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    color_attachment.setPixelFormat(pixel_format);
    color_attachment.setBlendingEnabled(true);
    color_attachment.setRgbBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setAlphaBlendOperation(metal::MTLBlendOperation::Add);
    color_attachment.setSourceRGBBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setSourceAlphaBlendFactor(metal::MTLBlendFactor::One);
    color_attachment.setDestinationRGBBlendFactor(metal::MTLBlendFactor::OneMinusSourceAlpha);
    color_attachment.setDestinationAlphaBlendFactor(metal::MTLBlendFactor::OneMinusSourceAlpha);

    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .expect("could not create render pipeline state")
}

// Align to multiples of 256 make Metal happy.
fn align_offset(offset: &mut usize) {
    *offset = (*offset).div_ceil(256) * 256;
}

#[cfg(any(test, feature = "test-support"))]
pub struct MetalHeadlessRenderer {
    renderer: MetalRenderer,
}

#[cfg(any(test, feature = "test-support"))]
impl MetalHeadlessRenderer {
    pub fn new() -> Self {
        let instance_buffer_pool = Arc::new(Mutex::new(InstanceBufferPool::default()));
        let renderer = MetalRenderer::new_headless(instance_buffer_pool);
        Self { renderer }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl gpui::PlatformHeadlessRenderer for MetalHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.renderer.render_scene_to_image(scene, size)
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        self.renderer.render_scene(scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn gpui::PlatformAtlas> {
        self.renderer.sprite_atlas().clone()
    }
}

#[cfg(all(test, feature = "runtime_shaders"))]
mod tests {
    use super::*;

    #[test]
    fn surface_shaders_compile_at_runtime() {
        let _renderer = MetalHeadlessRenderer::new();
    }
}
