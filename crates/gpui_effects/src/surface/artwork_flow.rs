use std::{sync::Arc, time::Instant};

use gpui::{
    App, Bounds, EffectShader, EffectUniforms, Element, ElementId, GlobalElementId, ImageId,
    ImageSource, InspectorElementId, IntoElement, LayoutId, PaintEffect, Pixels, RenderImage,
    Style, StyleRefinement, Styled, Window, prelude::*,
};

const THUMBNAIL_SIZE: u32 = 40;
const CROSSFADE_SECONDS: f32 = 5. / 3.;

/// Appearance of a softly layered, image-derived background.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct ArtworkFlowOptions {
    /// Artwork brightness in `0..=1`, before the backdrop overlays.
    pub brightness: f32,
    /// Chroma multiplier; one preserves the source saturation.
    pub saturation: f32,
    /// Contrast around the middle gray value.
    pub contrast: f32,
    /// Animation speed multiplier. Zero fixes all layer transforms.
    pub motion: f32,
    /// Stable, spatial dithering strength in `0..=2`.
    pub dither: f32,
}

impl Default for ArtworkFlowOptions {
    fn default() -> Self {
        Self {
            brightness: 0.7,
            saturation: 2.75,
            contrast: 1.9,
            motion: 1.,
            dither: 0.,
        }
    }
}

impl ArtworkFlowOptions {
    fn uniforms(self, elapsed: f32, blend: f32) -> EffectUniforms {
        let time = elapsed * self.motion;
        let mut uniforms = EffectUniforms::new().with_slot(
            0,
            [
                self.brightness.clamp(0., 1.),
                self.saturation.max(0.),
                self.contrast.max(0.),
                blend,
            ],
        );
        for (index, speed) in [0.09, -0.24, 0.18, -0.12].into_iter().enumerate() {
            let angle = time * speed;
            let (sin, cos) = angle.sin_cos();
            let center = match index {
                1 => [-0.1, -0.1],
                2 => [0.25 * (-angle * 0.75).cos(), 0.25 * (-angle * 0.75).sin()],
                3 => [
                    0.05 + 0.25 * (-angle * 0.75).cos(),
                    0.05 + 0.25 * (-angle * 0.75).sin(),
                ],
                _ => [0., 0.],
            };
            uniforms.set_slot(index + 1, [center[0], center[1], cos, sin]);
        }
        uniforms.with_slot(5, [1.25, 0.8, 0.5, 0.25])
    }
}

/// Creates an animated backdrop that preserves the source image's broad color regions.
/// A stable ID caches the small image and retains artwork-change crossfades.
/// Supply elapsed seconds with [`ArtworkFlow::time`]; the element does not run a timer.
pub fn artwork_flow(id: impl Into<ElementId>, source: impl Into<ImageSource>) -> ArtworkFlow {
    ArtworkFlow {
        id: id.into(),
        source: source.into(),
        options: ArtworkFlowOptions::default(),
        time: 0.,
        downsample: 1,
        adaptive_layout: false,
        style: StyleRefinement::default(),
    }
}

/// Styled artwork layers with cached image preprocessing and paint-time transforms.
pub struct ArtworkFlow {
    id: ElementId,
    source: ImageSource,
    options: ArtworkFlowOptions,
    time: f32,
    downsample: u32,
    adaptive_layout: bool,
    style: StyleRefinement,
}

impl ArtworkFlow {
    /// Replaces the appearance parameters.
    pub fn options(mut self, options: ArtworkFlowOptions) -> Self {
        self.options = options;
        self
    }

    /// Distributes layers over both viewport axes, with a mirrored full-coverage
    /// base and feathered overlays. Useful for portrait and changing aspect ratios.
    /// Defaults to false, which preserves the width-based composition.
    pub fn adaptive_layout(mut self, enabled: bool) -> Self {
        self.adaptive_layout = enabled;
        self
    }

    /// Reduces the source and blur-pass resolution by a divisor clamped to `1..=8`.
    /// Color adjustment, dithering and rounded edges remain at native resolution.
    /// Layout and image-change crossfades are unchanged. Defaults to one.
    pub fn downsample(mut self, divisor: u32) -> Self {
        self.downsample = divisor.clamp(1, 8);
        self
    }

    /// Sets elapsed animation seconds. Keep this value fixed to pause the layers.
    pub fn time(mut self, seconds: f32) -> Self {
        self.time = seconds;
        self
    }
}

