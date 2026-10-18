use super::*;

pub(super) fn check(renderer: &mut WgpuOffscreenRenderer) -> anyhow::Result<()> {
    renderer.resize(size(DevicePixels(20), DevicePixels(12)));
    let region = bounds(0., 0., 20., 12.);
    let scene = |strength: f32, identity: bool| {
        let mut source = Scene::default();
        source.insert_primitive(quad(bounds(2., 2., 4., 4.), 0xff000080));
        source.insert_primitive(quad(bounds(16., 0., 3., 8.), 0x00ff00ff));
        source.insert_primitive(quad(bounds(19., 0., 1., 8.), 0x0000ffff));
        source.finish();
        let Primitive::SubtreeLayer(mut capture) = layer(source, region, 1.) else {
            unreachable!()
        };
        if !identity {
            capture.composite.shader = gpui_effects::pixelate_shader();
            capture.composite.uniforms.set_slot(0, [8., 0., 0., 0.]);
            capture
                .composite
                .uniforms
                .set_slot(1, [strength, 0., 0., 0.]);
        }
        let mut scene = Scene::default();
        scene.insert_primitive(quad(region, 0x203040ff));
        scene.insert_primitive(Primitive::SubtreeLayer(capture));
        scene.finish();
        scene
    };
    assert_eq!(
        renderer.render_rgba(&scene(0., false))?,
        renderer.render_rgba(&scene(0., true))?
    );
    let actual = renderer.render_rgba(&scene(1., false))?;
    let mut reference = Scene::default();
    reference.insert_primitive(quad(region, 0x203040ff));
    reference.insert_primitive(quad(bounds(0., 0., 8., 8.), 0xff000080));
    reference.insert_primitive(quad(bounds(16., 0., 4., 8.), 0x00ff00ff));
    reference.finish();
    let expected = renderer.render_rgba(&reference)?;
    for (x, y) in [(2, 2), (6, 6), (10, 2), (18, 2), (2, 10), (18, 10)] {
        let p = (y * 20 + x) * 4;
        assert!(
            actual[p..p + 4]
                .iter()
                .zip(&expected[p..p + 4])
                .all(|(a, b)| a.abs_diff(*b) <= 2),
            "pixel cells must retain sampled alpha and use the partial-cell center at {x},{y}"
        );
    }
    Ok(())
}
