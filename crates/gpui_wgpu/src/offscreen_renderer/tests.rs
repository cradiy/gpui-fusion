use super::*;
use gpui::{
    Bounds, ContentMask, FontId, GlyphId, MonochromeSprite, RenderGlyphParams, ScaledPixels,
    SubpixelSprite, TransformationMatrix, point, px, rgba, size,
};
use std::borrow::Cow;

#[test]
#[ignore = "requires a GPU adapter"]
fn rounded_subtree_pixels_survive_renderer_rebuilds() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let bounds = bounds(0., 0., 64., 64.);
    let mut content = Scene::default();
    content.insert_primitive(gpui::Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: rgba(0xff8033ff).into(),
        ..Default::default()
    });
    content.finish();
    let shader = gpui::EffectShader::wgsl_image(
        r#"
        fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
            let half_size = input.size * 0.5;
            let radius = params.slots[0].x;
            let q = abs((input.uv - vec2<f32>(0.5)) * input.size)
                - half_size + vec2<f32>(radius);
            let distance = length(max(q, vec2<f32>(0.0)))
                + min(max(q.x, q.y), 0.0) - radius;
            let color = sample_effect_image(input, input.uv);
            return vec4<f32>(color.rgb, color.a * (1.0 - smoothstep(-0.5, 0.5, distance)));
        }
    "#,
    );
    let mut scene = Scene::default();
    scene.insert_primitive(gpui::Primitive::SubtreeLayer(gpui::SubtreeLayer {
        scene: std::rc::Rc::new(content),
        second_scene: None,
        scene3d: None,
        intermediate_effects: Default::default(),
        composite: gpui::EffectQuad {
            order: 0,
            bounds,
            effect_bounds: bounds,
            content_mask: ContentMask { bounds },
            transformation: Default::default(),
            corner_radii: Default::default(),
            shader,
            uniforms: gpui::EffectUniforms::new().with_slot(0, [16., 0., 0., 0.]),
            time: 0.,
            opacity: 1.,
            image_tile: None,
            second_image_tile: None,
            third_image_tile: None,
            fourth_image_tile: None,
        },
    }));
    scene.finish();
    let mut expected = None;
    for _ in 0..2 {
        let mut renderer = WgpuOffscreenRenderer::with_context(
            context.clone(),
            size(DevicePixels(64), DevicePixels(64)),
        )?;
        let pixels = renderer.render_rgba(&scene)?;
        assert_eq!(&pixels[..3], [0, 0, 0], "rounded corner must be clipped");
        assert!(
            pixels[(32 * 64 + 32) * 4] > 200,
            "center must remain visible"
        );
        if let Some(expected) = &expected {
            assert_eq!(&pixels, expected);
        } else {
            expected = Some(pixels);
        }
    }
    Ok(())
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(width), ScaledPixels(height)),
    )
}

