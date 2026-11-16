use std::borrow::Cow;

use gpui::{
    AtlasKey, AtlasTile, BackdropBlur, Bounds, ContentMask, DevicePixels, EffectQuad,
    EffectUniforms, ImageId, RenderImageParams, ScaledPixels as P, Scene, point, size,
};
use gpui_effects::{liquid_glass_content_shader, liquid_glass_shader};
use gpui_wgpu::WgpuOffscreenRenderer;

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<P> {
    Bounds::new(point(P(x), P(y)), size(P(w), P(h)))
}

fn white_mask(renderer: &WgpuOffscreenRenderer) -> AtlasTile {
    renderer
        .sprite_atlas()
        .get_or_insert_with(
            &AtlasKey::Image(RenderImageParams {
                image_id: ImageId(981),
                frame_index: 0,
            }),
            &mut || {
                Ok(Some((
                    size(DevicePixels(1), DevicePixels(1)),
                    Cow::Owned(vec![255; 4]),
                )))
            },
        )
        .unwrap()
        .unwrap()
}

fn content(tile: AtlasTile, uniforms: EffectUniforms, opacity: f32, scale: f32) -> Scene {
    let viewport = bounds(0., 0., 80. * scale, 48. * scale);
    let mut scene = Scene::default();
    scene.insert_primitive(EffectQuad {
        order: 0,
        bounds: viewport,
        effect_bounds: viewport,
        transformation: Default::default(),
        content_mask: ContentMask { bounds: viewport },
        shader: liquid_glass_content_shader(),
        uniforms,
        time: 0.,
        corner_radii: Default::default(),
        opacity,
        image_tile: Some(tile),
        second_image_tile: None,
        third_image_tile: None,
        fourth_image_tile: None,
    });
    scene.finish();
    scene
}

fn glass(center_x: f32, scale: f32, bulge: f32) -> Scene {
    let viewport = bounds(0., 0., 80. * scale, 48. * scale);
    let mut scene = Scene::default();
    scene.insert_primitive(BackdropBlur {
        order: 0,
        bounds: viewport,
        content_mask: ContentMask { bounds: viewport },
        corner_radii: Default::default(),
        blur_radius: P(0.),
        opacity: 1.,
        shader: Some(liquid_glass_shader()),
        uniforms: EffectUniforms::new()
            .with_slot(0, [1., 1., 0., 0.])
            .with_slot(1, [1.; 4])
            .with_slot(3, [-0.6, -0.8, 0., center_x * scale])
            .with_slot(4, [12. * scale; 4])
            .with_slot(6, [48. * scale, 28. * scale, 1., 24. * scale])
            .with_slot(7, [0.8, 0.2, bulge * scale, -bulge * 0.25 * scale])
            .with_slot(8, [0.; 4])
            .with_slot(10, [0., 0., 0., scale]),
        time: 0.,
        pointer: point(0., 0.),
        pointer_active: false,
    });
    scene.finish();
    scene
}