impl Styled for ArtworkFlow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for ArtworkFlow {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

struct ArtworkState {
    source_id: Option<ImageId>,
    current: Option<Arc<RenderImage>>,
    previous: Option<Arc<RenderImage>>,
    changed_at: Instant,
}

impl Default for ArtworkState {
    fn default() -> Self {
        Self {
            source_id: None,
            current: None,
            previous: None,
            changed_at: Instant::now(),
        }
    }
}

impl ArtworkState {
    fn blend(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.changed_at).as_secs_f32() / CROSSFADE_SECONDS)
            .min(1.);
        t
    }

    fn update(&mut self, source: &Arc<RenderImage>, now: Instant) {
        if self.source_id == Some(source.id) {
            return;
        }
        let Some(prepared) = prepare_artwork(source) else {
            return;
        };
        self.previous = self.current.take().map(|current| {
            if let Some(previous) = &self.previous {
                mix_artwork(previous, &current, self.blend(now))
            } else {
                current
            }
        });
        self.current = Some(prepared);
        self.source_id = Some(source.id);
        self.changed_at = now;
    }
}

fn prepare_artwork(source: &RenderImage) -> Option<Arc<RenderImage>> {
    let bytes = source.as_bytes(0)?;
    let size = source.size(0);
    let (width, height) = (size.width.0 as usize, size.height.0 as usize);
    if width == 0 || height == 0 {
        return None;
    }
    let image = image::RgbaImage::from_fn(THUMBNAIL_SIZE, THUMBNAIL_SIZE, |x, y| {
        let x = x as f32 / (THUMBNAIL_SIZE - 1) as f32 * (width - 1) as f32;
        let y = y as f32 / (THUMBNAIL_SIZE - 1) as f32 * (height - 1) as f32;
        let (left, top) = (x.floor() as usize, y.floor() as usize);
        let (right, bottom) = ((left + 1).min(width - 1), (top + 1).min(height - 1));
        let read = |x, y, channel| {
            let index = (y * width + x) * 4;
            let value = bytes[index + channel] as f32;
            if channel == 3 {
                value
            } else {
                value * bytes[index + 3] as f32 / 255.
            }
        };
        image::Rgba(std::array::from_fn(|channel| {
            let upper = read(left, top, channel)
                + (read(right, top, channel) - read(left, top, channel)) * x.fract();
            let lower = read(left, bottom, channel)
                + (read(right, bottom, channel) - read(left, bottom, channel)) * x.fract();
            (upper + (lower - upper) * y.fract()).round() as u8
        }))
    });
    let mut thumbnail = image;
    for pixel in thumbnail.pixels_mut() {
        let alpha = pixel[3] as f32;
        for channel in 0..3 {
            pixel[channel] = if alpha > 0. {
                (pixel[channel] as f32 * 255. / alpha).round() as u8
            } else {
                0
            };
        }
    }
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(thumbnail)
    ])))
}

fn mix_artwork(previous: &RenderImage, current: &RenderImage, blend: f32) -> Arc<RenderImage> {
    let previous = previous.as_bytes(0).unwrap();
    let current = current.as_bytes(0).unwrap();
    let pixels = previous
        .chunks_exact(4)
        .zip(current.chunks_exact(4))
        .flat_map(|(a, b)| {
            let alpha = a[3] as f32 * (1. - blend) + b[3] as f32 * blend;
            std::array::from_fn::<_, 4, _>(|channel| {
                if channel == 3 {
                    alpha.round() as u8
                } else if alpha > 0. {
                    ((a[channel] as f32 * a[3] as f32 * (1. - blend)
                        + b[channel] as f32 * b[3] as f32 * blend)
                        / alpha)
                        .round() as u8
                } else {
                    0
                }
            })
        })
        .collect();
    let image = image::RgbaImage::from_raw(THUMBNAIL_SIZE, THUMBNAIL_SIZE, pixels).unwrap();
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image
    )]))
}

