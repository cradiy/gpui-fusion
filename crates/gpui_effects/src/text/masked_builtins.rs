use crate::{MaskedEffect, effect_svg, effect_text};
use gpui::{Div, EffectShader, Element, Rgba, SharedString, Svg};

fn color_slot(color: impl Into<Rgba>) -> [f32; 4] {
    let color = color.into();
    [color.r, color.g, color.b, color.a]
}

fn configure_spectrum<E, C>(effect: MaskedEffect<E>, colors: [C; 4]) -> MaskedEffect<E>
where
    E: Element,
    C: Copy + Into<Rgba>,
{
    effect
        .uniform(0, color_slot(colors[0]))
        .uniform(1, color_slot(colors[1]))
        .uniform(2, color_slot(colors[2]))
        .uniform(3, color_slot(colors[3]))
        .uniform(4, [1.0, 0.16, 0.08, 0.0])
}

/// Creates animated spectrum text with four customizable colors.
pub fn spectrum_text<C>(text: impl Into<SharedString>, colors: [C; 4]) -> MaskedEffect<Div>
where
    C: Copy + Into<Rgba>,
{
    configure_spectrum(effect_text(text, spectrum_mask_shader()), colors)
}

/// Creates an animated spectrum-filled monochrome SVG.
pub fn spectrum_svg<C>(path: impl Into<SharedString>, colors: [C; 4]) -> MaskedEffect<Svg>
where
    C: Copy + Into<Rgba>,
{
    configure_spectrum(effect_svg(path, spectrum_mask_shader()), colors)
}

/// Returns the portable mask shader used by [`spectrum_text`] and [`spectrum_svg`].
pub fn spectrum_mask_shader() -> EffectShader {
    EffectShader::wgsl_mask(SPECTRUM_MASK_WGSL)
}

const SPECTRUM_MASK_WGSL: &str = include_str!("shaders/spectrum_mask.wgsl");
