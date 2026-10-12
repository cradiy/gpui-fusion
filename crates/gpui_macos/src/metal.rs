use objc2::{rc::Retained, runtime::ProtocolObject};
pub(crate) use objc2_foundation::NSRange;
pub(crate) use objc2_metal::*;
pub(crate) use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

macro_rules! metal_types {
    ($($owned:ident, $borrowed:ident, $protocol:ident);* $(;)?) => {$ (
        pub(crate) type $borrowed = ProtocolObject<dyn $protocol>;
        pub(crate) type $owned = Retained<$borrowed>;
    )*};
}
metal_types! {
    Device, DeviceRef, MTLDevice;
    Buffer, BufferRef, MTLBuffer;
    Texture, TextureRef, MTLTexture;
    Library, LibraryRef, MTLLibrary;
    CommandQueue, CommandQueueRef, MTLCommandQueue;
    CommandBuffer, CommandBufferRef, MTLCommandBuffer;
    RenderCommandEncoder, RenderCommandEncoderRef, MTLRenderCommandEncoder;
    RenderPipelineState, RenderPipelineStateRef, MTLRenderPipelineState;
    SamplerState, SamplerStateRef, MTLSamplerState;

}
pub(crate) type MetalDrawableRef = ProtocolObject<dyn CAMetalDrawable>;
pub(crate) type MetalLayer = Retained<CAMetalLayer>;
pub(crate) type MetalLayerRef = CAMetalLayer;
pub(crate) type TextureDescriptor = MTLTextureDescriptor;
pub(crate) type RenderPassDescriptor = MTLRenderPassDescriptor;
pub(crate) type RenderPipelineDescriptor = MTLRenderPipelineDescriptor;
pub(crate) type RenderPassColorAttachmentDescriptorRef = MTLRenderPassColorAttachmentDescriptor;
pub(crate) type SamplerDescriptor = MTLSamplerDescriptor;
pub(crate) type CompileOptions = MTLCompileOptions;

#[cfg(not(feature = "runtime_shaders"))]
pub(crate) fn library_from_data(device: &DeviceRef, bytes: &[u8]) -> Result<Library, String> {
    let data = dispatch2::DispatchData::from_bytes(bytes);
    device
        .newLibraryWithData_error(&data)
        .map_err(|error| error.to_string())
}

pub(crate) fn library_from_source(
    device: &DeviceRef,
    source: &str,
    options: &MTLCompileOptions,
) -> Result<Library, String> {
    device
        .newLibraryWithSource_options_error(
            &objc2_foundation::NSString::from_str(source),
            Some(options),
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn clear_color(red: f64, green: f64, blue: f64, alpha: f64) -> MTLClearColor {
    MTLClearColor {
        red,
        green,
        blue,
        alpha,
    }
}
pub(crate) fn region_2d(x: usize, y: usize, width: usize, height: usize) -> MTLRegion {
    MTLRegion {
        origin: MTLOrigin { x, y, z: 0 },
        size: MTLSize {
            width,
            height,
            depth: 1,
        },
    }
}
