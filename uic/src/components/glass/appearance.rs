use gpui::{Hsla, hsla, px, rgb};
use gpui_effects::LiquidGlassStyle;

/// Optical surfaces and semantic colors for a glass segmented control.
/// Layout, typography, outer corners, background, and shadow use `Styled`.
#[derive(Clone, Copy, Debug)]
pub struct GlassSegmentedAppearance {
    pub surface: LiquidGlassStyle,
    pub selection: LiquidGlassStyle,
    /// Color of monochrome content covered by the animated selection surface.
    /// Alpha multiplies the content's original alpha; uncovered pixels keep their color.
    pub selected_text: Hsla,
    pub disabled_opacity: f32,
}

impl GlassSegmentedAppearance {
    pub fn light() -> Self {
        let mut surface = LiquidGlassStyle::optical()
            .blur_radius(px(1.))
            .refraction(px(24.))
            .thickness(px(12.))
            .dispersion(0.04)
            .tint(hsla(0., 0., 1., 0.015));
        surface.shape.roundness = 2.;
        surface.shadow.spread = px(6.);
        surface.shadow.strength = 0.05;
        surface.shadow.offset = gpui::point(px(0.), px(2.));
        surface.optics.fresnel_strength = 0.18;
        surface.optics.glare_strength = 0.65;
        let mut selection = LiquidGlassStyle::optical()
            .blur_radius(px(0.))
            .refraction(px(8.))
            .thickness(px(16.))
            .dispersion(0.02);
        selection.shape.roundness = 2.;
        selection.shadow.strength = 0.;
        selection.optics.fresnel_strength = 0.12;
        selection.optics.glare_strength = 0.35;
        Self {
            surface,
            selection,
            selected_text: rgb(0x007aff).into(),
            disabled_opacity: 0.45,
        }
    }

    /// Pair with a light foreground color through `Styled::text_color`.
    pub fn dark() -> Self {
        let mut material = Self::light();
        material.surface.tint = hsla(0.61, 0.12, 0.10, 0.04);
        material.selection.tint = hsla(0., 0., 1., 0.025);
        material.selected_text = rgb(0x64b5ff).into();
        material
    }
}

impl Default for GlassSegmentedAppearance {
    fn default() -> Self {
        Self::light()
    }
}
