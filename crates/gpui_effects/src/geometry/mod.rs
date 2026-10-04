//! Shapes, spatial distortion and depth effects.

mod deformation;
mod depth_parallax;
mod displacement_map;
mod lens;
mod ripple;
mod sdf;
mod sticky;

pub use deformation::{DeformationOptions, ElasticOffset, deformation_shader, subtree_deformation};
pub use depth_parallax::{DepthParallaxOptions, depth_parallax, depth_parallax_shader};
pub use displacement_map::{
    DisplacementMapOptions, DisplacementMapPreset, DisplacementMapSampling, DisplacementSourceEdge,
    displacement_map_shader, masked_displacement_map_shader, subtree_displacement_map,
};
pub use lens::{LensOptions, lens_shader, subtree_lens};
pub use ripple::{MAX_RIPPLES, Ripple, RippleOptions, ripple_shader, subtree_ripples};
pub use sdf::{MAX_SDF_SHAPES, SdfOptions, SdfScene, SdfShape, SdfTransform, sdf};
pub use sticky::{StickyShape, paint_sticky_shapes, sticky_shape_shader};