impl Element for ArtworkFlow {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Style) {
        let _ = self.source.use_data(None, window, cx);
        let mut style = Style::default();
        style.refine(&self.style);
        (window.request_layout(style.clone(), [], cx), style)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Style,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let source = self.source.use_data(None, window, cx).and_then(Result::ok);
        let now = Instant::now();
        let images = window.with_element_state(
            id.expect("artwork flow requires an ID"),
            |state: Option<ArtworkState>, _| {
                let mut state = state.unwrap_or_default();
                if let Some(source) = source {
                    state.update(&source, now);
                }
                let blend = state.blend(now);
                if blend >= 1. {
                    state.previous = None;
                }
                let images = state.current.clone().map(|current| {
                    let previous = state.previous.clone().unwrap_or_else(|| current.clone());
                    let blend = if state.previous.is_some() { blend } else { 1. };
                    (current, previous, blend)
                });
                (images, state)
            },
        );
        let Some((current, previous, blend)) = images else {
            return;
        };
        if blend < 1. && window.is_window_active() {
            window.request_animation_frame();
        }
        let radii = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let corners = [
            f32::from(radii.top_left),
            f32::from(radii.top_right),
            f32::from(radii.bottom_right),
            f32::from(radii.bottom_left),
        ]
        .map(|r| r * window.raster_scale_factor());
        let options = self.options;
        let time = self.time;
        let downsample = self.downsample;
        let adaptive = if self.adaptive_layout { 1. } else { 0. };
        let paint_source = move |window: &mut Window| {
            let effect = PaintEffect::new(bounds, artwork_flow_shader())
                .image(current, 0)
                .second_image(previous, 0)
                .uniforms(options.uniforms(time, blend).with_slot(
                    6,
                    [900. * window.raster_scale_factor(), -3.25, adaptive, 0.],
                ));
            let _ = window.paint_effect(effect);
        };
        style.paint(bounds, window, cx, move |window, _| {
            if downsample > 1 {
                let color = color_pass(options, corners);
                window.with_subtree_effect(
                    bounds,
                    color.shader,
                    color.uniforms,
                    0.,
                    1.,
                    |window| {
                        window.with_subtree_raster_scale(1. / downsample as f32, |window| {
                            let passes = blur_passes(window.raster_scale_factor());
                            window.with_subtree_effect_chain(bounds, &passes, 1., paint_source);
                        });
                    },
                );
            } else {
                let passes = processing_passes(options, window.raster_scale_factor(), corners);
                window.with_subtree_effect_chain(bounds, &passes, 1., paint_source);
            }
        });
    }
}

/// Image-layer compositing shader; the second image is the outgoing artwork.
pub fn artwork_flow_shader() -> EffectShader {
    EffectShader::wgsl_two_images(include_str!("shaders/artwork_flow.wgsl"))
}

fn processing_passes(
    options: ArtworkFlowOptions,
    scale: f32,
    corners: [f32; 4],
) -> Vec<gpui::SubtreeEffectPass> {
    let mut passes = blur_passes(scale);
    passes.push(color_pass(options, corners));
    passes
}

fn blur_passes(scale: f32) -> Vec<gpui::SubtreeEffectPass> {
    let mut passes = Vec::with_capacity(9);
    passes.extend(
        [5.5, 10.5, 20.5, 10.5, 40.5, 20.5, 80.5, 40.5]
            .into_iter()
            .map(|offset| gpui::SubtreeEffectPass {
                shader: artwork_flow_blur_shader(),
                uniforms: EffectUniforms::new().with_slot(0, [offset * scale, 0., 0., 0.]),
                time: 0.,
                images: Default::default(),
                bloom: None,
                feedback: None,
                distance_field: None,
                particles: None,
                particle_transition: None,
            }),
    );
    passes
}

fn color_pass(options: ArtworkFlowOptions, corners: [f32; 4]) -> gpui::SubtreeEffectPass {
    gpui::SubtreeEffectPass {
        shader: artwork_flow_color_shader(),
        uniforms: EffectUniforms::new()
            .with_slot(
                0,
                [
                    options.brightness.clamp(0., 1.),
                    options.saturation.max(0.),
                    options.contrast.max(0.),
                    options.dither.clamp(0., 2.),
                ],
            )
            .with_slot(1, corners),
        time: 0.,
        images: Default::default(),
        bloom: None,
        feedback: None,
        distance_field: None,
        particles: None,
        particle_transition: None,
    }
}

/// Four-corner Kawase sampling over the previously composited artwork.
pub fn artwork_flow_blur_shader() -> EffectShader {
    EffectShader::wgsl_image(include_str!("shaders/artwork_flow_blur.wgsl"))
}

