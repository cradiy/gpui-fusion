use gpui::{Div, EffectShader, EffectUniforms, PaintEffect, Pixels, Rgba, Size, div, px, rgb};

/// A soft light band moving across a styled surface.
#[derive(Clone, Copy, Debug)]
pub struct LightSweepOptions {
    /// Sweep progress in 0..=1. Both endpoints leave the surface unlit.
    pub progress: f32,
    /// Travel direction in degrees, clockwise from rightward motion.
    pub angle: f32,
    /// Full band width in logical pixels, perpendicular to the band.
    pub width: Pixels,
    pub color: Rgba,
    pub opacity: f32,
}

impl Default for LightSweepOptions {
    fn default() -> Self {
        Self {
            progress: 0.,
            angle: 20.,
            width: px(100.),
            color: rgb(0xffffff),
            opacity: 0.18,
        }
    }
}

impl LightSweepOptions {
    /// Parameters for [`light_sweep_shader`], using logical geometry and raster scale.
    pub fn uniforms(self, size: Size<Pixels>, scale: f32) -> EffectUniforms {
        let valid = self.progress.is_finite()
            && self.angle.is_finite()
            && f32::from(self.width).is_finite()
            && self.width > px(0.)
            && self.opacity.is_finite();
        if !valid {
            return EffectUniforms::new();
        }
        let (sin, cos) = (self.angle % 360.).to_radians().sin_cos();
        let half_width = f32::from(self.width) * scale * 0.5;
        let extent =
            (cos.abs() * f32::from(size.width) + sin.abs() * f32::from(size.height)) * scale * 0.5;
        let progress = self.progress.clamp(0., 1.);
        let center = (progress * 2. - 1.) * (extent + half_width);
        let opacity = if progress > 0. && progress < 1. {
            self.opacity.clamp(0., 1.)
        } else {
            0.
        };
        EffectUniforms::new()
            .with_slot(0, [cos, sin, center, half_width])
            .with_slot(1, [self.color.r, self.color.g, self.color.b, self.color.a])
            .with_slot(2, [opacity, 0., 0., 0.])
    }
}

/// A styled container with a light sweep beneath its children.
/// The caller animates `progress`; no animation frames or subtree captures are scheduled.
pub fn light_sweep(options: LightSweepOptions) -> Div {
    div().on_paint_before_children(move |bounds, style, window, _| {
        let uniforms = options.uniforms(bounds.size, window.raster_scale_factor());
        if uniforms.slots()[2][0] <= 0. {
            return;
        }
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let _ = window.paint_effect(
            PaintEffect::new(bounds, light_sweep_shader())
                .uniforms(uniforms)
                .corner_radii(corners),
        );
    })
}

/// Slot 0: travel direction.xy, band center and half-width in device pixels;
/// slot 1: light RGBA; slot 2.x: opacity.
pub fn light_sweep_shader() -> EffectShader {
    EffectShader::wgsl(include_str!("shaders/light_sweep.wgsl"))
}
