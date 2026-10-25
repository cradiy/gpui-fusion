//! Reusable GPUI audio and video playback components.
//!
//! `MediaPlayer` owns backend-neutral state, timeline and control handling.
//! A pluggable `MediaBackend` owns demuxing, decoding, audio output and the
//! playback clock. With the video feature, `VideoPlayer` renders only the
//! newest decoded video frame with GPUI's dynamic `surface` element. It
//! deliberately contains no controls, pointer behavior, status overlay or
//! fullscreen policy; host applications build those features from the
//! exported state and events.
//!
//! `AudioPlayer` requires only the audio feature. It defaults to Symphonia/CPAL
//! on native targets and browser audio on Web, or accepts a caller-selected
//! media backend through the shared playback layer. It has no built-in UI.

#[cfg(any(feature = "audio", feature = "video"))]
mod playback;
#[cfg(any(feature = "audio", feature = "video"))]
pub use playback::{
    MediaPlayer, MediaPlayerBuilder, MediaPlayerEvent, MediaPlayerOptions, PreparedMediaPlayer,
    SystemPlaybackOptions,
};

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "video")]
mod video;

#[cfg(feature = "audio")]
pub use audio::{
    AudioInfo, AudioPlayer, AudioPlayerBuilder, AudioPlayerEvent, AudioPlayerOptions, AudioSource,
    AudioStreamHint, AudioStreamWriter, PreparedAudioPlayer,
};
pub use gpui_media_core::*;
#[cfg(feature = "video")]
pub use video::*;
