#![cfg(not(target_family = "wasm"))]

use gpui::{
    BackdropBlur, Bounds, ContentMask, DevicePixels, Quad, ScaledPixels as P, Scene, point, rgb,
    size,
};
use gpui_wgpu::WgpuOffscreenRenderer;

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<P> {
    Bounds::new(point(P(x), P(y)), size(P(w), P(h)))
}

#[test]
#[ignore = "requires a GPU adapter"]
fn large_backdrop_blurs_have_a_continuous_falloff_without_repeated_strokes() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(256), DevicePixels(256)))?;
    let viewport = bounds(0., 0., 256., 256.);
    for radius in [8., 24., 48., 64.] {
        for horizontal in [false, true] {
            let mut scene = Scene::default();
            scene.insert_primitive(Quad {
                bounds: if horizontal {
                    bounds(0., 128., 256., 2.)
                } else {
                    bounds(128., 0., 2., 256.)
                },
                content_mask: ContentMask { bounds: viewport },
                background: rgb(0xffffff).into(),
                ..Default::default()
            });
            scene.insert_primitive(BackdropBlur {
                order: 0,
                bounds: viewport,
                content_mask: ContentMask { bounds: viewport },
                corner_radii: Default::default(),
                blur_radius: P(radius),
                opacity: 1.,
                shader: None,
                uniforms: Default::default(),
                time: 0.,
                pointer: point(0., 0.),
                pointer_active: false,
            });
            scene.finish();
            let image = renderer.render_rgba(&scene)?;
            let sample = |offset: usize| {
                let (x, y) = if horizontal {
                    (128, offset)
                } else {
                    (offset, 128)
                };
                image[(y * 256 + x) * 4]
            };
            assert!(
                sample(129) > 20,
                "the stroke must remain visible after spreading"
            );
            for offset in 131..224 {
                assert!(
                    sample(offset) <= sample(offset - 1).saturating_add(2),
                    "blur must fall off continuously, radius={radius}, horizontal={horizontal}, offset={offset}: {} -> {}",
                    sample(offset - 1),
                    sample(offset)
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn large_backdrop_radius_continues_to_spread_beyond_sixty_four_device_pixels() -> anyhow::Result<()>
{
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(256), DevicePixels(256)))?;
    let viewport = bounds(0., 0., 256., 256.);
    let mut spreads = Vec::new();
    for radius in [64., 128.] {
        let mut scene = Scene::default();
        for (bounds, color) in [(viewport, 0x000000), (bounds(124., 0., 8., 256.), 0xffffff)] {
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
            blur_radius: P(radius),
            opacity: 1.,
            shader: None,
            uniforms: Default::default(),
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        });
        scene.finish();
        let image = renderer.render_rgba(&scene)?;
        let mut weight = 0.;
        let mut moment = 0.;
        for x in 0..256 {
            let value = image[(128 * 256 + x) * 4] as f64;
            weight += value;
            moment += value * (x as f64 - 127.5).powi(2);
        }
        anyhow::ensure!(weight > 0., "the blurred stripe must remain visible");
        spreads.push(moment / weight);
    }
    assert!(
        spreads[1] > spreads[0] * 2.,
        "large blur radii must broaden the image instead of saturating at 64 pixels: {spreads:?}"
    );
    Ok(())
}
