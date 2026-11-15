//! Theme-aware styles for GPUI components.

pub mod components;
pub mod style;
pub mod theme;

pub use style::default_styles;
pub use theme::default_theme;

pub mod prelude {
    pub use crate::style::Styled;
}