#[test]
#[ignore = "requires a GPU adapter"]
fn content_coverage_matches_glass_during_fractional_motion_and_deformation() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    let coverage_shader = |shader: gpui::BackdropShader| {
        gpui::BackdropShader::wgsl(format!(
            "{}\nfn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {{ let material = coverage_material(input, params); return vec4<f32>(vec3<f32>(1.0), material.a); }}",
            shader
                .wgsl_source()
                .replace("fn backdrop_effect(", "fn coverage_material(")
        ))
    };
    for scale in [1., 1.5, 2.] {
        renderer.resize(size(
            DevicePixels((80. * scale) as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for (bulge, ellipticity, roundness, merged) in [
            (0., 0., 2., false),
            (3., 0.5, 2., false),
            (3., 1., 2., false),
            (0., 0., 5., true),
            (2., 0., 7., true),
        ] {
            let circle = if merged {
                [18. * scale, 4. * scale, 16. * scale, 10. * scale]
            } else {
                [0.; 4]
            };
            let mut edge_samples = Vec::new();
            for step in 0..8 {
                let center_x = 40. + step as f32 / 8.;
                let uniforms = EffectUniforms::new()
                    .with_slot(0, [center_x * scale, 24. * scale, 48. * scale, 28. * scale])
                    .with_slot(1, [12. * scale; 4])
                    .with_slot(2, [0.8, 0.2, bulge * scale, -bulge * 0.25 * scale])
                    .with_slot(3, [1.; 4])
                    .with_slot(4, [0., 0., 0., 1.])
                    .with_slot(5, [ellipticity, roundness, 0., 0.])
                    .with_slot(6, circle);
                let actual = renderer.render_rgba(&content(tile, uniforms, 1., scale))?;
                let mut glass = glass(center_x, scale, bulge);
                glass.backdrop_blurs[0].uniforms = glass.backdrop_blurs[0]
                    .uniforms
                    .with_slot(6, [48. * scale, 28. * scale, 1. + ellipticity, 24. * scale])
                    .with_slot(11, [roundness, 0., 0., 0.])
                    .with_slot(12, circle);
                glass.backdrop_blurs[0].shader = Some(coverage_shader(liquid_glass_shader()));
                let expected = renderer.render_rgba(&glass)?;
                // Backdrop composition can quantize coverage in an intermediate
                // UNORM target. Compare linear coverage, not amplified dark sRGB values.
                let linear = |value: u8| {
                    let value = value as f32 / 255.;
                    if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    }
                };
                let worst = actual
                    .chunks_exact(4)
                    .zip(expected.chunks_exact(4))
                    .enumerate()
                    .max_by(|(_, (a, b)), (_, (c, d))| {
                        (linear(a[0]) - linear(b[0]))
                            .abs()
                            .total_cmp(&(linear(c[0]) - linear(d[0])).abs())
                    })
                    .unwrap();
                assert!(
                    actual
                        .chunks_exact(4)
                        .zip(expected.chunks_exact(4))
                        .all(|(a, b)| a[0].abs_diff(b[0]) <= 1
                            || (linear(a[0]) - linear(b[0])).abs() <= 2. / 255.),
                    "content boundary must follow glass at scale {scale}, bulge {bulge}, step {step}: worst {worst:?}"
                );
                if scale == 1. && bulge == 0. && !merged {
                    edge_samples.push(expected[(24 * 80 + 16) * 4]);
                }
            }
            if !edge_samples.is_empty() {
                assert!(
                    edge_samples.windows(2).all(|pair| pair[0] > pair[1]),
                    "fractional motion must not hold then jump at integer pixels: {edge_samples:?}"
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn content_recolors_only_covered_pixels_and_preserves_transparency() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    let uniforms = EffectUniforms::new()
        .with_slot(0, [60., 24., 40., 48.])
        .with_slot(3, [0., 0., 1., 1.])
        .with_slot(4, [1., 0., 0., 0.5]);
    let pixels = renderer.render_rgba(&content(tile, uniforms, 0.5, 1.))?;
    let left = &pixels[(24 * 80 + 20) * 4..][..3];
    let right = &pixels[(24 * 80 + 60) * 4..][..3];
    assert_eq!([left[0], left[1], left[2]], [right[2], right[1], right[0]]);
    assert_eq!(left[1..], [0, 0]);
    // Linear alpha 0.5 * element opacity 0.5, encoded into an sRGB render target.
    assert!(
        (135..=139).contains(&left[0]),
        "alpha applied exactly once: {left:?}"
    );
    let transparent = uniforms.with_slot(4, [1., 0., 0., 0.]);
    let pixels = renderer.render_rgba(&content(tile, transparent, 1., 1.))?;
    assert!(pixels.chunks_exact(4).all(|pixel| pixel[..3] == [0, 0, 0]));
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn glass_refracts_recolored_content_beyond_its_original_bounds() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for fraction in [0., 0.25, 0.5, 0.75] {
            let center = 40. + fraction;
            let source = EffectUniforms::new()
                .with_slot(0, [center * scale, 24. * scale, 48. * scale, 28. * scale])
                .with_slot(1, [12. * scale; 4])
                .with_slot(3, [0., 0., 1., 1.])
                .with_slot(4, [1., 0., 0., 1.]);
            let render = |renderer: &mut WgpuOffscreenRenderer,
                          refraction: f32,
                          thickness: f32|
             -> anyhow::Result<Vec<u8>> {
                let mut scene = Scene::default();
                for (x, stroke_width) in [(8., 3.), (50. + fraction, 3.), (59. + fraction, 2.)] {
                    let mut stroke = content(tile, source, 1., scale).effects.remove(0);
                    stroke.bounds =
                        bounds(x * scale, 4. * scale, stroke_width * scale, 40. * scale);
                    stroke.effect_bounds = stroke.bounds;
                    scene.insert_primitive(stroke);
                }
                let mut lens = glass(center, scale, 0.).backdrop_blurs.remove(0);
                lens.uniforms = lens
                    .uniforms
                    .with_slot(0, [1., 1., refraction * scale, thickness * scale])
                    .with_slot(1, [0.; 4])
                    .with_slot(2, [0., 0., 0., 1.]);
                scene.insert_primitive(lens);
                scene.finish();
                renderer.render_rgba(&scene)
            };
            let flat = render(&mut renderer, 0., 14.)?;
            let refracted = render(&mut renderer, 12., 14.)?;
            let pixel = |pixels: &[u8], x: f32| {
                let index = ((24. * scale) as usize * width + (x * scale) as usize) * 4;
                [pixels[index], pixels[index + 1], pixels[index + 2]]
            };
            assert_eq!(pixel(&flat, 9.), pixel(&refracted, 9.));
            assert_eq!(
                pixel(&refracted, 9.),
                [255, 0, 0],
                "uncovered content keeps its source color"
            );
            let bent = ((60. * scale) as usize..(64. * scale) as usize)
                .map(|x| (x as f32 + 0.5) / scale)
                .filter(|x| pixel(&flat, *x)[2] < 10)
                .map(|x| pixel(&refracted, x))
                .max_by_key(|pixel| pixel[2])
                .unwrap();
            assert!(
                bent[2] > bent[0].saturating_add(8) && bent[0] < 10 && bent[1] < 10,
                "the blue stroke must bend beyond its source bounds at scale {scale}, offset {fraction}: {bent:?}"
            );
            assert_eq!(
                pixel(&flat, 51. + fraction),
                pixel(&refracted, 51. + fraction),
                "the flat interior preserves the stroke"
            );
            let compact = render(&mut renderer, 5., 6.)?;
            for x in 28..58 {
                assert_eq!(
                    pixel(&compact, x as f32 + fraction),
                    pixel(&flat, x as f32 + fraction),
                    "a compact lens preserves interior geometry at scale {scale}, offset {fraction}, x {x}"
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn untinted_lens_material_matches_reference_profile() -> anyhow::Result<()> {
    use gpui::{BackdropShader, Corners, Quad, px, rgb};
    use gpui_effects::LiquidGlassStyle;

    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(384), DevicePixels(256)))?;
    let viewport = bounds(0., 0., 384., 256.);
    let reference = BackdropShader::wgsl(include_str!("support/liquid_glass_reference.wgsl"))
        .with_mipmapped_backdrop();
    let reference_source = gpui::compose_backdrop_shader_wgsl(&reference);
    let module = naga::front::wgsl::parse_str(&reference_source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&reference_source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&reference_source)));
    for (half_width, half_height, radius, roundness) in [(110., 80., 8., 5.), (67., 39., 26., 2.)] {
        for circle_x in [None, Some(-150.), Some(-175.), Some(-205.)] {
            for frost in [0., 0.65] {
                for (magnification, scale) in
                    [(0., 1.), (0.12, 1.), (0.4, 1.), (0.12, 1.5), (0.12, 2.)]
                {
                    let mut appearance = LiquidGlassStyle::optical();
                    appearance.shape.roundness = roundness;
                    appearance.frost = frost;
                    appearance.optics.magnification = magnification;
                    appearance.blur_radius = px(6.);
                    appearance.shape.merge = circle_x.map(|x| gpui_effects::LiquidGlassMerge {
                        offset: gpui::point(px(x / scale), px(0.)),
                        radius: px(48. / scale),
                        smoothness: px(40. / scale),
                    });
                    let uniforms = appearance
                        .uniforms(Corners::all(px(radius / scale)), scale)
                        .with_slot(3, [0., 0., 0., 192.])
                        .with_slot(6, [half_width * 2., half_height * 2., 1., 128.]);
                    let render = |renderer: &mut WgpuOffscreenRenderer, shader| {
                        let mut scene = Scene::default();
                        scene.insert_primitive(Quad {
                            bounds: viewport,
                            content_mask: ContentMask { bounds: viewport },
                            background: rgb(0x33485a).into(),
                            ..Default::default()
                        });
                        for (ix, color) in [0xe6b455, 0x704cb8, 0x49bea1, 0xf0e7d0]
                            .into_iter()
                            .enumerate()
                        {
                            scene.insert_primitive(Quad {
                                bounds: bounds(35. + ix as f32 * 90., 0., 32., 256.),
                                content_mask: ContentMask { bounds: viewport },
                                background: rgb(color).into(),
                                ..Default::default()
                            });
                        }
                        for column in 0..16 {
                            scene.insert_primitive(Quad {
                                bounds: bounds(115. + column as f32 * 10., 94., 2., 68.),
                                content_mask: ContentMask { bounds: viewport },
                                background: rgb(0x151515).into(),
                                ..Default::default()
                            });
                        }
                        scene.insert_primitive(BackdropBlur {
                            order: 0,
                            bounds: viewport,
                            content_mask: ContentMask { bounds: viewport },
                            corner_radii: Default::default(),
                            blur_radius: P(6. * scale),
                            opacity: 1.,
                            shader: Some(shader),
                            uniforms,
                            time: 0.,
                            pointer: point(0., 0.),
                            pointer_active: false,
                        });
                        scene.finish();
                        renderer.render_rgba(&scene)
                    };
                    let actual = render(&mut renderer, liquid_glass_shader())?;
                    let expected = render(&mut renderer, reference.clone())?;
                    // Compare the entire interior, including superellipse corners.
                    let mut worst = 0;
                    let mut location = (0, 0, [0; 3], [0; 3]);
                    for y in 0..256 {
                        for x in 0..384 {
                            let qx = ((x as f32 + 0.5 - 192.).abs() - half_width + radius).max(0.);
                            let qy = ((y as f32 + 0.5 - 128.).abs() - half_height + radius).max(0.);
                            let signed_x = (x as f32 + 0.5 - 192.).abs() - half_width + radius;
                            let signed_y = (y as f32 + 0.5 - 128.).abs() - half_height + radius;
                            let mut distance = (qx.powf(roundness) + qy.powf(roundness))
                                .powf(1. / roundness)
                                - radius
                                + signed_x.max(signed_y).min(0.);
                            if let Some(offset) = circle_x {
                                let circle = ((x as f32 + 0.5 - 192. - offset).powi(2)
                                    + (y as f32 + 0.5 - 128.).powi(2))
                                .sqrt()
                                    - 48.;
                                let h = (0.5 + 0.5 * (circle - distance) / 40.).clamp(0., 1.);
                                distance = circle * (1. - h) + distance * h - 40. * h * (1. - h);
                            }
                            if distance > -2. {
                                continue;
                            }
                            let offset = (y * 384 + x) * 4;
                            for channel in 0..3 {
                                let difference =
                                    actual[offset + channel].abs_diff(expected[offset + channel]);
                                if difference > worst {
                                    worst = difference;
                                    location = (
                                        x,
                                        y,
                                        [actual[offset], actual[offset + 1], actual[offset + 2]],
                                        [
                                            expected[offset],
                                            expected[offset + 1],
                                            expected[offset + 2],
                                        ],
                                    );
                                }
                            }
                        }
                    }
                    assert!(
                        worst <= 3,
                        "reference mismatch: circle={circle_x:?}, magnification={magnification}, scale={scale}, frost={frost}, worst={worst}, location={location:?}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn optical_material_preserves_transparent_backdrops() -> anyhow::Result<()> {
    use gpui_effects::LiquidGlassStyle;

    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let source = liquid_glass_shader()
        .wgsl_source()
        .replace("fn backdrop_effect(", "fn optical_material(")
        .replace("sample_backdrop_lod(", "sample_empty_backdrop(");
    let shader = gpui::BackdropShader::wgsl(format!(
        "{source}\nfn sample_empty_backdrop(input: BackdropInput, offset: vec2<f32>, lod: f32) -> vec4<f32> {{ return vec4<f32>(0.0); }}\nfn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {{ let material = optical_material(input, params); return vec4<f32>(vec3<f32>(material.a), 1.0); }}"
    ));
    for (material, expected) in [
        (LiquidGlassStyle::clear(), 0..=0),
        (LiquidGlassStyle::frosted(), 1..=254),
        (
            LiquidGlassStyle::clear().tint(gpui::hsla(0., 0., 1., 1.)),
            255..=255,
        ),
    ] {
        let mut scene = glass(40., 1., 0.);
        let surface = &mut scene.backdrop_blurs[0];
        surface.uniforms = material.uniforms(Default::default(), 1.);
        surface.shader = Some(shader.clone());
        let pixels = renderer.render_rgba(&scene)?;
        let center = pixels[(24 * 80 + 40) * 4];
        assert!(
            expected.contains(&center),
            "unexpected glass alpha over an empty backdrop: {center}, expected {expected:?}"
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn environment_reflection_follows_curvature_without_rim_lighting() -> anyhow::Result<()> {
    use gpui::{Corners, Quad, px, rgb};
    use gpui_effects::LiquidGlassStyle;

    let viewport = bounds(0., 0., 160., 112.);
    let pane = bounds(16., 16., 128., 80.);
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(160), DevicePixels(112)))?;
    let mut material = LiquidGlassStyle::clear().thickness(px(16.));
    material.optics.rim_strength = 0.;
    material.optics.caustic_strength = 0.;
    let mut frames = Vec::new();
    for (reflection, direction) in [
        (0., None),
        (0.5, None),
        (0.5, Some(point(-1., 0.))),
        (0.5, Some(point(1., 0.))),
    ] {
        material.optics.environment_reflection = reflection;
        material.optics.directional_light = f32::from(direction.is_some());
        material.optics.light_direction = direction.unwrap_or(point(0., -1.));
        let mut scene = Scene::default();
        scene.insert_primitive(Quad {
            bounds: viewport,
            content_mask: ContentMask { bounds: viewport },
            background: rgb(0x202830).into(),
            ..Default::default()
        });
        scene.insert_primitive(BackdropBlur {
            order: 1,
            bounds: pane,
            content_mask: ContentMask { bounds: viewport },
            corner_radii: Default::default(),
            blur_radius: P(0.),
            opacity: 1.,
            shader: Some(liquid_glass_shader()),
            uniforms: material.uniforms(Corners::all(px(12.)), 1.),
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        });
        scene.finish();
        frames.push(renderer.render_rgba(&scene)?);
    }
    let pixel =
        |frame: usize, x: usize, y: usize| &frames[frame][(y * 160 + x) * 4..(y * 160 + x + 1) * 4];
    assert_eq!(pixel(0, 80, 56), pixel(1, 80, 56));
    assert_eq!(pixel(0, 80, 8), pixel(1, 80, 8));
    assert!(pixel(1, 80, 18)[0] > pixel(0, 80, 18)[0] + 8);
    assert!(pixel(1, 80, 18)[0] > pixel(1, 80, 28)[0]);
    assert!(pixel(2, 18, 56)[0] > pixel(3, 18, 56)[0]);
    assert!(pixel(3, 141, 56)[0] > pixel(2, 141, 56)[0]);
    assert_eq!(pixel(2, 80, 56), pixel(3, 80, 56));
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn rounded_panes_form_and_release_a_continuous_neck() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let coverage_shader = gpui::BackdropShader::wgsl(format!(
        "{}\nfn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {{ let material = coverage_material(input, params); return vec4<f32>(vec3<f32>(1.0), material.a); }}",
        liquid_glass_shader()
            .wgsl_source()
            .replace("fn backdrop_effect(", "fn coverage_material(")
    ));
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for (offset, joined) in [(30., true), (42., false)] {
            let mut scene = glass(30., scale, 0.);
            let pane = &mut scene.backdrop_blurs[0];
            pane.shader = Some(coverage_shader.clone());
            pane.uniforms = pane
                .uniforms
                .with_slot(4, [12. * scale; 4])
                .with_slot(6, [32. * scale, 24. * scale, 1., 24. * scale])
                .with_slot(11, [2., 20. * scale, 18. * scale, 9. * scale])
                .with_slot(12, [offset * scale, 0., 0., 16. * scale]);
            let image = renderer.render_rgba(&scene)?;
            let red =
                |x: f32, y: f32| image[(((y * scale) as usize * width) + (x * scale) as usize) * 4];
            assert!(red(30., 24.) > 245 && red(30. + offset, 24.) > 245);
            if joined {
                for x in 45..53 {
                    assert!(
                        red(x as f32, 24.) > 245,
                        "gap in the fused neck at scale {scale}, x {x}"
                    );
                }
                assert!(red(48., 9.) < 5, "union must retain curved shoulders");
            } else {
                assert!(
                    red(54., 24.) < 5,
                    "detached panes still connected at scale {scale}"
                );
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn tinted_glass_preserves_color_and_transmits_backdrop_tones() -> anyhow::Result<()> {
    use gpui::{Corners, Quad, px, rgb};
    use gpui_effects::LiquidGlassStyle;

    let viewport = bounds(0., 0., 128., 64.);
    let pane = bounds(8., 8., 112., 48.);
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(128), DevicePixels(64)))?;
    let mut material = LiquidGlassStyle::clear().thickness(px(8.));
    material.optics.top_reflection = 0.;
    material.optics.inner_shading = 0.;
    material.optics.environment_reflection = 0.;
    let render = |renderer: &mut WgpuOffscreenRenderer, material: LiquidGlassStyle| {
        let mut scene = Scene::default();
        for (x, color) in [(0., 0x18212a), (64., 0xf5eade)] {
            scene.insert_primitive(Quad {
                bounds: bounds(x, 0., 64., 64.),
                content_mask: ContentMask { bounds: viewport },
                background: rgb(color).into(),
                ..Default::default()
            });
        }
        scene.insert_primitive(BackdropBlur {
            order: 1,
            bounds: pane,
            content_mask: ContentMask { bounds: viewport },
            corner_radii: Corners::all(P(8.)),
            blur_radius: P(0.),
            opacity: 1.,
            shader: Some(liquid_glass_shader()),
            uniforms: material.uniforms(Corners::all(px(8.)), 1.),
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        });
        scene.finish();
        renderer.render_rgba(&scene)
    };
    let pixel = |pixels: &[u8], x: usize, y: usize| {
        let i = (y * 128 + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    for tint in [0x2264c8, 0xd23f65, 0x15906b] {
        material.tint = rgb(tint).into();
        let fixed = render(&mut renderer, material.transmission(0.))?;
        assert_eq!(pixel(&fixed, 32, 32), pixel(&fixed, 96, 32));
        let transmitted = render(&mut renderer, material.transmission(1.))?;
        let dark = pixel(&transmitted, 32, 32);
        let light = pixel(&transmitted, 96, 32);
        let dominant = (0..3).max_by_key(|&c| light[c]).unwrap();
        assert!(
            light[dominant] > dark[dominant] + 20,
            "backdrop detail: {dark:?}, {light:?}"
        );
        for sample in [dark, light] {
            assert_eq!(sample[3], 255);
            let max = *sample[..3].iter().max().unwrap() as f32;
            let min = *sample[..3].iter().min().unwrap() as f32;
            assert!((max - min) / max > 0.6, "washed out body color: {sample:?}");
            let actual: gpui::Hsla = gpui::rgb(
                (u32::from(sample[0]) << 16) | (u32::from(sample[1]) << 8) | u32::from(sample[2]),
            )
            .into();
            let expected: gpui::Hsla = rgb(tint).into();
            let delta = (actual.h - expected.h).abs();
            assert!(delta.min(1. - delta) < 0.025, "hue changed: {sample:?}");
        }
        let reflected = render(&mut renderer, {
            let mut material = material.transmission(1.);
            material.optics.environment_reflection = 0.55;
            material
        })?;
        assert_eq!(pixel(&transmitted, 32, 32), pixel(&reflected, 32, 32));
        assert_ne!(pixel(&transmitted, 32, 10), pixel(&reflected, 32, 10));
    }
    material.tint.a = 0.;
    assert_eq!(
        render(&mut renderer, material.transmission(0.))?,
        render(&mut renderer, material.transmission(1.))?,
        "transmission must not alter untinted glass"
    );
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn ring_refraction_preserves_the_hole_and_outer_backdrop() -> anyhow::Result<()> {
    use gpui::{Corners, Quad, px, rgb};
    use gpui_effects::LiquidGlassStyle;

    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(96), DevicePixels(96)))?;
    for scale in [1., 1.5, 2.] {
        let side = (96. * scale) as i32;
        renderer.resize(size(DevicePixels(side), DevicePixels(side)));
        let viewport = bounds(0., 0., side as f32, side as f32);
        let backdrop = || {
            let mut scene = Scene::default();
            for column in 0..24 {
                scene.insert_primitive(Quad {
                    bounds: bounds(column as f32 * 4. * scale, 0., 4. * scale, 96. * scale),
                    content_mask: ContentMask { bounds: viewport },
                    background: rgb(if column % 2 == 0 { 0x30485d } else { 0xb7d2e8 }).into(),
                    ..Default::default()
                });
            }
            scene
        };
        let mut baseline = backdrop();
        baseline.finish();
        let expected = renderer.render_rgba(&baseline)?;
        let style = LiquidGlassStyle::clear()
            .thickness(px(5.))
            .refraction(px(4.));
        let mut scene = backdrop();
        scene.insert_primitive(BackdropBlur {
            order: 0,
            bounds: viewport,
            content_mask: ContentMask { bounds: viewport },
            corner_radii: Corners::default(),
            blur_radius: P(0.),
            opacity: 1.,
            shader: Some(liquid_glass_shader()),
            uniforms: style
                .ring_uniforms(px(8.), scale)
                .with_slot(3, [0., 0., 0., 48. * scale])
                .with_slot(6, [64. * scale, 64. * scale, 1., 48. * scale]),
            time: 0.,
            pointer: point(0., 0.),
            pointer_active: false,
        });
        scene.finish();
        let actual = renderer.render_rgba(&scene)?;
        let optics = scene.backdrop_blurs[0].uniforms.slots()[0];
        scene.backdrop_blurs[0].uniforms = scene.backdrop_blurs[0]
            .uniforms
            .with_slot(0, [optics[0], optics[1], 0., optics[3]]);
        let unwarped = renderer.render_rgba(&scene)?;
        let mut changed_band_pixels = 0;
        for y in 0..side as usize {
            for x in 0..side as usize {
                let radius = ((x as f32 + 0.5 - 48. * scale).powi(2)
                    + (y as f32 + 0.5 - 48. * scale).powi(2))
                .sqrt()
                    / scale;
                let offset = (y * side as usize + x) * 4;
                let difference = (0..3)
                    .map(|channel| actual[offset + channel].abs_diff(expected[offset + channel]))
                    .max()
                    .unwrap();
                if radius < 23. || radius > 33. {
                    assert!(
                        difference <= 1,
                        "ring modified backdrop outside its band at {x},{y}, scale {scale}"
                    );
                } else if (0..3).any(|channel| {
                    actual[offset + channel].abs_diff(unwarped[offset + channel]) > 8
                }) {
                    changed_band_pixels += 1;
                }
            }
        }
        assert!(
            changed_band_pixels > 80,
            "ring must visibly refract its band at scale {scale}"
        );
    }
    Ok(())
}
