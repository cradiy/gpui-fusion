#![cfg(not(target_family = "wasm"))]

use gpui::{
    Bounds, ContentMask, Corners, DevicePixels, EffectQuad, ScaledPixels, Scene,
    TransformationMatrix, point, rgb, size,
};
use gpui_effects::{GradientPoint, point_gradient_shader, point_gradient_uniforms};
use gpui_wgpu::WgpuOffscreenRenderer;

#[test]
#[ignore = "requires a GPU adapter"]
fn gradient_updates_positions_and_preserves_alpha_and_corners() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(200), DevicePixels(100)))?;
    let bounds = Bounds::new(
        point(ScaledPixels(0.), ScaledPixels(0.)),
        size(ScaledPixels(200.), ScaledPixels(100.)),
    );
    let mut render = |points| -> anyhow::Result<Vec<u8>> {
        let mut scene = Scene::default();
        scene.insert_primitive(EffectQuad {
            order: 0,
            bounds,
            effect_bounds: bounds,
            transformation: TransformationMatrix::unit(),
            content_mask: ContentMask { bounds },
            shader: point_gradient_shader(),
            uniforms: point_gradient_uniforms(points),
            time: 0.,
            corner_radii: Corners::all(ScaledPixels(20.)),
            opacity: 1.,
            image_tile: None,
            second_image_tile: None,
            third_image_tile: None,
            fourth_image_tile: None,
        });
        scene.finish();
        renderer.render_rgba(&scene)
    };
    let red = GradientPoint::new(point(0.25, 0.5), rgb(0xff0000)).radius(0.5);
    let blue = GradientPoint::new(point(0.75, 0.5), rgb(0x0000ff)).radius(0.5);
    let pixel = |image: &[u8], x: usize, y: usize| -> [u8; 4] {
        image[(y * 200 + x) * 4..][..4].try_into().unwrap()
    };
    let original = render([red, red, blue, blue])?;
    assert!(pixel(&original, 50, 50)[0] > 240);
    assert!(pixel(&original, 150, 50)[2] > 240);
    assert_eq!(pixel(&original, 0, 0)[..3], [0, 0, 0]);
    let moved = render([
        GradientPoint {
            position: blue.position,
            ..red
        },
        GradientPoint {
            position: blue.position,
            ..red
        },
        GradientPoint {
            position: red.position,
            ..blue
        },
        GradientPoint {
            position: red.position,
            ..blue
        },
    ])?;
    assert!(
        pixel(&moved, 50, 50)[2] > 240,
        "updates must replace previous positions"
    );

    let mut transparent_blue = blue;
    transparent_blue.position = red.position;
    transparent_blue.color.a = 0.;
    let transparent = render([red, red, transparent_blue, transparent_blue])?;
    let center = pixel(&transparent, 100, 50);
    let mut half_red = red;
    half_red.color.a = 0.5;
    let reference = render([half_red; 4])?;
    assert_eq!(center, pixel(&reference, 100, 50));
    assert!(
        center[0] > 120 && center[2] < 2,
        "transparent blue must not tint the blend"
    );
    let empty = render([GradientPoint { radius: 0., ..red }; 4])?;
    assert!(empty.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
    let tiny = render([red.radius(0.01); 4])?;
    assert!(
        pixel(&tiny, 195, 50)[0] > 240,
        "distant pixels must not underflow to black"
    );
    Ok(())
}
