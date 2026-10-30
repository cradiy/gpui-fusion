use super::*;
use crate::{WgpuContext, WgpuExternalRenderTarget, WgpuExternalRendererConfig};
use gpui::{Bounds, ContentMask, DevicePixels, EffectQuad, Quad, ScaledPixels, point, rgba, size};
use std::rc::Rc;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(width), ScaledPixels(height)),
    )
}

fn layer(source: Scene, offset: f32) -> SubtreeLayer {
    let bounds = bounds(0., 0., 64., 64.);
    SubtreeLayer {
        scene: Rc::new(source),
        second_scene: None,
        scene3d: None,
        intermediate_effects: Default::default(),
        composite: EffectQuad {
            order: 0,
            bounds,
            effect_bounds: bounds,
            transformation: Default::default(),
            content_mask: ContentMask { bounds },
            shader: gpui::EffectShader::wgsl_image(
                r#"
fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let position = vec3<f32>(input.uv * input.size, 1.0);
    let source = vec2<f32>(
        dot(params.slots[0].xyz, position),
        dot(params.slots[1].xyz, position),
    ) / input.size;
    if any(source < vec2<f32>(0.0)) || any(source >= vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    return sample_effect_image(input, source);
}
"#,
            ),
            uniforms: gpui::EffectUniforms::default()
                .with_slot(0, [1., 0., -offset, 0.])
                .with_slot(1, [0., 1., 0., 0.]),
            time: 0.,
            corner_radii: Default::default(),
            opacity: 1.,
            image_tile: None,
            second_image_tile: None,
            third_image_tile: None,
            fourth_image_tile: None,
        },
    }
}

fn scene(color: u32, offset: f32, nested: bool, siblings: bool) -> Scene {
    let mut source = Scene::default();
    let rect = bounds(8., 8., 16., 16.);
    source.insert_primitive(Quad {
        bounds: rect,
        content_mask: ContentMask { bounds: rect },
        background: rgba(color).into(),
        ..Default::default()
    });
    source.finish();
    if nested {
        let mut parent = Scene::default();
        parent.insert_primitive(gpui::Primitive::SubtreeLayer(layer(source, 0.)));
        parent.finish();
        source = parent;
    }
    let layer = layer(source, offset);
    let mut result = Scene::default();
    result.insert_primitive(gpui::Primitive::SubtreeLayer(layer.clone()));
    if siblings {
        result.insert_primitive(gpui::Primitive::SubtreeLayer(layer));
    }
    result.finish();
    result
}

