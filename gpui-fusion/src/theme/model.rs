use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::style::{StyleType, StyleValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleError {
    pub path: String,
    pub message: String,
}
impl StyleError {
    pub(crate) fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for StyleError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "StyleValue<T>: Serialize",
        deserialize = "StyleValue<T>: Deserialize<'de>"
    )
)]
pub struct ModeValues<T: StyleType> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<StyleValue<T>>,
    #[serde(default)]
    pub modes: BTreeMap<String, StyleValue<T>>,
}
impl<T: StyleType> ModeValues<T> {
    pub fn constant(value: impl Into<StyleValue<T>>) -> Self {
        Self {
            default: Some(value.into()),
            modes: BTreeMap::new(),
        }
    }
    pub fn mode(mut self, name: impl Into<String>, value: impl Into<StyleValue<T>>) -> Self {
        self.modes.insert(name.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "type",
    content = "values",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TokenDefinition {
    Color(ModeValues<gpui::Hsla>),
    Length(ModeValues<gpui::Pixels>),
    Number(ModeValues<f32>),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct StyleSet {
    pub extends: Option<String>,
    pub tokens: BTreeMap<String, TokenDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub material: crate::style::MaterialMode,
    pub modes: BTreeSet<String>,
    pub style_sets: BTreeMap<String, StyleSet>,
}

/// Local overrides are resolved from outermost to innermost scope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeScope {
    pub style_set: Option<String>,
    pub tokens: BTreeMap<String, TokenDefinition>,
}

impl Theme {
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Self)
    }
    pub fn from_json(source: &str) -> Result<Self, StyleError> {
        let theme: Self =
            serde_json::from_str(source).map_err(|e| StyleError::new("$", e.to_string()))?;
        theme.validate()?;
        Ok(theme)
    }

    pub fn validate(&self) -> Result<(), StyleError> {
        if self.modes.is_empty() {
            return Err(StyleError::new("modes", "at least one mode is required"));
        }
        if self.modes.iter().any(|name| name.is_empty()) {
            return Err(StyleError::new("modes", "mode names must not be empty"));
        }
        for (name, set) in &self.style_sets {
            if name.is_empty() {
                return Err(StyleError::new(
                    "style_sets",
                    "style names must not be empty",
                ));
            }
            let mut names = BTreeSet::new();
            let mut current = Some(name.as_str());
            let mut inherited_tokens = BTreeSet::new();
            while let Some(parent) = current {
                if !names.insert(parent) {
                    return Err(StyleError::new(name, "style inheritance cycle"));
                }
                let style = self
                    .style_sets
                    .get(parent)
                    .ok_or_else(|| StyleError::new(parent, "unknown style set"))?;
                inherited_tokens.extend(style.tokens.keys());
                current = style.extends.as_deref();
            }
            for (token, definition) in &set.tokens {
                if token.is_empty() {
                    return Err(StyleError::new(name, "token names must not be empty"));
                }
                let modes: Vec<_> = match definition {
                    TokenDefinition::Color(v) => v.modes.keys().collect(),
                    TokenDefinition::Length(v) => v.modes.keys().collect(),
                    TokenDefinition::Number(v) => v.modes.keys().collect(),
                };
                for mode in modes {
                    if !self.modes.contains(mode) {
                        return Err(StyleError::new(
                            format!("{name}.{token}.{mode}"),
                            "unknown mode",
                        ));
                    }
                }
            }
            for mode in &self.modes {
                let context = ResolveContext::new(self, mode, name)?;
                for (token, definition) in &set.tokens {
                    let validate = match definition {
                        TokenDefinition::Color(values) => values
                            .default
                            .as_ref()
                            .map(|value| context.resolve(value).map(|_| ())),
                        TokenDefinition::Length(values) => values
                            .default
                            .as_ref()
                            .map(|value| context.resolve(value).map(|_| ())),
                        TokenDefinition::Number(values) => values
                            .default
                            .as_ref()
                            .map(|value| context.resolve(value).map(|_| ())),
                    };
                    if let Some(result) = validate {
                        result.map_err(|error| {
                            StyleError::new(format!("{name}.{token}.default"), error.to_string())
                        })?;
                    }
                }

                for token in &inherited_tokens {
                    // Resolve inherited aliases against the selected child's overrides.
                    let definitions = context.definitions(name, token, false)?;
                    let definition = definitions[0];
                    match definition {
                        TokenDefinition::Color(_) => {
                            context.resolve(&StyleValue::<gpui::Hsla>::from(
                                crate::style::Token::new(*token),
                            ))?;
                        }
                        TokenDefinition::Length(_) => {
                            context.resolve(&StyleValue::<gpui::Pixels>::from(
                                crate::style::Token::new(*token),
                            ))?;
                        }
                        TokenDefinition::Number(_) => {
                            context.resolve(&StyleValue::<f32>::from(crate::style::Token::new(
                                *token,
                            )))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ResolveContext<'a> {
    theme: &'a Theme,
    mode: &'a str,
    style_set: &'a str,
    scopes: Vec<&'a ThemeScope>,
}

impl<'a> ResolveContext<'a> {
    pub fn new(theme: &'a Theme, mode: &'a str, style_set: &'a str) -> Result<Self, StyleError> {
        if !theme.modes.contains(mode) {
            return Err(StyleError::new(mode, "unknown mode"));
        }
        if !theme.style_sets.contains_key(style_set) {
            return Err(StyleError::new(style_set, "unknown style set"));
        }
        Ok(Self {
            theme,
            mode,
            style_set,
            scopes: Vec::new(),
        })
    }

    pub fn scoped(mut self, scope: &'a ThemeScope) -> Result<Self, StyleError> {
        if let Some(name) = scope.style_set.as_deref() {
            if !self.theme.style_sets.contains_key(name) {
                return Err(StyleError::new(name, "unknown style set"));
            }
            self.style_set = name;
        }
        self.scopes.push(scope);
        Ok(self)
    }

    fn definitions(
        &self,
        style: &str,
        token: &str,
        qualified: bool,
    ) -> Result<Vec<&TokenDefinition>, StyleError> {
        let mut definitions = Vec::new();
        if !qualified {
            for scope in self.scopes.iter().rev() {
                if let Some(value) = scope.tokens.get(token) {
                    definitions.push(value);
                }
            }
        }
        let mut current = Some(style);
        let mut visited = BTreeSet::new();
        while let Some(name) = current {
            if !visited.insert(name) {
                return Err(StyleError::new(name, "style inheritance cycle"));
            }
            let set = self
                .theme
                .style_sets
                .get(name)
                .ok_or_else(|| StyleError::new(name, "unknown style set"))?;
            if let Some(value) = set.tokens.get(token) {
                definitions.push(value);
            }
            current = set.extends.as_deref();
        }
        if definitions.is_empty() {
            return Err(StyleError::new(format!("{style}.{token}"), "unknown token"));
        }
        Ok(definitions)
    }

    pub fn resolve<T: StyleType>(&self, value: &StyleValue<T>) -> Result<T, StyleError> {
        self.resolve_inner(value, self.style_set, false, &mut Vec::new(), 0)
    }

    fn resolve_inner<T: StyleType>(
        &self,
        value: &StyleValue<T>,
        style: &str,
        qualified: bool,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<T, StyleError> {
        if depth >= 64 {
            return Err(StyleError::new(
                stack.join(" -> "),
                "style expression exceeds nesting limit",
            ));
        }
        let result = match value {
            StyleValue::Literal { value } => value.clone(),
            StyleValue::Mix { from, to, weight } => {
                if !weight.is_finite() || !(0. ..=1.).contains(weight) {
                    return Err(StyleError::new(
                        stack.join(" -> "),
                        "mix weight must be in 0..=1",
                    ));
                }
                self.resolve_inner(from, style, qualified, stack, depth + 1)?
                    .mix(
                        self.resolve_inner(to, style, qualified, stack, depth + 1)?,
                        *weight,
                    )
            }
            StyleValue::Derived { source, transform } => self
                .resolve_inner(source, style, qualified, stack, depth + 1)?
                .transform(transform)
                .ok_or_else(|| StyleError::new(stack.join(" -> "), "invalid transformation"))?,
            StyleValue::Token { token } => {
                let selected = token.style_set.as_deref().unwrap_or(style);
                let qualified = qualified || token.style_set.is_some();
                let key = format!(
                    "{selected}.{}.{}@{}",
                    token.name,
                    self.mode,
                    if qualified { "theme" } else { "scope" }
                );
                if stack.contains(&key) {
                    return Err(StyleError::new(key, "token reference cycle"));
                }
                let mut expression = None;
                for definition in self.definitions(selected, &token.name, qualified)? {
                    let values = T::definition(definition)
                        .ok_or_else(|| StyleError::new(&key, "token type mismatch"))?;
                    expression = values.modes.get(self.mode).or(values.default.as_ref());
                    if expression.is_some() {
                        break;
                    }
                }
                let expression = expression
                    .ok_or_else(|| StyleError::new(&key, "no value for mode and no default"))?;
                stack.push(key.clone());
                let resolved = self
                    .resolve_inner(expression, selected, qualified, stack, depth + 1)
                    .map_err(|e| StyleError::new(key, e.to_string()));
                stack.pop();
                resolved?
            }
        };
        if !result.validate() {
            return Err(StyleError::new(stack.join(" -> "), "invalid style value"));
        }
        Ok(result)
    }
}

/// Validated theme state. Hosts notify dependent views after successful changes.
pub struct ThemeStore {
    theme: Theme,
    mode: String,
    revision: u64,
}
impl ThemeStore {
    pub fn new(theme: Theme, mode: impl Into<String>) -> Result<Self, StyleError> {
        theme.validate()?;
        let mode = mode.into();
        if !theme.modes.contains(&mode) {
            return Err(StyleError::new(mode, "unknown mode"));
        }
        Ok(Self {
            theme,
            mode,
            revision: 0,
        })
    }
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
    pub fn mode(&self) -> &str {
        &self.mode
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn set_material_mode(&mut self, material: crate::style::MaterialMode) -> bool {
        if self.theme.material == material {
            return false;
        }
        self.theme.material = material;
        self.revision = self.revision.wrapping_add(1);
        true
    }
    pub fn set_mode(&mut self, mode: impl Into<String>) -> Result<bool, StyleError> {
        let mode = mode.into();
        if !self.theme.modes.contains(&mode) {
            return Err(StyleError::new(mode, "unknown mode"));
        }
        if mode == self.mode {
            return Ok(false);
        }
        self.mode = mode;
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub fn replace_json(&mut self, source: &str) -> Result<(), StyleError> {
        let theme = serde_json::from_str(source)
            .map_err(|error| StyleError::new("$", error.to_string()))?;
        self.replace(theme)
    }
    pub fn replace(&mut self, theme: Theme) -> Result<(), StyleError> {
        theme.validate()?;
        if !theme.modes.contains(&self.mode) {
            return Err(StyleError::new(&self.mode, "replacement omits active mode"));
        }
        self.theme = theme;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
}

/// A complete application appearance selection, loadable from one JSON document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ThemeConfig {
    pub theme: Theme,
    pub mode: String,
    pub style_set: String,
}
impl ThemeConfig {
    pub fn from_json(source: &str) -> Result<Self, StyleError> {
        let config: Self = serde_json::from_str(source)
            .map_err(|error| StyleError::new("$", error.to_string()))?;
        config.theme.validate()?;
        ResolveContext::new(&config.theme, &config.mode, &config.style_set)?;
        Ok(config)
    }
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, StyleError> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path)
            .map_err(|error| StyleError::new(path.display().to_string(), error.to_string()))?;
        Self::from_json(&source)
    }
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Self)
    }
}
