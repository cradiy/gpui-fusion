#![cfg(not(target_family = "wasm"))]

use gpui::{
    Bounds, ContentMask, Corners, DevicePixels, EffectQuad, ScaledPixels, Scene,
    TransformationMatrix, point, px, size,
};
use gpui_effects::{SpotlightOptions, spotlight_shader};
use gpui_wgpu::WgpuOffscreenRenderer;

#[test]
#[ignore = "requires a GPU adapter"]
fn spotlight_tracks_local_light_and_preserves_clip_at_multiple_scales() -> anyhow::Result<()> {
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
        let corners = Corners {
            top_left: px(20.),
            top_right: px(12.),
            bottom_right: px(0.),
            bottom_left: px(32.),
        };
        let mut render = |options: SpotlightOptions| -> anyhow::Result<Vec<u8>> {
            let mut scene = Scene::default();
            scene.insert_primitive(EffectQuad {
                order: 0,
                bounds,
                effect_bounds: bounds,
                transformation: TransformationMatrix::unit(),
                content_mask: ContentMask { bounds },
                shader: spotlight_shader(),
                uniforms: options.uniforms(size(px(224.), px(96.)), corners, scale),
                time: 0.,
                corner_radii: corners.map(|r| ScaledPixels(f32::from(r) * scale)),
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
        let options = SpotlightOptions {
            center: point(0.5, 0.),
            color: gpui::white().into(),
            radius: px(80.),
            ..Default::default()
        };
        let lit = render(options)?;
        assert!(
            red(&lit, 128, 16) > red(&lit, 128, 30) + 60,
            "edge must be brighter than surface"
        );
        assert!(
            red(&lit, 128, 30) > red(&lit, 128, 75) + 15,
            "light must fall off locally"
        );
        assert_eq!(red(&lit, 30, 30), 0, "distant surface must stay unlit");
        assert_eq!(red(&lit, 128, 14), 0, "light must stay within the surface");

        let moved = render(SpotlightOptions {
            center: point(0.85, 1.),
            ..options
        })?;
        assert_eq!(red(&moved, 128, 16), 0, "previous edge must stop glowing");
        assert!(
            red(&moved, 206, 111) > 150,
            "new edge must follow the light"
        );
        let no_edge = render(SpotlightOptions {
            edge_width: px(0.),
            ..options
        })?;
        assert!(red(&lit, 128, 16) > red(&no_edge, 128, 16) + 60);
        let corner = render(SpotlightOptions {
            center: point(0., 0.),
            ..options
        })?;
        assert_eq!(
            red(&corner, 17, 17),
            0,
            "rounded corner must remain clipped"
        );
        assert!(red(&corner, 37, 17) > 100);
        let hidden = render(SpotlightOptions {
            strength: 0.,
            ..options
        })?;
        assert!(hidden.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
    }
    Ok(())
}
