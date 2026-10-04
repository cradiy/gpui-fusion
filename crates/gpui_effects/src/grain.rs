use gpui::{Div, EffectShader, EffectUniforms, PaintEffect, Pixels, div, px};

/// Static surface texture painted beneath a container's children.
#[derive(Clone, Copy, Debug)]
pub struct GrainOptions {
    /// Grain cell size in logical pixels, clamped to 0.5..=8 and at least one device pixel.
    pub size: Pixels,
    /// Maximum overlay opacity, clamped to 0..=1. Zero disables painting.
    pub strength: f32,
    /// Uses independent color channels instead of monochrome grain.
    pub colored: bool,
}

impl Default for GrainOptions {
    fn default() -> Self {
        Self {
            size: px(0.75),
            strength: 0.12,
            colored: false,
        }
    }
}

impl GrainOptions {
    /// Parameters for [`grain_shader`], using the current raster scale.
    pub fn uniforms(self, scale: f32) -> EffectUniforms {
        let size = f32::from(self.size);
        if !size.is_finite() || !self.strength.is_finite() || !scale.is_finite() || scale <= 0. {
            return EffectUniforms::new();
        }
        EffectUniforms::new().with_slot(
            0,
            [
                (size.clamp(0.5, 8.) * scale).max(1.),
                self.strength.clamp(0., 1.),
                f32::from(self.colored),
                0.,
            ],
        )
    }
}

/// A styled container with static grain above its background and beneath its children.
/// Grain follows the resolved corner radii and does not capture content or schedule frames.
pub fn surface_grain(options: GrainOptions) -> Div {
    div().on_paint_before_children(move |bounds, style, window, _| {
        let uniforms = options.uniforms(window.raster_scale_factor());
        if uniforms.slots()[0][1] <= 0. {
            return;
        }
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let _ = window.paint_effect(
            PaintEffect::new(bounds, grain_shader())
                .uniforms(uniforms)
                .corner_radii(corners),
        );
    })
}

/// Slot 0: cell size in device pixels, strength, colored flag, unused.
pub fn grain_shader() -> EffectShader {
    EffectShader::wgsl(include_str!("shaders/grain.wgsl"))
}
