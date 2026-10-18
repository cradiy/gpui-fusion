#![cfg(not(target_family = "wasm"))]

use gpui::{
    Bounds, ContentMask, Corners, DevicePixels, EffectQuad, ScaledPixels, Scene,
    TransformationMatrix, point, px, size,
};
use gpui_effects::{LightSweepOptions, light_sweep_shader};
use gpui_wgpu::WgpuOffscreenRenderer;

#[test]
#[ignore = "requires a GPU adapter"]
fn sweep_moves_across_surface_and_preserves_clip_at_multiple_scales() -> anyhow::Result<()> {
    for scale in [1., 2.] {
        let width = (256. * scale) as usize;
        let mut renderer = WgpuOffscreenRenderer::new(size(
            DevicePixels(width as i32),
            DevicePixels((128. * scale) as i32),
        ))?;
        let bounds = Bounds::new(
            point(ScaledPixels(16. * scale), ScaledPixels(16. * scale)),
            size(ScaledPixels(224. * scale), ScaledPixels(96. * scale)),
        );
        let mut render = |options: LightSweepOptions| -> anyhow::Result<Vec<u8>> {
            let mut scene = Scene::default();
            scene.insert_primitive(EffectQuad {
                order: 0,
                bounds,
                effect_bounds: bounds,
                transformation: TransformationMatrix::unit(),
                content_mask: ContentMask { bounds },
                shader: light_sweep_shader(),
                uniforms: options.uniforms(size(px(224.), px(96.)), scale),
                time: 0.,
                corner_radii: Corners::all(ScaledPixels(20. * scale)),
                opacity: 1.,
                image_tile: None,
                second_image_tile: None,
                third_image_tile: None,
                fourth_image_tile: None,
            });
            scene.finish();
            renderer.render_rgba(&scene)
        };
        let red = |pixels: &[u8], x: usize, y: usize| {
            pixels[(((y as f32 * scale) as usize) * width + (x as f32 * scale) as usize) * 4]
        };
        let options = LightSweepOptions {
            progress: 0.5,
            angle: 0.,
            width: px(48.),
            opacity: 1.,
            ..Default::default()
        };
        let middle = render(options)?;
        assert!(red(&middle, 128, 64) > 240);
        assert_eq!(red(&middle, 90, 64), 0);
        assert_eq!(
            red(&middle, 128, 14),
            0,
            "light must respect the surface bounds"
        );
        let moved = render(LightSweepOptions {
            progress: 0.75,
            ..options
        })?;
        assert_eq!(red(&moved, 128, 64), 0, "previous position must be unlit");
        assert!(red(&moved, 196, 64) > 240);
        let vertical = render(LightSweepOptions {
            angle: 90.,
            ..options
        })?;
        assert!(
            red(&vertical, 64, 64) > 240,
            "90 degrees must produce a horizontal band"
        );
        assert_eq!(red(&vertical, 128, 30), 0);
        let corner = render(LightSweepOptions {
            width: px(1000.),
            ..options
        })?;
        assert_eq!(
            red(&corner, 17, 17),
            0,
            "rounded corner must remain clipped"
        );
        for options in [
            LightSweepOptions {
                progress: 0.,
                ..options
            },
            LightSweepOptions {
                progress: 1.,
                ..options
            },
            LightSweepOptions {
                opacity: 0.,
                ..options
            },
            LightSweepOptions {
                width: px(0.),
                ..options
            },
            LightSweepOptions {
                progress: f32::NAN,
                ..options
            },
        ] {
            assert!(
                render(options)?
                    .chunks_exact(4)
                    .all(|p| p[..3] == [0, 0, 0])
            );
        }
    }
    Ok(())
}
