use super::{StyleType, StyleValue};
use crate::theme::{ResolveContext, StyleError};
use gpui::{Hsla, Pixels};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    Solid,
    Dashed,
}

/// Unset inherits a lower layer; Clear explicitly removes that layer's value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound(
        serialize = "StyleValue<T>: Serialize",
        deserialize = "StyleValue<T>: Deserialize<'de>"
    )
)]
pub enum Property<T: StyleType> {
    #[default]
    Unset,
    Clear,
    Set(StyleValue<T>),
}
impl<T: StyleType> Property<T> {
    fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Resolved<T> {
    #[default]
    Unset,
    Clear,
    Value(T),
}

macro_rules! declarations {
    ($($field:ident: $ty:ty),* $(,)?) => {
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(default, deny_unknown_fields)]
        pub struct Declarations {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub background_gradient: Option<super::Gradient>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub border_gradient: Option<super::Gradient>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub border_motion: Option<super::BorderMotion>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub border_style: Option<BorderStyle>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub material: Option<super::Material>,
            $(#[serde(skip_serializing_if = "Property::is_unset")]
            pub $field: Property<$ty>,)*
        }
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct ComputedStyle {
            pub background_gradient: Option<gpui::Background>,
            pub border_gradient: Option<gpui::BorderGradient>,
            pub border_motion: Option<super::BorderMotion>,
            pub border_style: Option<BorderStyle>,
            pub material: Option<gpui_effects::LiquidGlassStyle>,
            $(pub $field: Resolved<$ty>,)*
        }
        impl Declarations {
            pub(super) fn apply(&self, context: &ResolveContext<'_>, target: &mut ComputedStyle) -> Result<(), StyleError> {
                $(match &self.$field {
                    Property::Unset => {},
                    Property::Clear => target.$field = Resolved::Clear,
                    Property::Set(value) => target.$field = Resolved::Value(value.resolve(context)
                        .map_err(|e| StyleError::new(stringify!($field), e.to_string()))?),
                })*
                if !self.background.is_unset() { target.background_gradient = None; }
                if !self.border_color.is_unset() { target.border_gradient = None; }
                if let Some(gradient) = &self.background_gradient { target.background_gradient = Some(gradient.background(context)?); }
                if let Some(gradient) = &self.border_gradient { target.border_gradient = Some(gradient.border(context)?); }
                if let Some(motion) = self.border_motion {
                    if let super::BorderMotion::Flow { seconds_per_cycle } = motion {
                        if !seconds_per_cycle.is_finite() || seconds_per_cycle <= 0. { return Err(StyleError::new("border_motion", "cycle duration must be finite and positive")); }
                    }
                    target.border_motion = Some(motion);
                }
                if let Some(material) = &self.material { target.material = material.resolve(context)?; }
                if let Some(border_style) = self.border_style { target.border_style = Some(border_style); }
                Ok(())
            }
        }
    };
}
declarations! {
    width: Pixels, height: Pixels, font_size: Pixels,
    padding_top: Pixels, padding_right: Pixels, padding_bottom: Pixels, padding_left: Pixels,
    gap: Pixels, radius: Pixels, border_width: Pixels,
    background: Hsla, foreground: Hsla, border_color: Hsla, opacity: f32,
    glass_edge_color: Hsla,
}

impl ComputedStyle {
    pub(super) fn validate(&self) -> Result<(), StyleError> {
        for (name, property) in [
            ("font_size", &self.font_size),
            ("width", &self.width),
            ("height", &self.height),
            ("padding_top", &self.padding_top),
            ("padding_right", &self.padding_right),
            ("padding_bottom", &self.padding_bottom),
            ("padding_left", &self.padding_left),
            ("gap", &self.gap),
            ("radius", &self.radius),
            ("border_width", &self.border_width),
        ] {
            if let Resolved::Value(value) = property {
                if !value.as_f32().is_finite() || value.as_f32() < 0. {
                    return Err(StyleError::new(
                        name,
                        "expected a finite nonnegative length",
                    ));
                }
            }
        }
        if let Resolved::Value(value) = self.opacity {
            if !value.is_finite() || !(0. ..=1.).contains(&value) {
                return Err(StyleError::new("opacity", "expected a number in 0..=1"));
            }
        }
        Ok(())
    }

    /// Builds a fresh GPUI refinement. Cleared paint properties use neutral values.
    pub fn to_gpui(&self) -> gpui::StyleRefinement {
        use gpui::Styled as _;
        let mut style = gpui::StyleRefinement::default();
        style.border_style = self.border_style.map(|border| match border {
            BorderStyle::Solid => gpui::BorderStyle::Solid,
            BorderStyle::Dashed => gpui::BorderStyle::Dashed,
        });
        macro_rules! set {
            ($field:ident, $method:ident, $clear:expr) => {
                match &self.$field {
                    Resolved::Unset => {}
                    Resolved::Clear => {
                        style = style.$method($clear);
                    }
                    Resolved::Value(value) => {
                        style = style.$method(*value);
                    }
                }
            };
        }
        set!(font_size, text_size, gpui::px(14.));
        set!(width, w, gpui::Length::Auto);
        set!(height, h, gpui::Length::Auto);
        set!(padding_top, pt, gpui::px(0.));
        set!(padding_right, pr, gpui::px(0.));
        set!(padding_bottom, pb, gpui::px(0.));
        set!(padding_left, pl, gpui::px(0.));
        set!(gap, gap, gpui::px(0.));
        set!(radius, rounded, gpui::px(0.));
        set!(border_width, border, gpui::px(0.));
        set!(background, bg, gpui::transparent_black());
        set!(border_color, border_color, gpui::transparent_black());
        set!(opacity, opacity, 1.);
        if let Resolved::Value(value) = self.foreground {
            style = style.text_color(value);
        }
        if let Some(background) = &self.background_gradient {
            style.background = Some(background.clone().into());
        }
        style.border_gradient = self.border_gradient.clone();
        style
    }

    pub fn changes_from(&self, previous: &Self) -> StyleChanges {
        let layout = self.font_size != previous.font_size
            || self.width != previous.width
            || self.height != previous.height
            || self.padding_top != previous.padding_top
            || self.padding_right != previous.padding_right
            || self.padding_bottom != previous.padding_bottom
            || self.padding_left != previous.padding_left
            || self.gap != previous.gap
            || self.border_width != previous.border_width;
        StyleChanges {
            layout,
            paint: layout || self != previous,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleChanges {
    pub layout: bool,
    pub paint: bool,
}
