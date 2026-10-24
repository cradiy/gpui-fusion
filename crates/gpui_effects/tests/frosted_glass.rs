#![cfg(not(target_family = "wasm"))]

use gpui::{
    BackdropBlur, Bounds, ContentMask, DevicePixels, EffectUniforms, Quad, ScaledPixels as P,
    Scene, point, rgb, size,
};
use gpui_effects::frosted_glass_shader;
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
        shader: Some(frosted_glass_shader()),
        uniforms: EffectUniforms::new()
            .with_slot(0, [128., 128., 128., 128.])
            .with_slot(2, [0., 0., 0., 1.])
            .with_slot(4, [96., 1., 1., 0.]),
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
