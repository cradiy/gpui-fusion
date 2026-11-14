use gpui::{Hsla, px, rgb};
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
        let mut surface = LiquidGlassStyle::clear()
            .refraction(px(0.))
            .thickness(px(0.))
            .dispersion(0.);
        surface.blur_radius = px(12.);
        surface.optics.center_blur = 1.;
        surface.frost = 0.65;
        surface.frost_center = rgb(0xdedfe2).into();
        surface.frost_edge = surface.frost_center;
        surface.shadow.strength = 0.;
        surface.optics.rim_strength = 0.;
        surface.optics.top_reflection = 0.;
        surface.optics.inner_shading = 0.;
        surface.optics.caustic_strength = 0.;
        let mut selection = LiquidGlassStyle::optical();
        selection.frost = 0.82;
        selection.optics.magnification = 0.12;
        Self {
            surface,
            selection,
            selected_text: rgb(0x1d1d1f).into(),
            disabled_opacity: 0.45,
        }
    }

    /// Pair with a light foreground color through `Styled::text_color`.
    pub fn dark() -> Self {
        let mut material = Self::light();
        material.surface.frost_center = rgb(0x24252a).into();
        material.surface.frost_edge = material.surface.frost_center;
        material.selection.frost_center = rgb(0x2a2c31).into();
        material.selection.frost_edge = rgb(0x1c1d21).into();
        material.selected_text = rgb(0xf5f5f7).into();
        material
    }
}

impl Default for GlassSegmentedAppearance {
    fn default() -> Self {
        Self::light()
    }
}
