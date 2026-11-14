use super::{ComputedStyle, Declarations, Property, State, StateSelector, StyleValue};
use crate::theme::{ResolveContext, StyleError};
use gpui::{Hsla, Pixels};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub when: StateSelector,
    pub style: StylePatch,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct StylePatch {
    pub properties: Declarations,
    pub states: Vec<StateRule>,
    pub parts: BTreeMap<String, StylePatch>,
}

/// Components implement this for their own part identifiers.
pub trait Part {
    fn name(&self) -> &str;
}
#[derive(Clone, Debug)]
pub struct PartName(pub String);
impl Part for PartName {
    fn name(&self) -> &str {
        &self.0
    }
}

macro_rules! property_methods {
    ($($method:ident => $field:ident: $ty:ty),* $(,)?) => {
        $(fn $method(mut self, value: impl Into<StyleValue<$ty>>) -> Self {
            self.style_mut().properties.$field = Property::Set(value.into());
            self
        })*
    };
}

/// Symbolic styling shared by Fusion elements; no GPUI style trait is required by callers.
pub trait Styled: Sized {
    type Part: Part;
    fn style_mut(&mut self) -> &mut StylePatch;

    property_methods! {
        w => width: Pixels, h => height: Pixels,
        pt => padding_top: Pixels, pr => padding_right: Pixels,
        pb => padding_bottom: Pixels, pl => padding_left: Pixels,
        gap => gap: Pixels, rounded => radius: Pixels, border => border_width: Pixels,
        bg => background: Hsla, text_color => foreground: Hsla,
        border_color => border_color: Hsla, opacity => opacity: f32,
    }
    fn p(self, value: impl Into<StyleValue<Pixels>>) -> Self {
        let value = value.into();
        self.pt(value.clone())
            .pr(value.clone())
            .pb(value.clone())
            .pl(value)
    }
    fn px(self, value: impl Into<StyleValue<Pixels>>) -> Self {
        let value = value.into();
        self.pl(value.clone()).pr(value)
    }
    fn py(self, value: impl Into<StyleValue<Pixels>>) -> Self {
        let value = value.into();
        self.pt(value.clone()).pb(value)
    }
    fn clear_bg(mut self) -> Self {
        self.style_mut().properties.background = Property::Clear;
        self
    }
    fn clear_text_color(mut self) -> Self {
        self.style_mut().properties.foreground = Property::Clear;
        self
    }
    fn state(
        mut self,
        when: impl Into<StateSelector>,
        build: impl FnOnce(StylePatch) -> StylePatch,
    ) -> Self {
        self.style_mut().states.push(StateRule {
            when: when.into(),
            style: build(StylePatch::default()),
        });
        self
    }
    fn hover(self, build: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.state(State::Hovered, build)
    }
    fn pressed(self, build: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.state(State::Pressed, build)
    }
    fn focus_visible(self, build: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.state(State::FocusVisible, build)
    }
    fn part(mut self, part: Self::Part, build: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        let target = self
            .style_mut()
            .parts
            .entry(part.name().to_owned())
            .or_default();
        *target = build(std::mem::take(target));
        self
    }
}
impl Styled for StylePatch {
    type Part = PartName;
    fn style_mut(&mut self) -> &mut StylePatch {
        self
    }
}

impl StylePatch {
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Self)
    }
    pub fn from_json(source: &str) -> Result<Self, StyleError> {
        serde_json::from_str(source).map_err(|e| StyleError::new("$", e.to_string()))
    }

    /// Checks every branch, including inactive states, against the supplied context and part schema.
    pub fn validate(&self, context: &ResolveContext<'_>, parts: &[&str]) -> Result<(), StyleError> {
        self.validate_inner(context, parts, 0)
    }
    fn validate_inner(
        &self,
        context: &ResolveContext<'_>,
        parts: &[&str],
        depth: usize,
    ) -> Result<(), StyleError> {
        if depth >= 64 {
            return Err(StyleError::new("style", "style nesting limit exceeded"));
        }
        let mut computed = ComputedStyle::default();
        self.properties.apply(context, &mut computed)?;
        computed.validate()?;
        for (index, rule) in self.states.iter().enumerate() {
            if rule.when.0.is_empty() {
                return Err(StyleError::new(
                    format!("states[{index}]"),
                    "state selector must not be empty",
                ));
            }
            rule.style
                .validate_inner(context, parts, depth + 1)
                .map_err(|e| StyleError::new(format!("states[{index}].{}", e.path), e.message))?;
        }
        for (name, style) in &self.parts {
            if !parts.contains(&name.as_str()) {
                return Err(StyleError::new(
                    format!("parts.{name}"),
                    "unknown component part",
                ));
            }
            style
                .validate_inner(context, &[], depth + 1)
                .map_err(|e| StyleError::new(format!("parts.{name}.{}", e.path), e.message))?;
        }
        Ok(())
    }

    fn apply(
        &self,
        context: &ResolveContext<'_>,
        states: &StateSelector,
        part: Option<&str>,
        target: &mut ComputedStyle,
        depth: usize,
    ) -> Result<(), StyleError> {
        if depth >= 64 {
            return Err(StyleError::new("style", "style nesting limit exceeded"));
        }
        if let Some(part) = part {
            if let Some(style) = self.parts.get(part) {
                style.apply(context, states, None, target, depth + 1)?;
            }
        } else {
            self.properties.apply(context, target)?;
        }
        for (index, rule) in self.states.iter().enumerate() {
            if rule.when.0.is_empty() {
                return Err(StyleError::new(
                    format!("states[{index}]"),
                    "state selector must not be empty",
                ));
            }
            if states.contains(&rule.when) {
                rule.style.apply(context, states, part, target, depth + 1)?;
            }
        }
        Ok(())
    }
}

/// Layers are ordered from defaults to caller overrides. Matching states are applied within each layer.
pub fn resolve_style(
    layers: &[&StylePatch],
    context: &ResolveContext<'_>,
    states: &StateSelector,
    part: Option<&str>,
) -> Result<ComputedStyle, StyleError> {
    let mut computed = ComputedStyle::default();
    for layer in layers {
        layer.apply(context, states, part, &mut computed, 0)?;
    }
    computed.validate()?;
    Ok(computed)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Recipe {
    pub parts: BTreeSet<String>,
    pub base: StylePatch,
    pub variants: BTreeMap<String, StylePatch>,
}
impl Recipe {
    pub fn validate(&self, context: &ResolveContext<'_>) -> Result<(), StyleError> {
        let parts: Vec<_> = self.parts.iter().map(String::as_str).collect();
        self.base.validate(context, &parts)?;
        for (name, variant) in &self.variants {
            variant
                .validate(context, &parts)
                .map_err(|e| StyleError::new(format!("variants.{name}"), e.to_string()))?;
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        variant: Option<&str>,
        instance: &StylePatch,
        context: &ResolveContext<'_>,
        states: &StateSelector,
        part: Option<&str>,
    ) -> Result<ComputedStyle, StyleError> {
        if part.is_some_and(|name| !self.parts.contains(name)) {
            return Err(StyleError::new("parts", "unknown component part"));
        }
        let mut layers = vec![&self.base];
        if let Some(name) = variant {
            layers.push(
                self.variants.get(name).ok_or_else(|| {
                    StyleError::new(format!("variants.{name}"), "unknown variant")
                })?,
            );
        }
        layers.push(instance);
        resolve_style(&layers, context, states, part)
    }
}
