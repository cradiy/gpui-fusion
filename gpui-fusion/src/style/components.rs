use super::Recipe;
use crate::theme::{ResolveContext, StyleError, Theme};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Component recipes are configured separately from the theme's semantic values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ComponentStyles {
    pub recipes: BTreeMap<String, Recipe>,
}
impl ComponentStyles {
    pub fn from_json(source: &str) -> Result<Self, StyleError> {
        serde_json::from_str(source).map_err(|error| StyleError::new("$", error.to_string()))
    }
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Self)
    }
    pub fn recipe(&self, name: &str) -> Result<&Recipe, StyleError> {
        self.recipes
            .get(name)
            .ok_or_else(|| StyleError::new(name, "unknown component recipe"))
    }
    pub fn validate(&self, theme: &Theme) -> Result<(), StyleError> {
        for mode in &theme.modes {
            for set in theme.style_sets.keys() {
                let context = ResolveContext::new(theme, mode, set)?;
                for (name, recipe) in &self.recipes {
                    if name.is_empty() {
                        return Err(StyleError::new(
                            "recipes",
                            "component name must not be empty",
                        ));
                    }
                    recipe.validate(&context).map_err(|error| {
                        StyleError::new(format!("{name}.{set}.{mode}"), error.to_string())
                    })?;
                }
            }
        }
        Ok(())
    }
}
