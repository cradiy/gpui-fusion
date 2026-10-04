#![cfg(not(target_family = "wasm"))]

use gpui::{
    BorderGradient, Bounds, ColorSpace, ContentMask, Corners, DevicePixels, Edges, EffectQuad,
    Primitive, Quad, ScaledPixels, Scene, SubtreeLayer, TransformationMatrix, border_color_stop,
    border_gradient, point, px, rgb, size,
};
use gpui_effects::{
    BorderTrailMode, BorderTrailOptions, border_trail_gradient_shader, border_trail_shader,
};
use gpui_wgpu::WgpuOffscreenRenderer;
use std::{cell::Cell, rc::Rc};

#[test]
#[ignore = "requires a GPU adapter"]
fn trail_follows_edges_and_corners_without_lighting_the_content() -> anyhow::Result<()> {
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
        for radius in [0., 20., 48.] {
            let perimeter = 640. - 8. * radius + std::f32::consts::TAU * radius;
            let mode = Cell::new(BorderTrailMode::Trail);
            let mut render =
                |progress, reverse, gradient: Option<BorderGradient>| -> anyhow::Result<Vec<u8>> {
                    let mut scene = Scene::default();
                    let composite = EffectQuad {
                        order: 0,
                        bounds,
                        effect_bounds: bounds,
                        transformation: TransformationMatrix::unit(),
                        content_mask: ContentMask { bounds },
                        shader: if gradient.is_some() {
                            border_trail_gradient_shader()
                        } else {
                            border_trail_shader()
                        },
                        uniforms: BorderTrailOptions {
                            progress,
                            reverse,
                            mode: mode.get(),
                            length: px(100.),
                            width: px(3.),
                            color: rgb(0xffffff),
                            ..Default::default()
                        }
                        .uniforms(
                            size(px(224.), px(96.)),
                            Corners::all(px(radius)),
                            scale,
                        ),
                        time: 0.,
                        corner_radii: Corners::default(),
                        opacity: 1.,
                        image_tile: None,
                        second_image_tile: None,
                        third_image_tile: None,
                        fourth_image_tile: None,
                    };
                    if let Some(gradient) = gradient {
                        let mut palette = Scene::default();
                        palette.insert_primitive(Quad {
                            bounds,
                            content_mask: ContentMask { bounds },
                            corner_radii: Corners::all(ScaledPixels(radius * scale)),
                            border_widths: Edges::all(ScaledPixels(4. * scale)),
                            border_gradient: gradient,
                            ..Default::default()
                        });
                        palette.finish();
                        scene.insert_primitive(Primitive::SubtreeLayer(SubtreeLayer {
                            composite,
                            scene: Rc::new(palette),
                            intermediate_effects: Default::default(),
                            scene3d: None,
                            second_scene: None,
                        }));
                    } else {
                        scene.insert_primitive(composite);
                    }
                    scene.finish();
                    renderer.render_rgba(&scene)
                };
            let red = |pixels: &[u8], x: f32, y: f32| {
                pixels[(((y * scale) as usize) * width + (x * scale) as usize) * 4]
            };
            let top_path = 100. - radius;
            let forward = render((top_path + 10.) / perimeter, false, None)?;
            assert!(
                red(&forward, 116., 17.) > 140,
                "head must follow the top edge"
            );
            assert!(
                red(&forward, 76., 17.) < red(&forward, 116., 17.),
                "tail must fade behind the head"
            );
            assert_eq!(
                red(&forward, 140., 17.),
                0,
                "surface ahead of the head must stay clear"
            );
            assert_eq!(red(&forward, 116., 30.), 0, "content must stay clear");
            let backward = render((top_path - 10.) / perimeter, true, None)?;
            assert!(red(&backward, 116., 17.) > 140);
            assert_eq!(
                red(&backward, 76., 17.),
                0,
                "reverse tail must change sides"
            );
            if radius > 0. {
                let corner_path = 224. - 2. * radius + radius * std::f32::consts::FRAC_PI_4;
                let corner = render((corner_path + 10.) / perimeter, false, None)?;
                let diagonal = (radius - 1.) * std::f32::consts::FRAC_1_SQRT_2;
                assert!(
                    red(
                        &corner,
                        16. + 224. - radius + diagonal,
                        16. + radius - diagonal
                    ) > 100,
                    "light must travel around the rounded corner"
                );
                assert_eq!(
                    red(&corner, 239., 16.),
                    0,
                    "outside of rounded corner must stay clear"
                );
            }
            assert_eq!(
                render(0., false, None)?,
                render(1., false, None)?,
                "loop seam must match"
            );
            if radius == 20. {
                let blue = |pixels: &[u8], x: f32, y: f32| {
                    pixels[(((y * scale) as usize) * width + (x * scale) as usize) * 4 + 2]
                };
                let gradient = Some(
                    border_gradient(
                        (0..32)
                            .map(|i| {
                                border_color_stop(
                                    if i < 16 { rgb(0xff0000) } else { rgb(0x0000ff) },
                                    i as f32 / 31.,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .color_space(ColorSpace::Srgb),
                );
                for reverse in [false, true] {
                    let phase = (top_path + if reverse { -10. } else { 10. }) / perimeter;
                    let colored = render(phase, reverse, gradient.clone())?;
                    assert!(
                        red(&colored, 116., 17.) > blue(&colored, 116., 17.),
                        "head must stay red in either direction"
                    );
                    let tail_x = if reverse { 166. } else { 66. };
                    assert!(
                        blue(&colored, tail_x, 17.) > red(&colored, tail_x, 17.),
                        "tail must stay blue in either direction"
                    );
                }
                assert_eq!(
                    render(0., false, gradient.clone())?,
                    render(1., false, gradient.clone())?
                );
                mode.set(BorderTrailMode::Border);
                let bright = render((top_path + 10.) / perimeter, false, gradient.clone())?;
                let dim = render(0.8, false, gradient)?;
                assert!(red(&bright, 116., 17.) > red(&dim, 116., 17.));
                assert!(
                    red(&dim, 116., 17.) > 0,
                    "complete border must remain visible"
                );
                assert_eq!(blue(&bright, 116., 17.), 0);
                assert_eq!(
                    blue(&dim, 116., 17.),
                    0,
                    "moving the light must not move the gradient"
                );
            }
        }
    }
    Ok(())
}
