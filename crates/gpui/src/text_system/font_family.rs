use crate::SharedString;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A platform-resolved category of fonts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GenericFontFamily {
    /// The platform's default user-interface font.
    #[default]
    SystemUi,
    /// A serif family.
    Serif,
    /// A sans-serif family.
    SansSerif,
    /// A fixed-pitch family.
    Monospace,
    /// A cursive family.
    Cursive,
    /// A decorative family.
    Fantasy,
}

/// A literal family name or an explicit platform-resolved font category.
/// String conversions always produce a literal name.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    /// A concrete family name, without virtual aliases.
    Named(SharedString),
    /// A category resolved by the platform or application configuration.
    Generic(GenericFontFamily),
}

impl Default for FontFamily {
    fn default() -> Self {
        GenericFontFamily::SystemUi.into()
    }
}

impl From<GenericFontFamily> for FontFamily {
    fn from(family: GenericFontFamily) -> Self {
        Self::Generic(family)
    }
}

impl From<SharedString> for FontFamily {
    fn from(name: SharedString) -> Self {
        Self::Named(name)
    }
}

impl From<String> for FontFamily {
    fn from(name: String) -> Self {
        Self::Named(name.into())
    }
}

impl From<&'static str> for FontFamily {
    fn from(name: &'static str) -> Self {
        Self::Named(SharedString::new_static(name))
    }
}

impl fmt::Display for FontFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => name.fmt(f),
            Self::Generic(family) => write!(f, "{family:?}"),
        }
    }
}
