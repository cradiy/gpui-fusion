use std::{fmt::Debug, marker::PhantomData};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::theme::{ResolveContext, StyleError};

/// A typed token reference, relative to the active style set unless qualified.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Token<T> {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_set: Option<String>,
    #[serde(skip)]
    marker: PhantomData<T>,
}

impl<T> Token<T> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            style_set: None,
            marker: PhantomData,
        }
    }

    pub fn in_style_set(mut self, name: impl Into<String>) -> Self {
        self.style_set = Some(name.into());
        self
    }
}

/// Values remain symbolic until resolved against a theme and mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound(
        serialize = "T: Serialize, T::Transform: Serialize",
        deserialize = "T: DeserializeOwned, T::Transform: DeserializeOwned"
    )
)]
pub enum StyleValue<T: StyleType> {
    Literal {
        value: T,
    },
    Token {
        token: Token<T>,
    },
    Derived {
        source: Box<Self>,
        transform: T::Transform,
    },
    Mix {
        from: Box<Self>,
        to: Box<Self>,
        weight: f32,
    },
}

impl<T: StyleType> From<T> for StyleValue<T> {
    fn from(value: T) -> Self {
        Self::Literal { value }
    }
}

impl<T: StyleType> From<Token<T>> for StyleValue<T> {
    fn from(token: Token<T>) -> Self {
        Self::Token { token }
    }
}

impl<T: StyleType> StyleValue<T> {
    pub fn mix(self, other: impl Into<Self>, weight: f32) -> Self {
        Self::Mix {
            from: Box::new(self),
            to: Box::new(other.into()),
            weight,
        }
    }

    pub fn resolve(&self, context: &ResolveContext<'_>) -> Result<T, StyleError> {
        context.resolve(self)
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for gpui::Hsla {}
    impl Sealed for gpui::Pixels {}
    impl Sealed for f32 {}
}

/// Supported property types and their serializable transformations.
pub trait StyleType:
    sealed::Sealed + Clone + Debug + PartialEq + Serialize + DeserializeOwned + schemars::JsonSchema
{
    type Transform: Clone + Debug + PartialEq + Serialize + DeserializeOwned + schemars::JsonSchema;
    fn validate(&self) -> bool;
    fn mix(self, other: Self, weight: f32) -> Self;
    fn transform(self, operation: &Self::Transform) -> Option<Self>;
    fn definition(value: &crate::theme::TokenDefinition)
    -> Option<&crate::theme::ModeValues<Self>>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColorTransform {
    Opacity { factor: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum NumericTransform {
    Scale { factor: f32 },
}

impl StyleType for gpui::Hsla {
    type Transform = ColorTransform;
    fn mix(self, other: Self, weight: f32) -> Self {
        let a = gpui::Rgba::from(self);
        let b = gpui::Rgba::from(other);
        gpui::Rgba {
            r: a.r + (b.r - a.r) * weight,
            g: a.g + (b.g - a.g) * weight,
            b: a.b + (b.b - a.b) * weight,
            a: a.a + (b.a - a.a) * weight,
        }
        .into()
    }
    fn validate(&self) -> bool {
        [self.h, self.s, self.l, self.a]
            .iter()
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
    }
    fn transform(mut self, operation: &Self::Transform) -> Option<Self> {
        let ColorTransform::Opacity { factor } = *operation;
        if !factor.is_finite() || !(0. ..=1.).contains(&factor) {
            return None;
        }
        self.a *= factor;
        Some(self)
    }
    fn definition(
        value: &crate::theme::TokenDefinition,
    ) -> Option<&crate::theme::ModeValues<Self>> {
        if let crate::theme::TokenDefinition::Color(values) = value {
            Some(values)
        } else {
            None
        }
    }
}

macro_rules! numeric_type {
    ($ty:ty, $variant:ident, $finite:expr) => {
        impl StyleType for $ty {
            type Transform = NumericTransform;
            fn mix(self, other: Self, weight: f32) -> Self {
                self + (other - self) * weight
            }
            fn validate(&self) -> bool {
                ($finite)(self)
            }
            fn transform(self, operation: &Self::Transform) -> Option<Self> {
                let NumericTransform::Scale { factor } = *operation;
                if !factor.is_finite() {
                    return None;
                }
                let value = self * factor;
                value.validate().then_some(value)
            }
            fn definition(
                value: &crate::theme::TokenDefinition,
            ) -> Option<&crate::theme::ModeValues<Self>> {
                if let crate::theme::TokenDefinition::$variant(values) = value {
                    Some(values)
                } else {
                    None
                }
            }
        }
        impl StyleValue<$ty> {
            pub fn scale(self, factor: f32) -> Self {
                Self::Derived {
                    source: Box::new(self),
                    transform: NumericTransform::Scale { factor },
                }
            }
        }
        impl Token<$ty> {
            pub fn scale(self, factor: f32) -> StyleValue<$ty> {
                StyleValue::from(self).scale(factor)
            }
        }
    };
}
numeric_type!(gpui::Pixels, Length, |v: &gpui::Pixels| v
    .as_f32()
    .is_finite());
numeric_type!(f32, Number, |v: &f32| v.is_finite());

impl StyleValue<gpui::Hsla> {
    pub fn opacity(self, factor: f32) -> Self {
        Self::Derived {
            source: Box::new(self),
            transform: ColorTransform::Opacity { factor },
        }
    }
}
impl Token<gpui::Hsla> {
    pub fn opacity(self, factor: f32) -> StyleValue<gpui::Hsla> {
        StyleValue::from(self).opacity(factor)
    }
}
impl From<gpui::Rgba> for StyleValue<gpui::Hsla> {
    fn from(value: gpui::Rgba) -> Self {
        Self::from(gpui::Hsla::from(value))
    }
}
