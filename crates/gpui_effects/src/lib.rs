//! GPU-driven visual effects for GPUI applications.
//!
//! Effects use WGSL as their canonical implementation. Applications may add
//! native MSL and HLSL implementations through [`gpui::EffectShader`] when an
//! effect needs platform-specific tuning.
//!
//! See [`glass_guide`] for the mergeable frosted-glass material.
//! See [`liquid_glass_guide`] for refractive material configuration and painting.
//! See [`timed_text_guide`] for karaoke timelines, grouped emphasis, and
//! playback-clock integration.

/// Complete usage guide for [`FrostedGlass`](crate::FrostedGlass).
#[doc = include_str!("../docs/glass.md")]
pub mod glass_guide {}

/// Usage guide for [`LiquidGlass`](crate::LiquidGlass) and [`paint_liquid_glass`].
#[doc = include_str!("../docs/liquid_glass.md")]
pub mod liquid_glass_guide {}

/// Usage guide for [`TimedText`](crate::TimedText).
#[doc = include_str!("../docs/timed_text.md")]
pub mod timed_text_guide {}

mod animation;
mod composition;
mod filters;
mod geometry;
mod lighting;
mod simulation;
mod surface;
mod text;

pub use animation::*;
pub use composition::*;
pub use filters::*;
pub use geometry::*;
pub use lighting::*;
pub use simulation::*;
pub use surface::*;
pub use text::*;