fn texture(context: &WgpuContext, width: u32) -> wgpu::Texture {
    context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("subtree cache regression"),
        size: wgpu::Extent3d {
            width,
            height: width,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn renderer(context: &WgpuContext) -> anyhow::Result<WgpuRenderer> {
    WgpuRenderer::new_external(
        context,
        WgpuExternalRendererConfig {
            size: size(DevicePixels(64), DevicePixels(64)),
            format: wgpu::TextureFormat::Rgba8Unorm,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            target_usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        },
    )
}

fn draw(renderer: &mut WgpuRenderer, scene: &Scene, target: &wgpu::Texture) -> usize {
    let view = target.create_view(&Default::default());
    for _ in 0..10 {
        if renderer.draw_external(scene, target, &view, wgpu::Color::TRANSPARENT) {
            return renderer.resources().subtree_cache.hits.get();
        }
    }
    panic!("scene did not fit after retries");
}

fn pixels(context: &WgpuContext, texture: &wgpu::Texture) -> Vec<u8> {
    let stride = (texture.width() * 4).div_ceil(256) * 256;
    let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride * texture.height()),
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
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    context.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    context
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    let result = data.to_vec();
    drop(data);
    buffer.unmap();
    result
}

#[test]
#[ignore = "requires a GPU adapter"]
fn subtree_sampling_preserves_premultiplied_interpolation_and_fractional_capture_edges()
-> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    let mut source = Scene::default();
    for (x, color) in [(0., 0xff000080), (16., 0x0000ffff)] {
        source.insert_primitive(Quad {
            bounds: bounds(x, 0., if x == 0. { 16. } else { 48. }, 64.),
            content_mask: ContentMask {
                bounds: bounds(0., 0., 64., 64.),
            },
            background: rgba(color).into(),
            ..Default::default()
        });
    }
    source.finish();
    let source = Rc::new(source);
    for region in [bounds(0., 0., 64., 64.), bounds(7.25, 7.25, 17.5, 17.5)] {
        for second in [false, true] {
            for q in [
                [-0.25, 16.],
                [0., 16.],
                [0.5, 16.],
                [7.25, 7.25],
                [8.125, 8.625],
                [15.75, 16.],
                [16.125, 16.75],
                [24.75, 16.],
                [63.75, 16.],
                [65., 16.],
            ] {
                let mut sampled = layer(Scene::default(), 0.);
                sampled.scene = source.clone();
                sampled.composite.bounds = region;
                sampled.composite.effect_bounds = region;
                sampled.composite.content_mask.bounds = region;
                sampled.composite.uniforms = gpui::EffectUniforms::default().with_slot(
                    0,
                    [
                        (q[0] - region.origin.x.0) / region.size.width.0,
                        (q[1] - region.origin.y.0) / region.size.height.0,
                        0.,
                        0.,
                    ],
                );
                sampled.composite.shader = if second {
                    sampled.second_scene = Some(source.clone());
                    gpui::EffectShader::wgsl_two_images(
                        "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { return sample_effect_second_image(input, params.slots[0].xy); }",
                    )
                } else {
                    gpui::EffectShader::wgsl_image(
                        "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { return sample_effect_image(input, params.slots[0].xy); }",
                    )
                };
                let mut scene = Scene::default();
                scene.insert_primitive(gpui::Primitive::SubtreeLayer(sampled));
                scene.finish();
                draw(&mut renderer, &scene, &target);
                let output = pixels(&context, &target);
                let pixel = q.map(|v| v - 0.5);
                let low = pixel.map(|v| v.floor() as i32);
                let fraction = pixel.map(|v| v - v.floor());
                let mut expected = [0.; 4];
                for y in 0..2 {
                    for x in 0..2 {
                        let p = [low[0] + x, low[1] + y];
                        if p[0] < 0
                            || p[1] < 0
                            || p[0] >= 64
                            || p[1] >= 64
                            || p[0] as f32 + 0.5 < region.left().0
                            || p[1] as f32 + 0.5 < region.top().0
                            || p[0] as f32 + 0.5 >= region.right().0
                            || p[1] as f32 + 0.5 >= region.bottom().0
                        {
                            continue;
                        }
                        let color = if p[0] < 16 {
                            [128., 0., 0., 128.]
                        } else {
                            [0., 0., 255., 255.]
                        };
                        let weight = if x == 0 {
                            1. - fraction[0]
                        } else {
                            fraction[0]
                        } * if y == 0 {
                            1. - fraction[1]
                        } else {
                            fraction[1]
                        };
                        for c in 0..4 {
                            expected[c] += color[c] * weight;
                        }
                    }
                }
                for c in 0..4 {
                    assert!(
                        (f32::from(output[(16 * 64 + 16) * 4 + c]) - expected[c]).abs() <= 2.,
                        "region={region:?}, second={second}, q={q:?}, channel={c}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a Vulkan adapter with allocation reports"]
#[cfg(target_os = "linux")]
fn idle_release_retires_completed_path_allocations_and_preserves_the_presented_pixels()
-> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    let mut builder = gpui::PathBuilder::fill();
    builder.move_to(point(gpui::px(8.), gpui::px(8.)));
    builder.line_to(point(gpui::px(56.), gpui::px(8.)));
    builder.line_to(point(gpui::px(32.), gpui::px(56.)));
    builder.close();
    let mut path = builder.build()?.scale(1.);
    path.color = gpui::rgb(0x6c91b3).into();
    path.content_mask = ContentMask {
        bounds: bounds(0., 0., 64., 64.),
    };
    let mut scene = Scene::default();
    scene.insert_primitive(path);
    scene.finish();
    draw(&mut renderer, &scene, &target);
    // This device has no concurrent submitters. Wait at the Vulkan level so
    // WGPU's deferred resource cleanup remains unpolled.
    let device = unsafe { context.device.as_hal::<wgpu_hal::api::Vulkan>() }
        .ok_or_else(|| anyhow::anyhow!("GPU backend is not Vulkan"))?;
    unsafe { device.raw_device().device_wait_idle()? };
    drop(device);
    let report = || {
        context
            .device
            .generate_allocator_report()
            .ok_or_else(|| anyhow::anyhow!("GPU backend does not report allocations"))
    };
    assert!(
        report()?
            .allocations
            .iter()
            .any(|allocation| allocation.name == "path_msaa")
    );
    renderer.release_idle_resources();
    assert!(
        !report()?.allocations.iter().any(|allocation| matches!(
            allocation.name.as_str(),
            "path_msaa" | "path_intermediate"
        )),
        "idle cleanup must retire completed resources without another frame or device poll"
    );
    let expected = pixels(&context, &target);
    draw(&mut renderer, &scene, &target);
    assert_eq!(pixels(&context, &target), expected);
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn ancestor_capture_reuses_nested_effect_pixels_and_invalidates_changed_content()
-> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    let make_scene = |color, wrapped: bool| {
        let quad = |color| scene(color, 0., false, false).subtree_layers.pop().unwrap();
        let mut mask = quad(color);
        mask.second_scene = Some(quad(0xffffff99).scene);
        mask.composite.shader = gpui::EffectShader::wgsl_two_images(
            "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { let color = sample_effect_image(input, input.uv); return vec4<f32>(color.rgb, color.a * sample_effect_second_image(input, input.uv).a); }",
        );
        let mut pair = Scene::default();
        pair.insert_primitive(gpui::Primitive::SubtreeLayer(mask));
        pair.finish();
        let mut processed = layer(pair, 0.);
        processed.intermediate_effects = vec![gpui::SubtreeEffectPass {
            shader: processed.composite.shader.clone(),
            uniforms: processed.composite.uniforms,
            time: 0.,
            images: Default::default(),
            bloom: None,
            feedback: None,
            distance_field: None,
            particles: None,
            particle_transition: None,
        }]
        .into();
        let mut source = Scene::default();
        source.insert_primitive(gpui::Primitive::SubtreeLayer(processed));
        source.finish();
        if !wrapped {
            return source;
        }
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::SubtreeLayer(layer(source, 0.)));
        scene.finish();
        scene
    };
    draw(&mut renderer, &make_scene(0x2f9d7380, false), &target);
    let reference = pixels(&context, &target);
    assert_eq!(
        draw(&mut renderer, &make_scene(0x2f9d7380, true), &target),
        0
    );
    let source_texture = renderer.resources().subtree_textures[0].clone();
    let output = pixels(&context, &target);
    assert!(output.iter().any(|value| *value != 0));
    assert!(
        output
            .iter()
            .zip(reference)
            .all(|(a, b)| a.abs_diff(b) <= 2)
    );
    assert!(draw(&mut renderer, &make_scene(0x2f9d7380, true), &target) > 0);
    assert!(renderer.resources().ui_captures.is_empty());
    assert_eq!(renderer.resources().subtree_textures[0], source_texture);
    assert_eq!(pixels(&context, &target), output);
    assert_eq!(
        draw(&mut renderer, &make_scene(0xb34d2680, true), &target),
        0
    );
    assert_ne!(pixels(&context, &target), output);
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn raster_capture_preserves_offset_alpha_and_nested_density() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    for (density, nested) in [
        (2., false),
        (2., false),
        (3., false),
        (2., true),
        (1., false),
        (0.5, false),
        (0.25, false),
    ] {
        let total_density = density * if nested { 2. } else { 1. };
        let mut source = Scene::default();
        source.raster_scale = Some(if nested { 2. } else { density });
        let rect = bounds(12., 14., 8., 6.).map(|p| ScaledPixels(p.0 * total_density));
        source.insert_primitive(Quad {
            bounds: rect,
            content_mask: ContentMask { bounds: rect },
            background: rgba(0xff000080).into(),
            ..Default::default()
        });
        source.finish();
        if nested {
            let mut inner = layer(source, 0.);
            inner.composite.bounds = bounds(8., 8., 48., 48.).map(|p| ScaledPixels(p.0 * density));
            inner.composite.effect_bounds = inner.composite.bounds;
            inner.composite.content_mask.bounds = inner.composite.bounds;
            source = Scene::default();
            source.raster_scale = Some(density);
            source.insert_primitive(gpui::Primitive::SubtreeLayer(inner));
            source.finish();
        }
        let mut outer = layer(source, 0.);
        outer.composite.bounds = bounds(8., 8., 48., 48.);
        outer.composite.effect_bounds = outer.composite.bounds;
        outer.composite.content_mask.bounds = outer.composite.bounds;
        outer.composite.uniforms = gpui::EffectUniforms::default()
            .with_slot(0, [0.5, 0., 0., 0.])
            .with_slot(1, [0., 0.5, 0., 0.]);
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::SubtreeLayer(outer));
        scene.finish();
        draw(&mut renderer, &scene, &target);
        let output = pixels(&context, &target);
        let sample = |x: usize, y: usize| &output[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(
            sample(22, 25),
            &[128, 0, 0, 128],
            "density={density}, nested={nested}"
        );
        for (x, y) in [(10, 10), (38, 25), (22, 38)] {
            assert_eq!(sample(x, y), &[0; 4]);
        }
        {
            let capture = &renderer.resources().ui_captures[0];
            assert_eq!(capture.texture.width(), (48. * density) as u32);
            assert_eq!(capture.renderer.capture_origin, [8. * density; 2]);
            assert!(renderer.resources().subtree_textures.is_empty());
            if nested {
                assert_eq!(
                    capture.renderer.resources().ui_captures[0].texture.width(),
                    192
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn regional_capture_matches_full_frame_pixels_after_moving_crop() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut full_renderer = renderer(&context)?;
    full_renderer.update_drawable_size(size(DevicePixels(128), DevicePixels(128)));
    let full_target = texture(&context, 128);
    let mut source = Scene::default();
    source.raster_scale = Some(2.);
    let rect = bounds(32., 36., 28., 24.);
    let clip = ContentMask {
        bounds: bounds(0., 0., 128., 128.),
    };
    source.insert_primitive(gpui::Shadow {
        order: 0,
        bounds: rect,
        content_mask: clip,
        color: rgba(0x00000080).into(),
        blur_radius: ScaledPixels(3.),
        corner_radii: gpui::Corners::all(ScaledPixels(5.)),
        element_bounds: rect,
        element_corner_radii: gpui::Corners::all(ScaledPixels(5.)),
        inset: 0,
        pad: 0,
    });
    source.insert_primitive(Quad {
        bounds: rect,
        content_mask: clip,
        background: rgba(0xff000080).into(),
        corner_radii: gpui::Corners::all(ScaledPixels(5.)),
        ..Default::default()
    });
    source.insert_primitive(gpui::Underline {
        order: 0,
        pad: 0,
        bounds: bounds(40., 66., 36., 6.),
        content_mask: clip,
        color: rgba(0x00ff00ff).into(),
        thickness: ScaledPixels(2.),
        wavy: true.into(),
    });
    let mut path = gpui::PathBuilder::fill();
    path.move_to(point(gpui::px(25.), gpui::px(40.)));
    path.line_to(point(gpui::px(35.), gpui::px(55.)));
    path.line_to(point(gpui::px(15.), gpui::px(55.)));
    path.close();
    let mut path = path.build()?.scale(2.);
    path.color = rgba(0x2244ffff).into();
    path.content_mask = clip;
    source.insert_primitive(path);
    let mut effect = layer(Scene::default(), 0.).composite;
    effect.bounds = bounds(80., 24., 20., 20.);
    effect.effect_bounds = effect.bounds;
    effect.content_mask = clip;
    effect.shader = gpui::EffectShader::wgsl(
        "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { return vec4<f32>(input.position / 128.0, 0.0, 1.0); }",
    );
    source.insert_primitive(effect);
    source.finish();
    draw(&mut full_renderer, &source, &full_target);
    let reference = pixels(&context, &full_target);
    let source = Rc::new(source);
    let mut cropped_renderer = renderer(&context)?;
    let target = texture(&context, 64);
    for crop in [
        bounds(8., 8., 48., 48.),
        bounds(10., 6., 48., 48.),
        bounds(10.25, 6.75, 40., 45.),
        bounds(-4., -3., 48., 48.),
    ] {
        let mut outer = layer(Scene::default(), 0.);
        outer.scene = source.clone();
        outer.composite.bounds = crop;
        outer.composite.effect_bounds = crop;
        outer.composite.content_mask.bounds = crop;
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::SubtreeLayer(outer));
        scene.finish();
        draw(&mut cropped_renderer, &scene, &target);
        let capture = &cropped_renderer.resources().ui_captures[0];
        let actual = pixels(&context, &capture.texture);
        let [left, top] = capture.renderer.capture_origin.map(|value| value as usize);
        let width = capture.texture.width() as usize;
        let stride = (width * 4).div_ceil(256) * 256;
        for row in 0..capture.texture.height() as usize {
            let expected =
                &reference[(row + top) * 512 + left * 4..(row + top) * 512 + (left + width) * 4];
            let actual = &actual[row * stride..row * stride + width * 4];
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "row {row}, crop {crop:?}"
            );
        }
        assert!(cropped_renderer.resources().subtree_textures.is_empty());
        assert!(
            cropped_renderer
                .resources()
                .path_intermediate_texture
                .is_none()
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn nested_capture_can_translate_source_from_outside_parent_crop() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    for (density, inner_density) in [(1., Some(2.)), (2., Some(2.)), (1., None), (2., None)] {
        let mut source = Scene::default();
        source.raster_scale = inner_density;
        let rect = bounds(44., 16., 4., 4.)
            .map(|p| ScaledPixels(p.0 * density * inner_density.unwrap_or(1.)));
        source.insert_primitive(Quad {
            bounds: rect,
            content_mask: ContentMask { bounds: rect },
            background: rgba(0xff0000ff).into(),
            ..Default::default()
        });
        source.finish();
        let mut inner = layer(source, -30. * density);
        inner.composite.bounds = bounds(0., 0., 64., 64.).map(|p| ScaledPixels(p.0 * density));
        inner.composite.effect_bounds = inner.composite.bounds;
        inner.composite.content_mask.bounds = inner.composite.bounds;
        let mut parent = Scene::default();
        parent.raster_scale = Some(density);
        parent.insert_primitive(gpui::Primitive::SubtreeLayer(inner));
        parent.finish();
        let mut outer = layer(parent, 0.);
        outer.composite.bounds = bounds(8., 8., 24., 24.);
        outer.composite.effect_bounds = outer.composite.bounds;
        outer.composite.content_mask.bounds = outer.composite.bounds;
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::SubtreeLayer(outer));
        scene.finish();
        draw(&mut renderer, &scene, &target);
        let output = pixels(&context, &target);
        assert_eq!(
            &output[(17 * 64 + 15) * 4..(17 * 64 + 15) * 4 + 4],
            &[255, 0, 0, 255],
            "density={density}, inner={inner_density:?}"
        );
        if inner_density.is_some() {
            let capture = &renderer.resources().ui_captures[0];
            assert_eq!(capture.texture.width(), (24. * density) as u32);
            assert_eq!(
                capture.renderer.resources().ui_captures[0].texture.width(),
                (128. * density) as u32
            );
        } else if density > 1. {
            assert_eq!(renderer.resources().ui_captures[0].texture.width(), 128);
        } else {
            assert!(renderer.resources().ui_captures.is_empty());
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn capture_diagnostics_track_reuse_nested_storage_and_release() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    let make_scene = |color, offset| {
        let mut source = Scene::default();
        source.raster_scale = Some(2.);
        let rect = bounds(8., 8., 16., 16.);
        source.insert_primitive(Quad {
            bounds: rect,
            content_mask: ContentMask { bounds: rect },
            background: rgba(color).into(),
            ..Default::default()
        });
        source.finish();
        let mut outer = Scene::default();
        outer.raster_scale = Some(2.);
        outer.insert_primitive(gpui::Primitive::SubtreeLayer(layer(source, 0.)));
        outer.finish();
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::SubtreeLayer(layer(outer, offset)));
        scene.finish();
        scene
    };
    draw(&mut renderer, &make_scene(0xff0000ff, 0.), &target);
    let first = renderer.diagnostics().unwrap();
    assert_eq!(first.capture_cache.hits, 0);
    assert_eq!(first.capture_cache.misses, 2);
    assert_eq!(first.capture_textures.len(), 2);
    let first_bytes: u64 = first
        .capture_textures
        .iter()
        .map(|t| t.estimated_bytes)
        .sum();
    assert!(first_bytes > 0);
    for texture in &first.capture_textures {
        assert_eq!(
            texture.estimated_bytes,
            u64::from(texture.width) * u64::from(texture.height) * 4
        );
        assert_eq!(texture.raster_scale, Some(2.));
    }
    draw(&mut renderer, &make_scene(0xff0000ff, 8.), &target);
    let reused = renderer.diagnostics().unwrap();
    assert_eq!(
        reused.capture_cache.hits, 1,
        "a reused parent does not visit the child"
    );
    assert_eq!(reused.capture_cache.misses, 0);
    assert_eq!(
        reused
            .capture_textures
            .iter()
            .map(|t| t.estimated_bytes)
            .sum::<u64>(),
        first_bytes
    );
    draw(&mut renderer, &make_scene(0x0000ffff, 8.), &target);
    assert_eq!(renderer.diagnostics().unwrap().capture_cache.misses, 2);
    draw(&mut renderer, &Scene::default(), &target);
    let empty = renderer.diagnostics().unwrap();
    assert!(empty.capture_textures.is_empty());
    assert_eq!(empty.capture_cache.hits + empty.capture_cache.misses, 0);
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn subtree_capture_cache_reuses_only_submitted_unchanged_single_writer_textures()
-> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let mut renderer = renderer(&context)?;
    let target = texture(&context, 64);
    assert_eq!(
        draw(&mut renderer, &scene(0xff0000ff, 0., false, false), &target),
        0
    );
    let original_texture = renderer.resources().subtree_textures[0].clone();
    // Fresh scene allocations and changed composite uniforms must still reuse the source.
    assert_eq!(
        draw(
            &mut renderer,
            &scene(0xff0000ff, 16., false, false),
            &target
        ),
        1
    );
    assert_eq!(renderer.resources().subtree_textures[0], original_texture);
    let image = pixels(&context, &target);
    assert_eq!(
        &image[(16 * 64 + 32) * 4..(16 * 64 + 32) * 4 + 4],
        &[255, 0, 0, 255]
    );
    assert_eq!(
        &image[(16 * 64 + 12) * 4..(16 * 64 + 12) * 4 + 4],
        &[0, 0, 0, 0]
    );
    assert_eq!(
        draw(
            &mut renderer,
            &scene(0x0000ffff, 16., false, false),
            &target
        ),
        0
    );
    let image = pixels(&context, &target);
    assert_eq!(
        &image[(16 * 64 + 32) * 4..(16 * 64 + 32) * 4 + 4],
        &[0, 0, 255, 255]
    );

    // An encoded but discarded update cannot become a cache hit on retry.
    let green = scene(0x00ff00ff, 0., false, false);
    let view = target.create_view(&Default::default());
    let mut discarded = context.device.create_command_encoder(&Default::default());
    assert!(matches!(
        renderer.encode_external_scene(
            &green,
            WgpuExternalRenderTarget {
                texture: &target,
                view: &view,
                command_encoder: &mut discarded,
            },
            true
        )?,
        super::super::SceneEncoding::Complete
    ));
    renderer.commit_encoded_scene(false);
    drop(discarded);
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    assert_eq!(draw(&mut renderer, &green, &target), 1);

    // Nested source textures can be reused independently; sibling scratch reuse cannot.
    assert_eq!(
        draw(&mut renderer, &scene(0xff0000ff, 0., true, false), &target),
        0
    );
    assert_eq!(
        draw(&mut renderer, &scene(0xff0000ff, 8., true, false), &target),
        1
    );
    assert_eq!(
        draw(&mut renderer, &scene(0xff0000ff, 8., true, true), &target),
        0
    );
    assert_eq!(
        draw(&mut renderer, &scene(0xff0000ff, 8., true, true), &target),
        0
    );
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    assert_eq!(draw(&mut renderer, &green, &target), 1);

    // Intermediate passes overwrite the same scratch targets as source captures.
    let mut processed = scene(0xff0000ff, 0., false, false);
    let composite = &processed.subtree_layers[0].composite;
    processed.subtree_layers[0].intermediate_effects = vec![gpui::SubtreeEffectPass {
        shader: composite.shader.clone(),
        uniforms: composite.uniforms,
        time: 0.,
        images: Default::default(),
        bloom: None,
        feedback: None,
        distance_field: None,
        particles: None,
        particle_transition: None,
    }]
    .into();
    assert_eq!(draw(&mut renderer, &processed, &target), 0);
    assert_eq!(draw(&mut renderer, &processed, &target), 0);
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    assert_eq!(draw(&mut renderer, &green, &target), 1);
    let image = pixels(&context, &target);
    assert_eq!(
        &image[(16 * 64 + 16) * 4..(16 * 64 + 16) * 4 + 4],
        &[0, 255, 0, 255]
    );

    renderer.is_bgr = !renderer.is_bgr;
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    // Caller-owned commands can overwrite scratch textures after a later submitted frame.
    let mut external = context.device.create_command_encoder(&Default::default());
    assert!(renderer.encode_external(
        &scene(0xff0000ff, 0., false, false),
        WgpuExternalRenderTarget {
            texture: &target,
            view: &view,
            command_encoder: &mut external,
        }
    ));
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    context.queue.submit([external.finish()]);
    assert_eq!(draw(&mut renderer, &green, &target), 0);
    let image = pixels(&context, &target);
    assert_eq!(
        &image[(16 * 64 + 16) * 4..(16 * 64 + 16) * 4 + 4],
        &[0, 255, 0, 255]
    );

    renderer.update_drawable_size(size(DevicePixels(80), DevicePixels(80)));
    let resized = texture(&context, 80);
    assert_eq!(draw(&mut renderer, &green, &resized), 0);
    assert_eq!(draw(&mut renderer, &green, &resized), 1);
    assert_ne!(renderer.resources().subtree_textures[0], original_texture);
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter; reports timings without asserting a speed threshold"]
fn subtree_capture_cache_100_quad_transform_benchmark() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    eprintln!("Adapter: {:?}", context.adapter.get_info());
    let mut renderer = renderer(&context)?;
    renderer.update_drawable_size(size(DevicePixels(1024), DevicePixels(1024)));
    let target = texture(&context, 1024);
    let mut source = Scene::default();
    for i in 0..100 {
        let rect = bounds((i % 10) as f32 * 100., (i / 10) as f32 * 100., 90., 90.);
        source.insert_primitive(Quad {
            bounds: rect,
            content_mask: ContentMask { bounds: rect },
            background: rgba(0xff0000ff).into(),
            ..Default::default()
        });
    }
    source.finish();
    let sources: Vec<_> = (0..120)
        .map(|i| {
            let mut scene = Scene::default();
            let mut copy = Scene::default();
            copy.replay(0..source.len(), &source);
            copy.finish();
            let mut layer = layer(copy, (i % 10) as f32);
            let viewport = bounds(0., 0., 1024., 1024.);
            layer.composite.bounds = viewport;
            layer.composite.effect_bounds = viewport;
            layer.composite.content_mask.bounds = viewport;
            let scale = 1. + (i % 20) as f32 * 0.025;
            layer
                .composite
                .uniforms
                .set_slot(0, [1. / scale, 0., -(i % 10) as f32 / scale, 0.]);
            layer
                .composite
                .uniforms
                .set_slot(1, [0., 1. / scale, 0., 0.]);
            scene.insert_primitive(gpui::Primitive::SubtreeLayer(layer));
            scene.finish();
            scene
        })
        .collect();
    for reuse in [false, true] {
        renderer.resources_mut().subtree_cache = SubtreeCaptureCache::default();
        renderer.resources_mut().subtree_cache.external_writes = !reuse;
        let mut samples = Vec::new();
        let mut hits = 0;
        for (i, scene) in sources.iter().enumerate() {
            let start = std::time::Instant::now();
            let reused = draw(&mut renderer, scene, &target);
            context.device.poll(wgpu::PollType::wait_indefinitely())?;
            if i >= 20 {
                samples.push(start.elapsed().as_secs_f64() * 1000.);
                hits += reused;
            }
        }
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "1024x1024 / 100 quads / 100 measured frames / reuse={reuse}: median={:.3}ms p95={:.3}ms source_hits={hits}",
            samples[50], samples[95]
        );
        assert_eq!(hits, if reuse { 100 } else { 0 });
    }
    Ok(())
}
