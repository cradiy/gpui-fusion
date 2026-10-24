use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Ordered fallback families, including literal names and generic categories.
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontFallbacks(pub Arc<Vec<super::FontFamily>>);

impl FontFallbacks {
    /// Get the ordered fallback families.
    pub fn fallback_list(&self) -> &[super::FontFamily] {
        self.0.as_slice()
    }

    /// Create a fallback chain from explicit families.
    pub fn from_fonts(fonts: Vec<super::FontFamily>) -> Self {
        FontFallbacks(Arc::new(fonts))
    }
}
