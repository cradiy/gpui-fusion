use gpui::{
    BackdropBlur, BackdropShader, Bounds, ContentMask, Corners, DevicePixels, EffectUniforms,
    ScaledPixels as P, Scene, point, px, size,
};
use gpui_effects::liquid_glass_shader;
use gpui_wgpu::WgpuOffscreenRenderer;
use uic::components::glass::GlassSegmentedAppearance;

#[test]
#[ignore = "requires a GPU adapter"]
fn pressed_lens_preserves_and_displaces_nested_surface_contours() -> anyhow::Result<()> {
    for scale in [1., 1.5, 2.] {
        let width = (256. * scale) as usize;
        let mut renderer = WgpuOffscreenRenderer::new(size(
            DevicePixels(width as i32),
            DevicePixels(width as i32),
        ))?;
        let viewport = Bounds::new(point(P(0.), P(0.)), size(P(width as f32), P(width as f32)));
        let backdrop = |shader, uniforms| BackdropBlur {
            order: 0,
            bounds: viewport,
            content_mask: ContentMask { bounds: viewport },
            corner_radii: Default::default(),
            blur_radius: P(0.),
            opacity: 1.,
            shader: Some(shader),
            uniforms,
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        };
        let mut scene = Scene::default();
        scene.insert_primitive(backdrop(
            BackdropShader::wgsl(r#"
                fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
                    let y = abs(input.position.y / params.slots[0].x - 128.0);
                    let contour = y >= 36.0 && y <= 38.0;
                    return select(vec4<f32>(0.2, 0.2, 0.2, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), contour);
                }
            "#),
            EffectUniforms::new().with_slot(0, [scale, 0., 0., 0.]),
        ));
        let mut material = GlassSegmentedAppearance::light().selection;
        material.refraction *= 88. / 120.;
        material.thickness *= 88. / 120.;
        material.frost = 0.;
        material.optics.rim_strength = 0.;
        material.optics.top_reflection = 0.;
        material.optics.inner_shading = 0.;
        material.optics.caustic_strength = 0.;
        let uniforms = material
            .uniforms(Corners::all(px(40.)), scale)
            .with_slot(3, [0., 0., 0., 128. * scale])
            .with_slot(6, [160. * scale, 88. * scale, 1., 128. * scale]);
        scene.insert_primitive(backdrop(liquid_glass_shader(), uniforms));
        scene.finish();
        let pixels = renderer.render_rgba(&scene)?;
        let displaced = ((85. * scale) as usize..(104. * scale) as usize)
            .filter(|y| {
                let source_y = (*y as f32 + 0.5) / scale;
                !(89.5..=92.5).contains(&source_y)
            })
            .any(|y| {
                let pixel = &pixels[(y * width + (128. * scale) as usize) * 4..][..3];
                pixel[0] > 180 && pixel[1] < 80 && pixel[2] < 80
            });
        assert!(
            displaced,
            "nested contour must remain visible and bend inside the lens at scale {scale}"
        );
    }
    Ok(())
}
