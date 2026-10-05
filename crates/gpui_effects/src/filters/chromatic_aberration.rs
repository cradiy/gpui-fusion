use gpui::{EffectShader, IntoElement, Pixels, PointerTransform, px};

use crate::{EffectStage, SubtreeEffect, subtree_effect_chain};

/// Geometry of the red and blue channel offsets.
#[derive(Clone, Copy, Debug, Default)]
pub enum ChromaticAberrationMode {
    /// Offset grows quadratically from zero at the center to `amount` at the corners.
    #[default]
    Radial,
    /// Constant offset along an angle in clockwise degrees from the horizontal.
    Directional { angle: f32 },
}

/// Static color separation with unchanged source coverage.
#[derive(Clone, Copy, Debug)]
pub struct ChromaticAberrationOptions {
    /// Maximum offset of each outer channel in logical pixels, clamped to 0..=32.
    pub amount: Pixels,
    pub mode: ChromaticAberrationMode,
}

impl Default for ChromaticAberrationOptions {
    fn default() -> Self {
        Self {
            amount: px(2.),
            mode: ChromaticAberrationMode::Radial,
        }
    }
}

/// Separates red and blue samples while retaining layout, input geometry and alpha.
pub fn subtree_chromatic_aberration<E: IntoElement>(
    element: E,
    options: ChromaticAberrationOptions,
) -> SubtreeEffect<E::Element> {
    subtree_effect_chain(element, [EffectStage::chromatic_aberration(options)])
}

impl EffectStage {
    /// Adds a three-sample color separation pass. Zero or invalid settings bypass it.
    pub fn chromatic_aberration(options: ChromaticAberrationOptions) -> Self {
        let (radial, angle) = match options.mode {
            ChromaticAberrationMode::Radial => (1., 0.),
            ChromaticAberrationMode::Directional { angle } => (0., angle),
        };
        let amount = f32::from(options.amount);
        let valid = amount.is_finite() && angle.is_finite();
        let amount = if valid { amount.clamp(0., 32.) } else { 0. };
        let (sin, cos) = if valid { angle % 360. } else { 0. }.to_radians().sin_cos();
        Self::new(chromatic_aberration_shader())
            .uniform_pixels(0, [px(amount), px(0.), px(0.), px(0.)])
            .uniform(1, [cos, sin, radial, 0.])
            .pointer_transform(PointerTransform::identity())
            .enabled(amount > 0.)
    }
}

/// Slot 0.x: channel offset in device pixels; slot 1: cosine, sine, radial flag, unused.
pub fn chromatic_aberration_shader() -> EffectShader {
    EffectShader::wgsl_image(include_str!("shaders/chromatic_aberration.wgsl"))
}
