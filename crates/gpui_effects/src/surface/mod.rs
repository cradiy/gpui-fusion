//! Backgrounds, gradients, glass and surface materials.

mod backdrop;
mod builtins;
mod color_flow;
mod fluted_glass;
mod glass;
mod grain;
mod liquid_glass;
mod liquid_glass_content;
mod material;
mod point_gradient;

pub use backdrop::*;
pub use builtins::*;
pub use color_flow::{
    ColorFlow, ColorFlowOptions, ColorFlowPalette, ColorFlowPaletteColor, color_flow,
    color_flow_shader,
};
pub use fluted_glass::{FlutedGlassOptions, fluted_glass, fluted_glass_shader};
pub use glass::{FrostedGlass, FrostedGlassAppearance, FrostedGlassShape};
pub use grain::{GrainOptions, grain_shader, surface_grain};
pub use liquid_glass::{
    LiquidGlass, LiquidGlassAppearance, LiquidGlassDeformation, liquid_glass_shader,
    paint_deformed_liquid_glass, paint_liquid_glass,
};
pub use liquid_glass_content::{
    LiquidGlassRegion, liquid_glass_content, liquid_glass_content_shader,
};
pub use material::{
    HolographicOptions, MaterialLight, MaterialSurface, holographic, holographic_image_shader,
    holographic_mask_shader, holographic_masked, holographic_shader,
};
pub use point_gradient::{
    GradientPoint, point_gradient, point_gradient_shader, point_gradient_uniforms,
};