/// Final color adjustment, backdrop overlays and corner clipping.
pub fn artwork_flow_color_shader() -> EffectShader {
    EffectShader::wgsl_image(include_str!("shaders/artwork_flow_color.wgsl"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pixel: impl Fn(u32, u32) -> image::Rgba<u8>) -> Arc<RenderImage> {
        Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image::RgbaImage::from_fn(16, 16, pixel)
        )]))
    }

    #[test]
    fn changing_artwork_retains_the_visible_mix_and_ignores_hidden_rgb() {
        let source = |hidden| {
            image(|x, _| {
                if x < 8 {
                    image::Rgba([20, 140, 240, 255])
                } else {
                    image::Rgba([hidden, 0, 0, 0])
                }
            })
        };
        assert_eq!(
            prepare_artwork(&source(0)).unwrap().as_bytes(0),
            prepare_artwork(&source(255)).unwrap().as_bytes(0)
        );

        let now = Instant::now();
        let first = source(0);
        let second = image(|_, _| image::Rgba([220, 130, 10, 255]));
        let third = image(|_, _| image::Rgba([80, 210, 50, 255]));
        let mut state = ArtworkState::default();
        state.update(&first, now);
        let cached = state.current.clone().unwrap();
        state.update(&first, now + std::time::Duration::from_secs(2));
        assert!(Arc::ptr_eq(&cached, state.current.as_ref().unwrap()));
        state.update(&second, now);
        let mid = now + std::time::Duration::from_secs_f32(CROSSFADE_SECONDS / 2.);
        let visible = mix_artwork(
            state.previous.as_ref().unwrap(),
            state.current.as_ref().unwrap(),
            state.blend(mid),
        );
        state.update(&third, mid);
        assert_eq!(
            state.previous.as_ref().unwrap().as_bytes(0),
            visible.as_bytes(0)
        );
        assert_eq!(state.blend(mid), 0.);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn gpu_layers_animate_pause_and_crossfade_between_artworks() -> anyhow::Result<()> {
        use gpui::{
            ContentMask, DevicePixels, EffectQuad, RenderImageParams, ScaledPixels, Scene, point,
            size,
        };
        use gpui_wgpu::WgpuOffscreenRenderer;
        use std::borrow::Cow;

        let coverage_shader = EffectShader::wgsl_two_images(format!(
            "{}\nfn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {{ let alpha = flow_alpha(input, params).a; return vec4<f32>(vec3<f32>(alpha), 1.0); }}",
            include_str!("shaders/artwork_flow.wgsl").replace("fn effect(", "fn flow_alpha("),
        ));
        for (width, height, adaptive) in [
            (64, 96, false),
            (96, 64, false),
            (72, 168, false),
            (72, 168, true),
            (168, 72, true),
            (96, 96, true),
        ] {
            let mut renderer =
                WgpuOffscreenRenderer::new(size(DevicePixels(width), DevicePixels(height)))?;
            let current = prepare_artwork(&image(|x, y| {
                image::Rgba([
                    if x < 8 { 240 } else { 20 },
                    (y * 14) as u8,
                    if x < 8 { 10 } else { 220 },
                    255,
                ])
            }))
            .unwrap();
            let previous =
                prepare_artwork(&image(|_, _| image::Rgba([210, 120, 30, 255]))).unwrap();
            let upload = |image: &RenderImage| {
                renderer
                    .sprite_atlas()
                    .get_or_insert_with(
                        &RenderImageParams {
                            image_id: image.id,
                            frame_index: 0,
                        }
                        .into(),
                        &mut || {
                            Ok(Some((
                                image.size(0),
                                Cow::Borrowed(image.as_bytes(0).unwrap()),
                            )))
                        },
                    )
                    .map(|tile| tile.unwrap())
            };
            let current = upload(&current)?;
            let previous = upload(&previous)?;
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(width as f32), ScaledPixels(height as f32)),
            );
            let layer = |content, bounds, mut passes: Vec<gpui::SubtreeEffectPass>| {
                let last = passes.pop().unwrap();
                gpui::Primitive::SubtreeLayer(gpui::SubtreeLayer {
                    scene: std::rc::Rc::new(content),
                    second_scene: None,
                    scene3d: None,
                    intermediate_effects: passes.into(),
                    composite: EffectQuad {
                        order: 0,
                        bounds,
                        effect_bounds: bounds,
                        content_mask: ContentMask { bounds },
                        transformation: Default::default(),
                        corner_radii: Default::default(),
                        shader: last.shader,
                        uniforms: last.uniforms,
                        time: 0.,
                        opacity: 1.,
                        image_tile: None,
                        second_image_tile: None,
                        third_image_tile: None,
                        fourth_image_tile: None,
                    },
                })
            };
            let mut render = |time, blend, motion, downsample: u32| {
                let density = 1. / downsample as f32;
                let source_bounds = bounds.map(|value| ScaledPixels(value.0 * density));
                let mut content = Scene::default();
                content.insert_primitive(EffectQuad {
                    order: 0,
                    bounds: source_bounds,
                    effect_bounds: source_bounds,
                    content_mask: ContentMask {
                        bounds: source_bounds,
                    },
                    transformation: Default::default(),
                    corner_radii: Default::default(),
                    shader: artwork_flow_shader(),
                    uniforms: ArtworkFlowOptions::default()
                        .motion(motion)
                        .uniforms(time, blend)
                        .with_slot(
                            6,
                            [90. * density, -3.25, if adaptive { 1. } else { 0. }, 0.],
                        ),
                    time: 0.,
                    opacity: 1.,
                    image_tile: Some(current),
                    second_image_tile: Some(previous),
                    third_image_tile: None,
                    fourth_image_tile: None,
                });
                content.finish();
                if adaptive && downsample == 1 {
                    content.effects[0].shader = coverage_shader.clone();
                    let coverage = renderer.render_rgba(&content)?;
                    assert!(
                        coverage
                            .chunks_exact(4)
                            .all(|pixel| pixel[..3].iter().all(|v| *v >= 254)),
                        "adaptive layers must cover the viewport before blur at {width}x{height}, time {time}"
                    );
                    content.effects[0].shader = artwork_flow_shader();
                }
                let mut scene = Scene::default();
                if downsample > 1 {
                    let mut reduced = Scene::default();
                    reduced.raster_scale = Some(density);
                    reduced.insert_primitive(layer(
                        content,
                        source_bounds,
                        blur_passes(0.1 * density),
                    ));
                    reduced.finish();
                    scene.insert_primitive(layer(
                        reduced,
                        bounds,
                        vec![color_pass(ArtworkFlowOptions::default(), [0.; 4])],
                    ));
                } else {
                    scene.insert_primitive(layer(
                        content,
                        bounds,
                        processing_passes(ArtworkFlowOptions::default(), 0.1, [0.; 4]),
                    ));
                }
                scene.finish();
                renderer.render_rgba(&scene)
            };
            let first = render(0., 1., 1., 1)?;
            let later = render(20., 1., 1., 1)?;
            assert!(
                first
                    .iter()
                    .zip(&later)
                    .filter(|(a, b)| a.abs_diff(**b) > 5)
                    .count()
                    > first.len() / 10
            );
            assert_eq!(render(0., 1., 0., 1)?, render(20., 1., 0., 1)?);
            let outgoing = render(0., 0., 1., 1)?;
            if adaptive || width > height {
                for channel in 0..3 {
                    let values = outgoing.chunks_exact(4).map(|pixel| pixel[channel]);
                    let minimum = values.clone().min().unwrap();
                    let maximum = values.max().unwrap();
                    assert!(
                        maximum - minimum <= 1,
                        "Kawase passes must preserve an opaque constant field at the edges: channel {channel}, {minimum}..{maximum}"
                    );
                }
            }
            if adaptive {
                for time in [20., 90.] {
                    let moved = render(time, 0., 1., 1)?;
                    assert!(
                        moved
                            .iter()
                            .zip(&outgoing)
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "rotating a constant opaque field must not reveal viewport edges"
                    );
                }
            }
            let middle = render(0., 0.5, 1., 1)?;
            assert_ne!(outgoing, first);
            assert_ne!(middle, outgoing);
            assert_ne!(middle, first);
            assert!(first.chunks_exact(4).all(|pixel| pixel[3] == 255));
            for divisor in [2, 4, 8] {
                let reduced = render(0., 1., 1., divisor)?;
                assert!(reduced.chunks_exact(4).all(|pixel| pixel[3] == 255));
                let error = reduced
                    .iter()
                    .zip(&first)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum::<u64>() as f64
                    / first.len() as f64;
                let tolerance = if divisor <= 4 { 4. } else { 12. };
                assert!(
                    error < tolerance,
                    "downsample {divisor}: mean channel error {error}"
                );
                assert_eq!(render(0., 1., 0., divisor)?, render(20., 1., 0., divisor)?);
                let mixed = render(0., 0.5, 1., divisor)?;
                assert_ne!(mixed, reduced);
                assert_ne!(mixed, render(0., 0., 1., divisor)?);
            }
        }
        Ok(())
    }
}
