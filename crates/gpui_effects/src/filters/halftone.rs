use gpui::{EffectShader, IntoElement, Pixels, PointerTransform, Rgba, px, rgb};

use crate::{EffectStage, SubtreeEffect, subtree_effect_chain};

/// Monochrome print screening for painted content.
#[derive(Clone, Copy, Debug)]
pub struct HalftoneOptions {
    /// Distance between dot centers in logical pixels, clamped to 2..=64.
    pub spacing: Pixels,
    /// Clockwise screen angle in degrees.
    pub angle: f32,
    /// Blend with the source, from zero (original) to one (print).
    pub strength: f32,
    /// Color of the printed dots.
    pub ink: Rgba,
    /// Color between dots. Source transparency is retained.
    pub paper: Rgba,
}

impl Default for HalftoneOptions {
    fn default() -> Self {
        Self {
            spacing: px(6.),
            angle: 30.,
            strength: 1.,
            ink: rgb(0x243149),
            paper: rgb(0xf2e6cf),
        }
    }
}

/// Applies a static dot screen without changing layout or pointer coordinates.
pub fn subtree_halftone<E: IntoElement>(
    element: E,
    options: HalftoneOptions,
) -> SubtreeEffect<E::Element> {
    subtree_effect_chain(element, [EffectStage::halftone(options)])
}

impl EffectStage {
    /// Converts cell luminance into antialiased dots and blends with the source.
    /// Zero strength or non-finite geometry disables the stage.
    pub fn halftone(options: HalftoneOptions) -> Self {
        let spacing = f32::from(options.spacing);
        let valid =
            spacing.is_finite() && options.angle.is_finite() && options.strength.is_finite();
        let strength = if valid {
            options.strength.clamp(0., 1.)
        } else {
            0.
        };
        let angle = if valid { options.angle % 360. } else { 0. };
        let (sin, cos) = angle.to_radians().sin_cos();
        let color = |color: Rgba| {
            [color.r, color.g, color.b, color.a]
                .map(|v| if v.is_finite() { v.clamp(0., 1.) } else { 0. })
        };
        Self::new(halftone_shader())
            .uniform_pixels(
                0,
                [
                    px(if valid { spacing.clamp(2., 64.) } else { 2. }),
                    px(0.),
                    px(0.),
                    px(0.),
                ],
            )
            .uniform(1, [cos, sin, strength, 0.])
            .uniform(2, color(options.ink))
            .uniform(3, color(options.paper))
            .pointer_transform(PointerTransform::identity())
            .enabled(strength > 0.)
    }
}

/// Slot 0.x: dot spacing in device pixels; slot 1: cosine, sine, strength, unused;
/// slots 2 and 3: straight ink and paper RGBA.
pub fn halftone_shader() -> EffectShader {
    EffectShader::wgsl_image(include_str!("shaders/halftone.wgsl"))
}
