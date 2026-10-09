use super::*;
use crate::{WgpuContext, WgpuExternalRendererConfig};
use gpui::{
    BackdropBlur, Bounds, ContentMask, DevicePixels, EffectUniforms, Quad, ScaledPixels, point,
    rgba, size,
};

#[test]
#[ignore = "requires a GPU adapter"]
fn glass_on_noncopyable_targets_matches_direct_rendering() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
    ] {
        let mut renderer = WgpuRenderer::new_external(
            &context,
            WgpuExternalRendererConfig {
                size: size(DevicePixels(64), DevicePixels(64)),
                format,
                alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
                target_usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
            },
        )?;
        for height in [64, 32] {
            renderer.update_drawable_size(size(DevicePixels(64), DevicePixels(height)));
            assert!(renderer.resources().presentation.is_none());
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(64.), ScaledPixels(height as f32)),
            );
            let mut scene = Scene::default();
            scene.insert_primitive(Quad {
                bounds,
                content_mask: ContentMask { bounds },
                background: rgba(0x3864b080).into(),
                ..Default::default()
            });
            scene.insert_primitive(Quad {
                bounds: Bounds::new(
                    point(ScaledPixels(28.), ScaledPixels(0.)),
                    size(ScaledPixels(8.), ScaledPixels(height as f32)),
                ),
                content_mask: ContentMask { bounds },
                background: rgba(0xe09030ff).into(),
                ..Default::default()
            });
            scene.finish();
            let plain_pixels = render(&mut renderer, &scene, true)?;
            assert!(renderer.resources().presentation.is_none());
            let glass_bounds = Bounds::new(
                point(ScaledPixels(8.), ScaledPixels(8.)),
                size(ScaledPixels(48.), ScaledPixels(height as f32 - 16.)),
            );
            scene.push_layer(glass_bounds);
            scene.insert_primitive(BackdropBlur {
                order: 0,
                bounds: glass_bounds,
                content_mask: ContentMask { bounds },
                corner_radii: Default::default(),
                blur_radius: ScaledPixels(4.),
                opacity: 1.,
                shader: Some(gpui_effects::liquid_glass_shader()),
                uniforms: EffectUniforms::new()
                    .with_slot(0, [1., 1., 6., 6.])
                    .with_slot(1, [1., 1., 1., 0.12])
                    .with_slot(2, [0.34, 0.04, 0.015, 0.22])
                    .with_slot(3, [-0.6, -0.8, 0.8, 0.])
                    .with_slot(4, [4.; 4]),
                time: 0.,
                pointer: point(0.5, 0.5),
                pointer_active: false,
            });
            scene.pop_layer();
            scene.finish();
            let direct = render(&mut renderer, &scene, true)?;
            assert_ne!(plain_pixels, direct, "glass must change the backdrop");
            let indirect = render(&mut renderer, &scene, false)?;
            assert_eq!(
                direct, indirect,
                "color, alpha and orientation must survive presentation: {format:?}, height={height}"
            );
            assert_eq!(render(&mut renderer, &scene, false)?, indirect);
            assert!(renderer.resources().presentation.is_some());
            for _ in 0..120 {
                renderer.ensure_intermediate_textures(&Scene::default());
            }
            assert!(renderer.resources().presentation.is_none());
        }
    }
    Ok(())
}

fn render(renderer: &mut WgpuRenderer, scene: &Scene, copyable: bool) -> anyhow::Result<Vec<u8>> {
    let device = renderer.resources().device.clone();
    let queue = renderer.resources().queue.clone();
    let config = renderer.surface_config.clone();
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | if copyable {
            wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::empty()
        };
    renderer.surface_config.usage = usage;
    let (target, view) = WgpuRenderer::create_backdrop_intermediate(
        &device,
        "test_surface",
        config.format,
        config.width,
        config.height,
        usage,
    );
    renderer.ensure_backdrop_effect_pipelines(scene);
    let mut encoder = device.create_command_encoder(&Default::default());
    assert_eq!(
        renderer.encode_presented_scene(scene, &target, &view, &mut encoder)?,
        SceneEncoding::Complete
    );
    // Read a non-copyable surface through a separate GPU sampling pass.
    let mut output = PresentationTarget::new(&device, &config);
    output.bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &output.pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    output.encode(&output.view, &mut encoder);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(config.height) * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        output.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(config.height),
            },
        },
        output.texture.size(),
    );
    queue.submit([encoder.finish()]);
    renderer.commit_encoded_scene(true);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    readback.map_async(wgpu::MapMode::Read, .., move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(10)),
    })?;
    rx.recv()??;
    anyhow::ensure!(
        renderer.last_error.lock().unwrap().is_none(),
        "GPU validation: {:?}",
        renderer.last_error.lock().unwrap()
    );
    let pixels = readback.get_mapped_range(..)?.to_vec();
    readback.unmap();
    Ok(pixels)
}
