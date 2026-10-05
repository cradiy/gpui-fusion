use gpui::{EffectShader, IntoElement, Pixels, PointerTransform, px};

use crate::{EffectStage, SubtreeEffect, subtree_effect_chain};

/// Square-cell sampling for artwork and painted subtrees.
#[derive(Clone, Copy, Debug)]
pub struct PixelateOptions {
    /// Cell width and height in logical pixels, clamped to 1..=128.
    /// Values at or below one bypass the filter.
    pub cell_size: Pixels,
    /// Blend from the original at zero to the pixelated image at one.
    pub strength: f32,
}

impl Default for PixelateOptions {
    fn default() -> Self {
        Self {
            cell_size: px(16.),
            strength: 1.,
        }
    }
}

/// Samples each cell at its center without changing layout or pointer geometry.
pub fn subtree_pixelate<E: IntoElement>(
    element: E,
    options: PixelateOptions,
) -> SubtreeEffect<E::Element> {
    subtree_effect_chain(element, [EffectStage::pixelate(options)])
}

impl EffectStage {
    /// Adds square-cell sampling. Zero strength or non-finite settings bypass capture.
    pub fn pixelate(options: PixelateOptions) -> Self {
        let cell = f32::from(options.cell_size);
        let valid = cell.is_finite() && options.strength.is_finite();
        let cell = if valid { cell.clamp(1., 128.) } else { 1. };
        let strength = if valid {
            options.strength.clamp(0., 1.)
        } else {
            0.
        };
        Self::new(pixelate_shader())
            .uniform_pixels(0, [px(cell), px(0.), px(0.), px(0.)])
            .uniform(1, [strength, 0., 0., 0.])
            .pointer_transform(PointerTransform::identity())
            .enabled(cell > 1. && strength > 0.)
    }
}

/// Slot 0.x: cell size in device pixels; slot 1.x: original-to-pixelated blend.
pub fn pixelate_shader() -> EffectShader {
    EffectShader::wgsl_image(include_str!("shaders/pixelate.wgsl"))
}
