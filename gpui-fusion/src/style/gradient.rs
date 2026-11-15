use super::StyleValue;
use crate::theme::{ResolveContext, StyleError};
use gpui::Hsla;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GradientStop {
    pub position: f32,
    pub color: StyleValue<Hsla>,
}

/// Ordered theme-aware stops shared by fill and perimeter gradients.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    pub angle: f32,
    pub stops: Vec<GradientStop>,
}
impl Gradient {
    pub fn new(angle: f32, stops: impl IntoIterator<Item = (f32, StyleValue<Hsla>)>) -> Self {
        Self {
            angle,
            stops: stops
                .into_iter()
                .map(|(position, color)| GradientStop { position, color })
                .collect(),
        }
    }
    fn colors(&self, context: &ResolveContext<'_>) -> Result<Vec<(f32, Hsla)>, StyleError> {
        if !self.angle.is_finite()
            || self.stops.len() < 2
            || self.stops.len() > 64
            || self
                .stops
                .iter()
                .any(|s| !s.position.is_finite() || !(0. ..=1.).contains(&s.position))
            || self.stops.windows(2).any(|s| s[0].position > s[1].position)
        {
            return Err(StyleError::new(
                "gradient",
                "expected a finite angle and 2..=64 ordered stops in 0..=1",
            ));
        }
        self.stops
            .iter()
            .map(|s| Ok((s.position, s.color.resolve(context)?)))
            .collect()
    }
    pub(super) fn background(
        &self,
        context: &ResolveContext<'_>,
    ) -> Result<gpui::Background, StyleError> {
        Ok(gpui::multi_linear_gradient(
            self.angle,
            self.colors(context)?
                .into_iter()
                .map(|(position, color)| gpui::linear_color_stop(color, position))
                .collect::<Vec<_>>(),
        ))
    }
    pub(super) fn border(
        &self,
        context: &ResolveContext<'_>,
    ) -> Result<gpui::BorderGradient, StyleError> {
        Ok(gpui::border_gradient(
            self.colors(context)?
                .into_iter()
                .map(|(position, color)| gpui::border_color_stop(color, position))
                .collect::<Vec<_>>(),
        )
        .phase(self.angle / 360.))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BorderMotion {
    Still,
    Flow { seconds_per_cycle: f32 },
}
