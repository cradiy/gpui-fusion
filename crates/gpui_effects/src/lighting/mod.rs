//! Border illumination, surface highlights and contour lighting.

mod border_glow;
mod border_trail;
mod contour_glow;
mod contour_relief;
mod contour_shadow;
mod light_sweep;
mod spotlight;

pub use border_glow::{BorderGlowOptions, border_glow};
pub use border_trail::{
    BorderTrailMode, BorderTrailOptions, border_trail, border_trail_gradient_shader,
    border_trail_shader,
};
pub use contour_glow::{ContourGlowOptions, contour_glow_shader, subtree_contour_glow};
pub use contour_relief::{
    ContourReliefOptions, contour_relief_shader, contour_surface_wgsl, subtree_contour_relief,
};
pub use contour_shadow::{ContourShadowOptions, contour_shadow_shader, subtree_contour_shadow};
pub use light_sweep::{LightSweepOptions, light_sweep, light_sweep_shader};
pub use spotlight::{SpotlightOptions, spotlight, spotlight_shader};
