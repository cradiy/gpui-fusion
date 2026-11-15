//! Theme-aware styles for GPUI components.

pub mod button;
pub mod runtime;
pub mod slider;
pub mod style;
pub mod switch;
pub mod theme;

pub mod prelude {
    pub use crate::style::Styled;
}

mod default_theme;
pub use default_theme::default_theme;

mod button_motion;
