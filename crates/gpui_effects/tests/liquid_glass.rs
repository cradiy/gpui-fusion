use std::borrow::Cow;

use gpui::{
    AtlasKey, AtlasTile, BackdropBlur, Bounds, ContentMask, DevicePixels, EffectQuad,
    EffectUniforms, ImageId, RenderImageParams, ScaledPixels as P, Scene, point, size,
};
use gpui_effects::{
    liquid_glass_compressed_shader, liquid_glass_content_shader, liquid_glass_full_surface_shader,
    liquid_glass_lens_shader, liquid_glass_shader,
};
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
            .with_slot(7, [0.8, 0.2, bulge * scale, -bulge * 0.25 * scale]),
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
    for scale in [1., 1.5, 2.] {
        renderer.resize(size(
            DevicePixels((80. * scale) as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for (bulge, ellipticity) in [(0., 0.), (3., 0.5), (3., 1.)] {
            let mut edge_samples = Vec::new();
            for step in 0..8 {
                let center_x = 40. + step as f32 / 8.;
                let uniforms = EffectUniforms::new()
                    .with_slot(0, [center_x * scale, 24. * scale, 48. * scale, 28. * scale])
                    .with_slot(1, [12. * scale; 4])
                    .with_slot(2, [0.8, 0.2, bulge * scale, -bulge * 0.25 * scale])
                    .with_slot(3, [1.; 4])
                    .with_slot(4, [0., 0., 0., 1.])
                    .with_slot(5, [ellipticity, 0., 0., 0.]);
                let actual = renderer.render_rgba(&content(tile, uniforms, 1., scale))?;
                let mut glass = glass(center_x, scale, bulge);
                glass.backdrop_blurs[0].uniforms = glass.backdrop_blurs[0]
                    .uniforms
                    .with_slot(6, [48. * scale, 28. * scale, 1. + ellipticity, 24. * scale]);
                let expected = renderer.render_rgba(&glass)?;
                let mut lens = glass;
                lens.backdrop_blurs[0].shader = Some(liquid_glass_lens_shader());
                lens.backdrop_blurs[0].uniforms = lens.backdrop_blurs[0]
                    .uniforms
                    .with_slot(5, [1.05, 3. * scale, 4. * scale, 0.35]);
                let lens_pixels = renderer.render_rgba(&lens)?;
                assert!(
                    lens_pixels
                        .iter()
                        .zip(&expected)
                        .all(|(a, b)| a.abs_diff(*b) <= 1),
                    "lens silhouette must match recolored content at scale {scale}, bulge {bulge}, step {step}"
                );
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
                if scale == 1. && bulge == 0. {
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
                for (x, stroke_width) in [(8., 3.), (50. + fraction, 3.), (59. + fraction, 1.)] {
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
            let flat = render(&mut renderer, 0., 10.)?;
            let refracted = render(&mut renderer, 12., 10.)?;
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
            assert_eq!(pixel(&flat, 61. + fraction), [0; 3]);
            let bent = pixel(&refracted, 61. + fraction);
            assert!(
                bent[2] > 180 && bent[0] < 10 && bent[1] < 10,
                "the blue stroke must bend beyond its source bounds at scale {scale}, offset {fraction}: {bent:?}"
            );
            assert_eq!(
                pixel(&flat, 51. + fraction),
                pixel(&refracted, 51. + fraction),
                "the flat interior preserves the stroke"
            );
            let compact = render(&mut renderer, 3., 6.)?;
            for x in 28..58 {
                assert_eq!(
                    pixel(&compact, x as f32 + fraction),
                    pixel(&flat, x as f32 + fraction),
                    "a compact lens preserves interior geometry at scale {scale}, offset {fraction}, x {x}"
                );
            }
            assert_eq!(pixel(&flat, 62. + fraction), [0; 3]);
            // A one-pixel source stroke can straddle two atlas samples. Look
            // for displaced coverage across the edge band rather than requiring
            // an opaque result at one subpixel phase.
            let edge = (61..64)
                .map(|x| pixel(&compact, x as f32 + fraction))
                .max_by_key(|pixel| pixel[2])
                .unwrap();
            assert!(
                edge[2] > 64 && edge[0] < 10 && edge[1] < 10,
                "the narrow band must still bend a crossing stroke at scale {scale}, offset {fraction}: {edge:?}"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn lens_reflects_the_surrounding_scene_at_its_edge_and_preserves_the_center() -> anyhow::Result<()>
{
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for fraction in [0., 0.375, 0.75] {
            let source = |color: [f32; 4], rect: Bounds<P>| {
                let uniforms = EffectUniforms::new()
                    .with_slot(3, color)
                    .with_slot(4, color);
                let mut primitive = content(tile, uniforms, 1., scale).effects.remove(0);
                primitive.bounds = rect;
                primitive.effect_bounds = rect;
                primitive
            };
            let render =
                |renderer: &mut WgpuOffscreenRenderer, lens: bool| -> anyhow::Result<Vec<u8>> {
                    let mut scene = Scene::default();
                    scene.insert_primitive(source(
                        [0., 0.4, 0., 1.],
                        bounds(0., 0., 80. * scale, 48. * scale),
                    ));
                    scene.insert_primitive(source(
                        [1., 0., 0., 1.],
                        bounds(
                            (64. + fraction) * scale,
                            12. * scale,
                            5. * scale,
                            24. * scale,
                        ),
                    ));
                    scene.insert_primitive(source(
                        [0., 0., 1., 1.],
                        bounds(
                            (38. + fraction) * scale,
                            12. * scale,
                            4. * scale,
                            24. * scale,
                        ),
                    ));
                    let mut surface = glass(40. + fraction, scale, 0.).backdrop_blurs.remove(0);
                    surface.uniforms = surface
                        .uniforms
                        .with_slot(0, [1., 1., 7. * scale, 12. * scale])
                        .with_slot(1, [0.; 4])
                        .with_slot(2, [0., 0., 0., 1.]);
                    if lens {
                        surface.shader = Some(liquid_glass_lens_shader());
                        surface.uniforms = surface
                            .uniforms
                            .with_slot(5, [1.025, 3. * scale, 4. * scale, 1.]);
                    }
                    scene.insert_primitive(surface);
                    scene.finish();
                    renderer.render_rgba(&scene)
                };
            let pane = render(&mut renderer, false)?;
            let lens = render(&mut renderer, true)?;
            let pixel = |pixels: &[u8], x: f32| {
                let i = ((24. * scale) as usize * width + (x * scale) as usize) * 4;
                [pixels[i], pixels[i + 1], pixels[i + 2]]
            };
            // Fractional motion can place the narrow reflected band between
            // raster samples. Measure reflected red coverage within the edge.
            let edge_end = ((64. + fraction) * scale).floor() as usize;
            let reflected = (edge_end.saturating_sub((4. * scale) as usize)..edge_end)
                .map(|x| {
                    let i = ((24. * scale) as usize * width + x) * 4;
                    (lens[i], pane[i])
                })
                .max_by_key(|(red, original)| red.saturating_sub(*original))
                .unwrap();
            assert!(
                reflected.0 > 5 && reflected.0 > reflected.1.saturating_add(3),
                "the lens edge must reflect the red scene outside its silhouette at scale {scale}, fraction {fraction}: {reflected:?}"
            );
            assert_eq!(
                pixel(&lens, 40. + fraction),
                [0, 0, 255],
                "central content stays legible"
            );
            assert_eq!(
                pixel(&lens, 71.),
                pixel(&pane, 71.),
                "uncovered background is unchanged"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn default_lens_transmission_does_not_add_a_dark_ring_to_a_uniform_scene() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    let optics = gpui_effects::LiquidGlassLensAppearance::default();
    let tint: gpui::Rgba = optics.tint.into();
    for scale in [1., 1.5, 2.] {
        renderer.resize(size(
            DevicePixels((80. * scale) as i32),
            DevicePixels((48. * scale) as i32),
        ));
        let color = [0.08, 0.17, 0.31, 1.];
        let source = || {
            content(
                tile,
                EffectUniforms::new()
                    .with_slot(3, color)
                    .with_slot(4, color),
                1.,
                scale,
            )
        };
        let expected = renderer.render_rgba(&source())?;
        for (fraction, bulge) in [(0., 0.), (0.375, 3.)] {
            let mut scene = Scene::default();
            scene.insert_primitive(source().effects.remove(0));
            let mut lens = glass(40. + fraction, scale, bulge).backdrop_blurs.remove(0);
            lens.shader = Some(liquid_glass_lens_shader());
            // Isolate default transmission from directional specular light.
            // Uniform surroundings cannot produce a contour-shaped absorption band.
            lens.uniforms = lens
                .uniforms
                .with_slot(
                    0,
                    [
                        optics.saturation,
                        optics.brightness,
                        optics.refraction.as_f32() * scale,
                        optics.thickness.as_f32() * scale,
                    ],
                )
                .with_slot(1, [tint.r, tint.g, tint.b, tint.a])
                .with_slot(
                    2,
                    [0., optics.edge_shadow, optics.dispersion, optics.clarity],
                )
                .with_slot(
                    5,
                    [
                        optics.magnification,
                        optics.edge_refraction.as_f32() * scale,
                        optics.edge_width.as_f32() * scale,
                        optics.fresnel,
                    ],
                );
            scene.insert_primitive(lens);
            scene.finish();
            let actual = renderer.render_rgba(&scene)?;
            assert!(
                actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "default glass transmission must preserve a uniform scene at scale {scale}, fraction {fraction}, bulge {bulge}"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn lens_transmission_keeps_scene_detail_registered_at_the_silhouette() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        let mut source = Scene::default();
        let white = EffectUniforms::new()
            .with_slot(3, [1.; 4])
            .with_slot(4, [1.; 4]);
        for rect in [
            bounds(16. * scale, 0., 1., 48. * scale),
            bounds(64. * scale, 0., 1., 48. * scale),
            bounds(0., 10. * scale, 80. * scale, 1.),
            bounds(0., 38. * scale, 80. * scale, 1.),
        ] {
            let mut stripe = content(tile, white, 1., scale).effects.remove(0);
            stripe.bounds = rect;
            stripe.effect_bounds = rect;
            source.insert_primitive(stripe);
        }
        source.finish();
        let expected = renderer.render_rgba(&source)?;
        let mut lens = glass(40. + 0.5 / scale, scale, 0.).backdrop_blurs.remove(0);
        lens.shader = Some(liquid_glass_lens_shader());
        lens.uniforms = lens
            .uniforms
            .with_slot(0, [1., 1., 7. * scale, 12. * scale])
            .with_slot(1, [0.; 4])
            .with_slot(2, [0., 0., 0.06, 1.])
            .with_slot(3, [-0.6, -0.8, 0.6 * scale, 40. * scale + 0.5])
            .with_slot(5, [1.05, 3. * scale, 5. * scale, 0.])
            .with_slot(6, [48. * scale, 28. * scale, 1., 24. * scale + 0.5]);
        source.insert_primitive(lens);
        source.finish();
        let actual = renderer.render_rgba(&source)?;
        for (x, y) in [(16., 24.), (64., 24.), (40., 10.), (40., 38.)] {
            let i = ((y * scale) as usize * width + (x * scale) as usize) * 4;
            assert!(
                actual[i..i + 3]
                    .iter()
                    .zip(&expected[i..i + 3])
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "transmission must meet the original scene at its boundary at scale {scale}, ({x},{y}): {:?} vs {:?}",
                &actual[i..i + 3],
                &expected[i..i + 3]
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn lens_transmission_preserves_scene_order_at_high_refraction() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        for ellipticity in [0., 0.5, 1.] {
            let mut scene = Scene::default();
            for x in 0..width {
                let value = 0.02 + 0.96 * x as f32 / (width - 1) as f32;
                let color = [value, value, value, 1.];
                let uniforms = EffectUniforms::new()
                    .with_slot(3, color)
                    .with_slot(4, color);
                let mut strip = content(tile, uniforms, 1., scale).effects.remove(0);
                strip.bounds = bounds(x as f32, 0., 1., 48. * scale);
                strip.effect_bounds = strip.bounds;
                scene.insert_primitive(strip);
            }
            let mut lens = glass(40.375, scale, 0.).backdrop_blurs.remove(0);
            lens.shader = Some(liquid_glass_lens_shader());
            lens.uniforms = lens
                .uniforms
                .with_slot(0, [1., 1., 100. * scale, 12. * scale])
                .with_slot(1, [0.; 4])
                .with_slot(2, [0., 0., 0.1, 1.])
                .with_slot(5, [1.2, 3. * scale, 4. * scale, 0.])
                .with_slot(6, [48. * scale, 28. * scale, 1. + ellipticity, 24. * scale]);
            scene.insert_primitive(lens);
            scene.finish();
            let image = renderer.render_rgba(&scene)?;
            for y in [18., 24., 30.] {
                let row = (y * scale) as usize;
                for x in 1..width {
                    for channel in 0..3 {
                        let previous = image[(row * width + x - 1) * 4 + channel];
                        let current = image[(row * width + x) * 4 + channel];
                        assert!(
                            current.saturating_add(2) >= previous,
                            "transmission reversed scene order at scale {scale}, ellipticity {ellipticity}, x {x}, y {row}, channel {channel}: {previous} -> {current}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn full_surface_refraction_bends_interior_detail_and_preserves_uncovered_scene()
-> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    for scale in [1., 1.5, 2.] {
        let width = (80. * scale) as usize;
        renderer.resize(size(
            DevicePixels(width as i32),
            DevicePixels((48. * scale) as i32),
        ));
        let mut source = Scene::default();
        for x in 0..width {
            let value = 0.02 + 0.96 * x as f32 / (width - 1) as f32;
            let color = [value, value, value, 1.];
            let uniforms = EffectUniforms::new()
                .with_slot(3, color)
                .with_slot(4, color);
            let mut strip = content(tile, uniforms, 1., scale).effects.remove(0);
            strip.bounds = bounds(x as f32, 0., 1., 48. * scale);
            strip.effect_bounds = strip.bounds;
            source.insert_primitive(strip);
        }
        let render = |renderer: &mut WgpuOffscreenRenderer,
                      full: bool,
                      reach: f32|
         -> anyhow::Result<Vec<u8>> {
            let mut scene = Scene::default();
            for strip in &source.effects {
                scene.insert_primitive(strip.clone());
            }
            let mut pane = glass(40., scale, 0.).backdrop_blurs.remove(0);
            pane.shader = Some(if full {
                liquid_glass_full_surface_shader()
            } else {
                liquid_glass_shader()
            });
            pane.uniforms = pane
                .uniforms
                .with_slot(0, [1., 1., reach * scale, 6. * scale])
                .with_slot(1, [0.; 4])
                .with_slot(2, [0., 0., 0., 1.]);
            scene.insert_primitive(pane);
            scene.finish();
            renderer.render_rgba(&scene)
        };
        let flat = render(&mut renderer, false, 0.)?;
        assert_eq!(
            flat,
            render(&mut renderer, true, 0.)?,
            "zero refraction must preserve the scene in both modes"
        );
        let edge = render(&mut renderer, false, 8.)?;
        let full = render(&mut renderer, true, 8.)?;
        let pixel = |image: &[u8], x: f32| {
            image[((24. * scale) as usize * width + (x * scale) as usize) * 4]
        };
        for x in [32., 48.] {
            assert_eq!(
                pixel(&flat, x),
                pixel(&edge, x),
                "edge-only refraction leaves the flat interior unchanged"
            );
            assert!(
                pixel(&flat, x).abs_diff(pixel(&full, x)) >= 2,
                "full refraction must displace interior scene detail at scale {scale}, x {x}"
            );
        }
        assert!(
            pixel(&full, 32.) > pixel(&flat, 32.) && pixel(&full, 48.) < pixel(&flat, 48.),
            "interior rays must converge toward the center"
        );
        for x in [8., 72.] {
            assert_eq!(
                pixel(&flat, x),
                pixel(&full, x),
                "uncovered scene must remain unchanged"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires a GPU adapter"]
fn highlight_compression_preserves_sharp_scene_detail_without_a_color_wash() -> anyhow::Result<()> {
    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(80), DevicePixels(48)))?;
    let tile = white_mask(&renderer);
    let render =
        |renderer: &mut WgpuOffscreenRenderer, compressed: bool| -> anyhow::Result<Vec<u8>> {
            let mut scene = Scene::default();
            for x in 0..80 {
                let color = if x % 2 == 0 {
                    [0.12, 0.04, 0.02, 1.]
                } else {
                    [0.8, 0.3, 0.1, 1.]
                };
                let uniforms = EffectUniforms::new()
                    .with_slot(3, color)
                    .with_slot(4, color);
                let mut strip = content(tile, uniforms, 1., 1.).effects.remove(0);
                strip.bounds = bounds(x as f32, 0., 1., 48.);
                strip.effect_bounds = strip.bounds;
                scene.insert_primitive(strip);
            }
            let mut pane = glass(40., 1., 0.).backdrop_blurs.remove(0);
            pane.shader = Some(if compressed {
                liquid_glass_compressed_shader(false)
            } else {
                liquid_glass_shader()
            });
            pane.uniforms = pane
                .uniforms
                .with_slot(0, [1., 1., 0., 10.])
                .with_slot(1, [0.; 4])
                .with_slot(2, [0., 0., 0., 1.]);
            scene.insert_primitive(pane);
            scene.finish();
            renderer.render_rgba(&scene)
        };
    let original = render(&mut renderer, false)?;
    let compressed = render(&mut renderer, true)?;
    let decode = |value: u8| {
        let value = value as f32 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let encode = |value: f32| {
        (255.
            * if value <= 0.0031308 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            })
        .round() as u8
    };
    for x in 24..56 {
        let i = (24 * 80 + x) * 4;
        let channels = [
            decode(original[i]),
            decode(original[i + 1]),
            decode(original[i + 2]),
        ];
        let peak = channels.into_iter().fold(0., f32::max);
        for channel in 0..3 {
            let expected = encode(channels[channel] / (1. + 2. * peak));
            assert!(
                compressed[i + channel].abs_diff(expected) <= 2,
                "compression must preserve each original pixel and RGB ratio, x={x}, channel={channel}"
            );
        }
        let next = i + 4;
        assert!(
            compressed[i].abs_diff(compressed[next]) > 20,
            "single-pixel scene detail must remain sharp"
        );
    }
    for x in [8, 72] {
        let i = (24 * 80 + x) * 4;
        assert_eq!(
            &original[i..i + 4],
            &compressed[i..i + 4],
            "uncovered scene must be unchanged"
        );
    }
    Ok(())
}
