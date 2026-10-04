//! Text presentation, timed reveals and spectrum masks.

mod masked_builtins;
mod text_blur;
mod timed_text;

pub use masked_builtins::{spectrum_mask_shader, spectrum_svg, spectrum_text};
pub use text_blur::TextBlur;
pub use timed_text::{TimedText, TimedTextEmphasis, TimedTextRevealWave, TimedTextUnit};