fn glyph_tile(
    renderer: &WgpuOffscreenRenderer,
    subpixel: bool,
    pixels: &[u8],
) -> anyhow::Result<gpui::AtlasTile> {
    Ok(renderer
        .sprite_atlas()
        .get_or_insert_with(
            &RenderGlyphParams {
                font_id: FontId(0),
                glyph_id: GlyphId(1),
                font_size: px(12.),
                subpixel_variant: point(0, 0),
                scale_factor: 1.,
                is_emoji: false,
                subpixel_rendering: subpixel,
                dilation: 0,
                blur_radius: 0,
            }
            .into(),
            &mut || {
                Ok(Some((
                    size(DevicePixels(3), DevicePixels(1)),
                    Cow::Borrowed(pixels),
                )))
            },
        )?
        .unwrap())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn shared_monochrome_coverage_opacity_and_transformed_clip() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = glyph_tile(&renderer, false, &[0, 128, 255])?;
    let mut scene = Scene::default();
    scene.insert_primitive(MonochromeSprite {
        order: 0,
        pad: 0,
        bounds: bounds(0., 0., 30., 12.),
        content_mask: ContentMask {
            bounds: bounds(8., 8., 54., 24.),
        },
        background: rgba(0xffffff80).into(),
        background_bounds: bounds(0., 0., 30., 12.),
        tile,
        transformation: TransformationMatrix {
            rotation_scale: [[2., 0.], [0., 2.]],
            translation: [8.5, 8.],
        },
    });
    scene.finish();
    let result = renderer.render_rgba(&scene)?;
    let pixel = |x: usize, y: usize| &result[(y * 80 + x) * 4..(y * 80 + x + 1) * 4];
    for (x, y) in [(4, 20), (18, 20), (64, 20), (38, 4), (38, 36)] {
        assert_eq!(&pixel(x, y)[..3], [0, 0, 0], "mask/clip at {x},{y}");
    }
    let full = pixel(58, 20);
    assert!(
        (187..=189).contains(&full[0]),
        "half-opacity white: {full:?}"
    );
    let partial = pixel(38, 20);
    let gamma = std::env::var("ZED_FONTS_GAMMA")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.8)
        .clamp(1., 2.2);
    let ratios = gpui::get_gamma_correction_ratios(gamma);
    // Pixel center 38.5 samples the middle texel's center.
    // White has brightness 1, so enhanced contrast has no effect.
    let coverage = 128. / 255.;
    let corrected = coverage
        + coverage * (1. - coverage) * ((ratios[0] + ratios[1]) * coverage + ratios[2] + ratios[3]);
    let expected = (255. * (1.055 * (corrected * (128. / 255.)).powf(1. / 2.4) - 0.055)).round();
    assert!(
        (partial[0] as f32 - expected).abs() <= 2.,
        "gamma-corrected coverage: {partial:?}, expected {expected}"
    );
    assert!(
        partial[0] > 0 && partial[0] < full[0],
        "partial coverage: {partial:?}"
    );
    assert_eq!(partial[0], partial[1]);
    assert_eq!(partial[1], partial[2]);
    renderer.renderer.release_idle_resources();
    assert_eq!(renderer.render_rgba(&scene)?, result);
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn scratch_targets_rebuild_after_idle_release() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(96), DevicePixels(64)))?;
    let mut builder = gpui::PathBuilder::fill();
    builder.move_to(point(px(8.), px(8.)));
    builder.line_to(point(px(88.), px(16.)));
    builder.line_to(point(px(40.), px(56.)));
    builder.close();
    let mut path = builder.build()?.scale(1.);
    path.color = rgba(0xff8033ff).into();
    path.content_mask = ContentMask {
        bounds: bounds(0., 0., 96., 64.),
    };
    let mut scene = Scene::default();
    scene.insert_primitive(path);
    scene.insert_primitive(gpui::BackdropBlur {
        order: Default::default(),
        bounds: bounds(24., 24., 48., 32.),
        content_mask: ContentMask {
            bounds: bounds(0., 0., 96., 64.),
        },
        blur_radius: ScaledPixels(4.),
        opacity: 1.,
        corner_radii: Default::default(),
        shader: None,
        uniforms: Default::default(),
        time: 0.,
        pointer: Default::default(),
        pointer_active: false,
    });
    scene.finish();
    let expected = renderer.render_rgba(&scene)?;
    assert!(expected.chunks_exact(4).any(|pixel| pixel[0] > 100));
    for _ in 0..3 {
        let before = renderer.memory_stats();
        assert!(before.path_texture_bytes > 0);
        assert!(before.backdrop_texture_bytes > 0);
        renderer.renderer.release_idle_resources();
        let after = renderer.memory_stats();
        assert_eq!(after.path_texture_bytes, 0);
        assert_eq!(after.backdrop_texture_bytes, 0);
        assert_eq!(renderer.render_rgba(&scene)?, expected);
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn monochrome_external_gradient_updates_without_replacing_glyph() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = glyph_tile(&renderer, false, &[255, 255, 255])?;
    let bounds = bounds(0., 0., 80., 48.);
    let mut background = gpui::multi_linear_gradient(
        90.,
        [
            gpui::linear_color_stop(gpui::rgb(0xff0000), 0.),
            gpui::linear_color_stop(gpui::rgb(0xff0000), 0.5),
            gpui::linear_color_stop(gpui::rgb(0xff0000), 1.),
        ],
    );
    let mut scene = Scene::default();
    for blue in [false, true] {
        if blue {
            background.set_gradient_stop(1, gpui::linear_color_stop(gpui::rgb(0x0000ff), 0.5));
        }
        scene.clear();
        scene.insert_primitive(MonochromeSprite {
            order: 0,
            pad: 0,
            bounds,
            content_mask: ContentMask { bounds },
            background: background.clone(),
            background_bounds: bounds,
            tile,
            transformation: TransformationMatrix::unit(),
        });
        scene.finish();
        let pixels = renderer.render_rgba(&scene)?;
        let pixel = &pixels[(24 * 80 + 40) * 4..][..4];
        assert!(pixel[if blue { 2 } else { 0 }] > 235, "{pixel:?}");
        assert!(pixel[if blue { 0 } else { 2 }] < 35, "{pixel:?}");
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter with dual-source blending"]
fn shared_subpixel_rgb_bgr_and_clip() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    anyhow::ensure!(
        renderer.context.supports_dual_source_blending(),
        "test requires dual-source blending"
    );
    // BGRA coverage: red, green, blue. Coverage is independent of the atlas alpha.
    let tile = glyph_tile(
        &renderer,
        true,
        &[0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255],
    )?;
    let mut scene = Scene::default();
    scene.insert_primitive(SubpixelSprite {
        order: 0,
        pad: 0,
        bounds: bounds(0., 0., 30., 12.),
        content_mask: ContentMask {
            bounds: bounds(8., 8., 54., 24.),
        },
        background: rgba(0xffffff80).into(),
        background_bounds: bounds(0., 0., 30., 12.),
        tile,
        transformation: TransformationMatrix {
            rotation_scale: [[2., 0.], [0., 2.]],
            translation: [8.5, 8.],
        },
    });
    scene.finish();
    for is_bgr in [false, true] {
        renderer.renderer.set_subpixel_layout(is_bgr);
        let result = renderer.render_rgba(&scene)?;
        let pixel = |x: usize, y: usize| &result[(y * 80 + x) * 4..(y * 80 + x + 1) * 4];
        for (x, channel) in [
            (18, if is_bgr { 2 } else { 0 }),
            (58, if is_bgr { 0 } else { 2 }),
        ] {
            let p = pixel(x, 20);
            for (index, value) in p[..3].iter().enumerate() {
                if index == channel {
                    assert!((187..=189).contains(value), "BGR={is_bgr}, {x}: {p:?}");
                } else {
                    assert_eq!(*value, 0, "BGR={is_bgr}, {x}: {p:?}");
                }
            }
        }
        assert_eq!(&pixel(64, 20)[..3], [0, 0, 0]);
        assert_eq!(&pixel(4, 20)[..3], [0, 0, 0]);
    }
    Ok(())
}
