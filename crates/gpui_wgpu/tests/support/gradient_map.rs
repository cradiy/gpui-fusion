use super::*;
use gpui::{EffectUniforms, Rgba, linear_color_stop, rgb};
use gpui_effects::{GradientMapPalette, gradient_map_shader};

pub(super) fn check(renderer: &mut WgpuOffscreenRenderer) -> anyhow::Result<()> {
    renderer.resize(size(DevicePixels(96), DevicePixels(48)));
    let region = bounds(0., 0., 96., 48.);
    let bands = |colors: [u32; 3]| {
        let mut scene = Scene::default();
        for (index, color) in colors.into_iter().enumerate() {
            scene.insert_primitive(quad(bounds(index as f32 * 32., 8., 32., 32.), color));
        }
        scene.finish();
        scene
    };
    let source = [0x000000ff, 0x80808080, 0xffffffff];
    let make_scene = |palette: &GradientMapPalette, strength| {
        let mut scene = Scene::default();
        scene.insert_primitive(quad(region, 0x203040ff));
        let Primitive::SubtreeLayer(mut capture) = layer(bands(source), region, 1.) else {
            unreachable!()
        };
        let mut ramp = Scene::default();
        let mut ramp_quad = quad(region, 0xffffffff);
        ramp_quad.background = palette.background();
        ramp.insert_primitive(ramp_quad);
        ramp.finish();
        capture.second_scene = Some(Rc::new(ramp));
        capture.composite.shader = gradient_map_shader();
        let color = |c: gpui::Hsla| {
            let c: Rgba = c.into();
            [c.r, c.g, c.b, c.a]
        };
        capture.composite.uniforms = EffectUniforms::new()
            .with_slot(0, [strength, 0., 0., 0.])
            .with_slot(1, color(palette.stops()[0].color))
            .with_slot(2, color(palette.stops().last().unwrap().color));
        scene.insert_primitive(Primitive::SubtreeLayer(capture));
        scene.finish();
        scene
    };
    let reference = |colors| {
        let mut scene = Scene::default();
        scene.insert_primitive(quad(region, 0x203040ff));
        scene.insert_primitive(layer(bands(colors), region, 1.));
        scene.finish();
        scene
    };
    let mut palette = GradientMapPalette::new(
        (0..25)
            .map(|index| {
                linear_color_stop(
                    rgb(match index {
                        0 => 0xff0000,
                        24 => 0x0000ff,
                        _ => 0x00ff00,
                    }),
                    index as f32 / 24.,
                )
            })
            .collect::<Vec<_>>(),
    );
    let unchanged = renderer.render_rgba(&make_scene(&palette, 0.))?;
    assert_eq!(unchanged, renderer.render_rgba(&reference(source))?);
    let actual = renderer.render_rgba(&make_scene(&palette, 1.))?;
    let expected = renderer.render_rgba(&reference([0xff0000ff, 0x00ff0080, 0x0000ffff]))?;
    // Screen sampling mixes neighboring source colors at band edges before the
    // nonlinear lookup. Compare flat interiors and fully transparent regions.
    for y in [2, 16, 24, 32, 45] {
        for x in [8, 16, 24, 40, 48, 56, 72, 80, 88] {
            let p = (y * 96 + x) * 4;
            assert!(
                actual[p..p + 4]
                    .iter()
                    .zip(&expected[p..p + 4])
                    .all(|(a, b)| a.abs_diff(*b) <= 2),
                "25-stop mapping must preserve endpoints, empty pixels and source coverage at {x},{y}"
            );
        }
    }
    palette.set_stop(12, linear_color_stop(rgb(0xffff00), 0.5));
    let edited = renderer.render_rgba(&make_scene(&palette, 1.))?;
    let p = (24 * 96 + 48) * 4;
    assert!(
        edited[p] > actual[p] + 30,
        "editing a shared stop must update the rendered ramp"
    );
    let mut transparent = palette.stops().to_vec();
    for stop in &mut transparent {
        stop.color.a = 0.;
    }
    let empty = renderer.render_rgba(&make_scene(&GradientMapPalette::new(transparent), 1.))?;
    assert_eq!(
        empty,
        renderer.render_rgba(&reference([0, 0, 0]))?,
        "transparent ramp must not create opaque pixels"
    );
    Ok(())
}
