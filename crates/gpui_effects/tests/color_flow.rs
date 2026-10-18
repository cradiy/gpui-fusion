#![cfg(not(target_family = "wasm"))]

use gpui_wgpu::{TextureEffectConfig, WgpuContext, WgpuTextureEffect, wgpu};

#[test]
#[ignore = "requires a GPU adapter"]
fn color_flow_dither_is_stable_unbiased_and_survives_downsampling() -> anyhow::Result<()> {
    let extent = 256;
    let context = WgpuContext::new_headless()?;
    let processor = WgpuTextureEffect::new(
        context.clone(),
        &gpui_effects::color_flow_shader(),
        TextureEffectConfig {
            output_format: wgpu::TextureFormat::Rgba8Unorm,
            ..Default::default()
        },
    )?;
    let image = context.create_texture(&wgpu::TextureDescriptor {
        label: Some("color flow input"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    context.queue.write_texture(
        image.as_image_copy(),
        &[255; 4],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        image.size(),
    );
    let render = |uniforms, time| {
        let output = processor.render(&[&image], [extent; 2], uniforms, time)?;
        read(&context, &output)
    };
    let expected = [26.25, 34.25, 42.25];
    let mut uniforms = gpui::EffectUniforms::new()
        .with_slot(0, [0.2, 1., 1., 0.])
        .with_slot(2, [0., 1., 0., 0.])
        .with_slot(
            3,
            [
                expected[0] / 255.,
                expected[1] / 255.,
                expected[2] / 255.,
                1.,
            ],
        )
        .with_slot(7, [1., 1., 1., 0.]);
    let plain = render(uniforms, 0.)?;
    assert!(
        plain
            .chunks_exact(4)
            .all(|pixel| pixel == [26, 34, 42, 255])
    );
    uniforms.set_slot(2, [0., 1., 2., 0.]);
    let pixels = render(uniforms, 0.)?;
    assert_eq!(
        render(uniforms, 11.)?,
        pixels,
        "dither must not shimmer over time"
    );
    let extent = extent as usize;
    for (channel, expected) in expected.into_iter().enumerate() {
        let mean = pixels
            .chunks_exact(4)
            .map(|p| p[channel] as f64)
            .sum::<f64>()
            / (extent * extent) as f64;
        assert!(
            (mean - expected as f64).abs() < 0.1,
            "dither must preserve the average color: {channel}, {mean}"
        );
        let mut block_variance = 0.;
        for y in (0..extent).step_by(4) {
            for x in (0..extent).step_by(4) {
                let mut sum = 0.;
                for dy in 0..4 {
                    for dx in 0..4 {
                        sum += pixels[((y + dy) * extent + x + dx) * 4 + channel] as f64;
                    }
                }
                block_variance += (sum / 16. - mean).powi(2);
            }
        }
        block_variance /= (extent * extent / 16) as f64;
        assert!(
            block_variance > 0.05,
            "downsampling must retain dither variation: {channel}, {block_variance}"
        );
    }
    uniforms.set_slot(0, [0.2, 1., 0., 0.]);
    assert!(
        render(uniforms, 0.)?
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255])
    );
    Ok(())
}

fn read(context: &WgpuContext, texture: &wgpu::Texture) -> anyhow::Result<Vec<u8>> {
    assert_eq!(texture.format(), wgpu::TextureFormat::Rgba8Unorm);
    let stride = (texture.width() * 4).div_ceil(256) * 256;
    let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("texture effect readback"),
        size: u64::from(stride) * u64::from(texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = context.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    context.queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    buffer.map_async(wgpu::MapMode::Read, .., move |result| {
        let _ = tx.send(result);
    });
    context.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(10)),
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(10))??;
    let mapped = buffer.get_mapped_range(..)?;
    let mut pixels = Vec::new();
    for row in mapped.chunks_exact(stride as usize) {
        pixels.extend_from_slice(&row[..texture.width() as usize * 4]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}
