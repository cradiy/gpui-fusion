use gpui::{Hsla, hsla, px, rgb};
use gpui_effects::{LiquidGlassAppearance, LiquidGlassLensAppearance};

/// Optical surfaces and semantic colors for a glass segmented control.
/// Layout, typography, outer corners, background, and shadow use `Styled`.
#[derive(Clone, Copy, Debug)]
pub struct GlassSegmentedAppearance {
    pub surface: LiquidGlassAppearance,
    pub selection: LiquidGlassLensAppearance,
    /// Color of monochrome content covered by the animated selection lens.
    /// Alpha multiplies the content's original alpha; uncovered pixels keep their color.
    pub selected_text: Hsla,
    pub disabled_opacity: f32,
}

impl GlassSegmentedAppearance {
    pub fn light() -> Self {
        Self {
            surface: LiquidGlassAppearance::regular()
                .blur_radius(px(3.))
                .clarity(0.8)
                .refraction(px(1.5))
                .thickness(px(10.))
                .highlight(0.2)
                .tint(hsla(0., 0., 1., 0.12)),
            selection: LiquidGlassLensAppearance::default()
                .blur_radius(px(2.))
                .clarity(1.)
                .refraction(px(7.))
                .thickness(px(12.))
                .edge_width(px(6.))
                .fresnel(0.8)
                .highlight(0.32)
                .rim_width(px(1.2))
                .dispersion(0.025)
                .tint(hsla(0., 0., 0., 0.08)),
            selected_text: rgb(0x007aff).into(),
            disabled_opacity: 0.45,
        }
    }

    /// Pair with a light foreground color through `Styled::text_color`.
    pub fn dark() -> Self {
        Self {
            surface: LiquidGlassAppearance::regular()
                .blur_radius(px(3.))
                .clarity(0.8)
                .refraction(px(1.5))
                .thickness(px(10.))
                .highlight(0.18)
                .tint(hsla(0., 0., 1., 0.08)),
            selection: LiquidGlassLensAppearance::default()
                .blur_radius(px(2.))
                .clarity(1.)
                .refraction(px(7.))
                .thickness(px(12.))
                .edge_width(px(6.))
                .fresnel(0.8)
                .highlight(0.32)
                .rim_width(px(1.2))
                .dispersion(0.025)
                .tint(hsla(0., 0., 0., 0.08)),
            selected_text: rgb(0x64b5ff).into(),
            ..Self::light()
        }
    }
}

impl Default for GlassSegmentedAppearance {
    fn default() -> Self {
        Self::light()
    }
}
