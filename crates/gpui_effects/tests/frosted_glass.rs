#![cfg(not(target_family = "wasm"))]

use gpui::{
    BackdropBlur, Bounds, ContentMask, DevicePixels, EffectUniforms, Quad, ScaledPixels as P,
    Scene, point, rgb, size,
};
use gpui_effects::{LiquidGlassStyle, liquid_glass_shader};
use gpui_wgpu::WgpuOffscreenRenderer;

#[test]
#[ignore = "requires a GPU adapter"]
fn broad_frosted_glass_blur_does_not_repeat_background_strokes() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(256), DevicePixels(256)))?;
    let viewport = Bounds::new(point(P(0.), P(0.)), size(P(256.), P(256.)));
    let stripe = Bounds::new(point(P(124.), P(0.)), size(P(8.), P(256.)));
    let mut scene = Scene::default();
    for (bounds, color) in [(viewport, 0x000000), (stripe, 0xffffff)] {
        scene.insert_primitive(Quad {
            bounds,
            content_mask: ContentMask { bounds: viewport },
            background: rgb(color).into(),
            ..Default::default()
        });
    }
    scene.insert_primitive(BackdropBlur {
        order: 0,
        bounds: viewport,
        content_mask: ContentMask { bounds: viewport },
        corner_radii: Default::default(),
        blur_radius: P(96.),
        opacity: 1.,
        shader: Some(liquid_glass_shader()),
        uniforms: LiquidGlassStyle::frosted()
            .refraction(gpui::px(0.))
            .tint(gpui::hsla(0., 0., 0., 0.))
            .uniforms(gpui::Corners::default(), 1.),
        time: 0.,
        pointer: point(0., 0.),
        pointer_active: false,
    });
    scene.finish();
    let image = renderer.render_rgba(&scene)?;
    let sample = |x: usize| image[(128 * 256 + x) * 4];
    assert!(sample(128) > 0, "the blurred stroke must remain visible");
    for x in 133..224 {
        assert!(
            sample(x) <= sample(x - 1).saturating_add(2),
            "frosted glass must have a continuous falloff, x={x}: {} -> {}",
            sample(x - 1),
            sample(x)
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn separable_gaussian_matches_impulse_kernel_after_target_resolution_changes() -> anyhow::Result<()>
{
    use gpui::BackdropShader;
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(97), DevicePixels(65)))?;
    let viewport = Bounds::new(point(P(0.), P(0.)), size(P(97.), P(65.)));
    let shader = BackdropShader::wgsl(
        "fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> { return sample_blurred_backdrop(input, vec2<f32>(0.0)); }",
    );
    let radius = 9;
    let sigma = radius as f32 / 3.;
    let weights: Vec<f32> = (-radius..=radius)
        .map(|x| (-0.5 * (x as f32 / sigma).powi(2)).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    let decode = |value: u8| {
        let value = value as f32 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    for gaussian in [true, false, true] {
        let mut scene = Scene::default();
        for (bounds, color) in [
            (viewport, 0x000000),
            (
                Bounds::new(point(P(48.), P(32.)), size(P(1.), P(1.))),
                0xffffff,
            ),
        ] {
            scene.insert_primitive(Quad {
                bounds,
                content_mask: ContentMask { bounds: viewport },
                background: rgb(color).into(),
                ..Default::default()
            });
        }
        scene.insert_primitive(BackdropBlur {
            order: 0,
            bounds: viewport,
            content_mask: ContentMask { bounds: viewport },
            corner_radii: Default::default(),
            blur_radius: P(radius as f32),
            opacity: 1.,
            shader: Some(if gaussian {
                shader.clone().with_gaussian_blur()
            } else {
                shader.clone()
            }),
            uniforms: EffectUniforms::new(),
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        });
        scene.finish();
        let pixels = renderer.render_rgba(&scene)?;
        if !gaussian {
            continue;
        }
        for y in -radius..=radius {
            for x in -radius..=radius {
                let expected =
                    weights[(x + radius) as usize] * weights[(y + radius) as usize] / total.powi(2);
                let actual = decode(pixels[(((32 + y) * 97 + 48 + x) * 4) as usize]);
                assert!(
                    (actual - expected).abs() < 0.0015,
                    "Gaussian impulse differs at ({x}, {y}): {actual} != {expected}"
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn glass_edge_color_preserves_center_and_exterior() -> anyhow::Result<()> {
    use gpui::{Corners, hsla, px};
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(128), DevicePixels(96)))?;
    let viewport = Bounds::new(point(P(0.), P(0.)), size(P(128.), P(96.)));
    let bounds = Bounds::new(point(P(16.), P(16.)), size(P(96.), P(64.)));
    for background in [0x202830, 0xe0d8d0] {
        for frost in [0., 0.55] {
            let mut images = Vec::new();
            for alpha in [0., 0.8] {
                let mut scene = Scene::default();
                scene.insert_primitive(Quad {
                    bounds: viewport,
                    content_mask: ContentMask { bounds: viewport },
                    background: rgb(background).into(),
                    ..Default::default()
                });
                scene.insert_primitive(BackdropBlur {
                    order: 0,
                    bounds,
                    content_mask: ContentMask { bounds: viewport },
                    corner_radii: Corners::all(P(12.)),
                    blur_radius: P(8.),
                    opacity: 1.,
                    shader: Some(liquid_glass_shader()),
                    uniforms: LiquidGlassStyle::optical()
                        .frost(frost)
                        .edge_color(hsla(0.3, 0.8, 0.5, alpha))
                        .uniforms(Corners::all(px(12.)), 1.),
                    time: 0.,
                    pointer: point(0., 0.),
                    pointer_active: false,
                });
                scene.finish();
                images.push(renderer.render_rgba(&scene)?);
            }
            for (x, y) in [(64, 48), (54, 42), (5, 5), (120, 88)] {
                let offset = (y * 128 + x) * 4;
                assert_eq!(
                    &images[0][offset..offset + 4],
                    &images[1][offset..offset + 4]
                );
            }
            let changed = (17..22).any(|y| {
                (40..88).any(|x| {
                    let offset = (y * 128 + x) * 4;
                    images[0][offset..offset + 3] != images[1][offset..offset + 3]
                })
            });
            assert!(changed, "edge tint must affect the curved boundary");
        }
    }
    Ok(())
}
