use gpui::{BackdropShader, Div, EffectUniforms, PaintBackdropEffect, Pixels, Rgba, div, px, rgba};

/// Optical controls for a static ribbed glass surface.
#[derive(Clone, Copy, Debug)]
pub struct FlutedGlassOptions {
    /// Distance between ribs in logical pixels, clamped to 4..=128.
    pub spacing: Pixels,
    /// Rib direction clockwise from vertical, in degrees.
    pub angle: f32,
    /// Maximum sampling displacement in logical pixels, limited to 45% of the spacing.
    /// Zero also disables rib lighting, leaving flat glass.
    pub refraction: Pixels,
    /// Background blur radius in logical pixels, clamped to 0..=64.
    pub blur_radius: Pixels,
    /// Sharp background contribution in 0..=1, reduced where the ribs compress detail.
    pub clarity: f32,
    /// Color wash. Alpha controls its contribution to the sampled background.
    pub tint: Rgba,
    /// Rib reflection strength in 0..=1.
    pub highlight: f32,
}

impl Default for FlutedGlassOptions {
    fn default() -> Self {
        Self {
            spacing: px(24.),
            angle: 0.,
            refraction: px(6.),
            blur_radius: px(3.),
            clarity: 0.8,
            tint: rgba(0xffffff12),
            highlight: 0.12,
        }
    }
}

fn finite(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

impl FlutedGlassOptions {
    /// Packs optical parameters using the current raster scale.
    pub fn uniforms(self, scale: f32) -> EffectUniforms {
        let scale = if scale.is_finite() && scale > 0. {
            scale
        } else {
            1.
        };
        let spacing = finite(f32::from(self.spacing), 4., 128., 24.);
        let angle = if self.angle.is_finite() {
            self.angle % 360.
        } else {
            0.
        };
        let (sin, cos) = angle.to_radians().sin_cos();
        EffectUniforms::new()
            .with_slot(
                0,
                [
                    spacing * scale,
                    finite(f32::from(self.refraction), 0., spacing * 0.45, 0.) * scale,
                    cos,
                    sin,
                ],
            )
            .with_slot(
                1,
                [
                    finite(self.clarity, 0., 1., 0.),
                    finite(self.highlight, 0., 1., 0.),
                    0.,
                    0.,
                ],
            )
            .with_slot(
                2,
                [self.tint.r, self.tint.g, self.tint.b, self.tint.a].map(|v| finite(v, 0., 1., 0.)),
            )
    }
}

/// A styled ribbed glass container sampling previously painted window content.
/// Children remain sharp. Layout, corners, borders and interaction use the normal Div APIs.
/// The material does not schedule animation frames.
pub fn fluted_glass(options: FlutedGlassOptions) -> Div {
    div().on_paint_before_children(move |bounds, style, window, _| {
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        window.paint_backdrop_effect(
            PaintBackdropEffect::new(
                bounds,
                px(finite(f32::from(options.blur_radius), 0., 64., 0.)),
                fluted_glass_shader(),
            )
            .corner_radii(corners)
            .uniforms(options.uniforms(window.raster_scale_factor())),
        );
    })
}

/// Slot 0: spacing, refraction in device pixels, direction cosine/sine;
/// slot 1: clarity, highlight, unused, unused; slot 2: tint RGBA.
pub fn fluted_glass_shader() -> BackdropShader {
    BackdropShader::wgsl(include_str!("shaders/fluted_glass.wgsl"))
}
