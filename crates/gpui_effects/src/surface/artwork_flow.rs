use std::{sync::Arc, time::Instant};

use gpui::{
    App, Bounds, EffectShader, EffectUniforms, Element, ElementId, GlobalElementId, ImageId,
    ImageSource, InspectorElementId, IntoElement, LayoutId, PaintEffect, Pixels, RenderImage,
    Style, StyleRefinement, Styled, Window, prelude::*,
};

const THUMBNAIL_SIZE: u32 = 48;
const CROSSFADE_SECONDS: f32 = 0.7;

/// Appearance of a softly layered, image-derived background.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct ArtworkFlowOptions {
    /// Display brightness in `0..=1`.
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
            brightness: 0.38,
            saturation: 1.8,
            contrast: 1.5,
            motion: 1.,
            dither: 1.,
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
        for (index, (speed, phase, radius)) in [
            (0.037, 0.35, 0.),
            (-0.061, 2.1, 0.09),
            (0.049, 4.4, 0.23),
            (-0.031, 1.3, 0.3),
        ]
        .into_iter()
        .enumerate()
        {
            let angle = time * speed + phase;
            let (sin, cos) = angle.sin_cos();
            let orbit = time * speed * 0.67 + phase;
            uniforms.set_slot(
                index + 1,
                [
                    radius * orbit.cos(),
                    radius * (orbit * 0.83).sin(),
                    cos,
                    sin,
                ],
            );
        }
        uniforms
            .with_slot(5, [1.9, 1.25, 0.9, 0.65])
            .with_slot(6, [0.38, self.dither.clamp(0., 2.), 0., 0.])
    }
}

/// Creates an animated backdrop that preserves the source image's broad color regions.
/// A stable ID caches the small blurred image and retains artwork-change crossfades.
/// Supply elapsed seconds with [`ArtworkFlow::time`]; the element does not run a timer.
pub fn artwork_flow(id: impl Into<ElementId>, source: impl Into<ImageSource>) -> ArtworkFlow {
    ArtworkFlow {
        id: id.into(),
        source: source.into(),
        options: ArtworkFlowOptions::default(),
        time: 0.,
        style: StyleRefinement::default(),
    }
}

/// Styled artwork layers with cached image preprocessing and paint-time transforms.
pub struct ArtworkFlow {
    id: ElementId,
    source: ImageSource,
    options: ArtworkFlowOptions,
    time: f32,
    style: StyleRefinement,
}

impl ArtworkFlow {
    /// Replaces the appearance parameters.
    pub fn options(mut self, options: ArtworkFlowOptions) -> Self {
        self.options = options;
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
        t * t * (3. - 2. * t)
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
    // Filter premultiplied BGRA so transparent texels cannot introduce color halos.
    let mut blurred = image::imageops::blur(&image, 4.5);
    for pixel in blurred.pixels_mut() {
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
        image::Frame::new(blurred)
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
        let effect = PaintEffect::new(bounds, artwork_flow_shader())
            .image(current, 0)
            .second_image(previous, 0)
            .uniforms(self.options.uniforms(self.time, blend))
            .corner_radii(radii);
        style.paint(bounds, window, cx, move |window, _| {
            let _ = window.paint_effect(effect);
        });
    }
}

/// Image-layer compositing shader; the second image is the outgoing artwork.
pub fn artwork_flow_shader() -> EffectShader {
    EffectShader::wgsl_two_images(include_str!("shaders/artwork_flow.wgsl"))
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

        let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(64), DevicePixels(96)))?;
        let current = prepare_artwork(&image(|x, y| {
            image::Rgba([
                if x < 8 { 240 } else { 20 },
                (y * 14) as u8,
                if x < 8 { 10 } else { 220 },
                255,
            ])
        }))
        .unwrap();
        let previous = prepare_artwork(&image(|_, _| image::Rgba([210, 120, 30, 255]))).unwrap();
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
            size(ScaledPixels(64.), ScaledPixels(96.)),
        );
        let mut render = |time, blend, motion| {
            let mut scene = Scene::default();
            scene.insert_primitive(EffectQuad {
                order: 0,
                bounds,
                effect_bounds: bounds,
                content_mask: ContentMask { bounds },
                transformation: Default::default(),
                corner_radii: Default::default(),
                shader: artwork_flow_shader(),
                uniforms: ArtworkFlowOptions::default()
                    .motion(motion)
                    .uniforms(time, blend),
                time: 0.,
                opacity: 1.,
                image_tile: Some(current),
                second_image_tile: Some(previous),
                third_image_tile: None,
                fourth_image_tile: None,
            });
            scene.finish();
            renderer.render_rgba(&scene)
        };
        let first = render(0., 1., 1.)?;
        let later = render(20., 1., 1.)?;
        assert!(
            first
                .iter()
                .zip(&later)
                .filter(|(a, b)| a.abs_diff(**b) > 5)
                .count()
                > first.len() / 10
        );
        assert_eq!(render(0., 1., 0.)?, render(20., 1., 0.)?);
        let outgoing = render(0., 0., 1.)?;
        let middle = render(0., 0.5, 1.)?;
        assert_ne!(outgoing, first);
        assert_ne!(middle, outgoing);
        assert_ne!(middle, first);
        assert!(first.chunks_exact(4).all(|pixel| pixel[3] == 255));
        Ok(())
    }
}
