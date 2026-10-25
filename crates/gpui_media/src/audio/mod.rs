#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_family = "wasm"))]
mod output;
mod player;
mod source;
#[cfg(not(target_family = "wasm"))]
mod worker;
#[cfg(target_family = "wasm")]
#[path = "browser.rs"]
mod worker;

pub use player::{
    AudioInfo, AudioPlayer, AudioPlayerBuilder, AudioPlayerEvent, AudioPlayerOptions,
    PreparedAudioPlayer,
};
pub use source::{AudioSource, AudioStreamHint, AudioStreamWriter};
