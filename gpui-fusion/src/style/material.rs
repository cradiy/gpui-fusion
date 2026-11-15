use super::StyleValue;
use crate::theme::{ResolveContext, StyleError};
use gpui::{Hsla, Pixels};
use gpui_effects::LiquidGlassStyle;
use serde::{Deserialize, Serialize};

/// Surface rendering independent of foreground content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Material {
    Plain,
    Glass {
        blur: StyleValue<Pixels>,
        refraction: StyleValue<Pixels>,
        thickness: StyleValue<Pixels>,
        edge_light: StyleValue<f32>,
        edge_color: StyleValue<Hsla>,
        frost: StyleValue<f32>,
        tint: StyleValue<Hsla>,
        frost_center: StyleValue<Hsla>,
        frost_edge: StyleValue<Hsla>,
    },
}

impl Material {
    pub fn resolve(&self, cx: &ResolveContext<'_>) -> Result<Option<LiquidGlassStyle>, StyleError> {
        let Self::Glass {
            blur,
            refraction,
            thickness,
            edge_light,
            edge_color,
            frost,
            tint,
            frost_center,
            frost_edge,
        } = self
        else {
            return Ok(None);
        };
        let mut glass = LiquidGlassStyle::optical();
        glass.blur_radius = blur.resolve(cx)?;
        glass.refraction = refraction.resolve(cx)?;
        glass.thickness = thickness.resolve(cx)?;
        let edge_light = edge_light.resolve(cx)?;
        glass.optics.rim_strength *= edge_light;
        glass.optics.top_reflection *= edge_light;
        glass.optics.caustic_strength *= edge_light;
        glass.edge_color = edge_color.resolve(cx)?;
        glass.frost = frost.resolve(cx)?;
        glass.tint = tint.resolve(cx)?;
        glass.frost_center = frost_center.resolve(cx)?;
        glass.frost_edge = frost_edge.resolve(cx)?;
        if [glass.blur_radius, glass.refraction, glass.thickness]
            .iter()
            .any(|x| x.as_f32() < 0.)
            || !(0. ..=1.).contains(&glass.frost)
            || !(0. ..=1.).contains(&edge_light)
        {
            return Err(StyleError::new("material", "invalid optical range"));
        }
        // Button-sized surfaces use a compact shadow and no interior magnification.
        glass.shadow.spread = gpui::px(8.);
        glass.shadow.offset.y = gpui::px(3.);
        Ok(Some(glass))
    }
}

/// Application-wide surface mode, independent of the color theme.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MaterialMode {
    #[default]
    Plain,
    Glass,
}
