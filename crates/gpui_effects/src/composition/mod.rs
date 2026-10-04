//! Effect elements, masks and subtree composition.

mod effect_stage;
mod element;
mod masked_effect;
mod masked_fill;
mod subtree_effect;

pub use effect_stage::EffectStage;
pub use element::{Effect, effect, four_image_effect, image_effect, two_image_effect};
pub use masked_effect::{MaskedEffect, effect_svg, effect_text, masked_effect};
pub use masked_fill::{MaskedFill, gradient_svg, gradient_text, masked_fill};
pub use subtree_effect::{SubtreeEffect, subtree_effect, subtree_effect_chain};
