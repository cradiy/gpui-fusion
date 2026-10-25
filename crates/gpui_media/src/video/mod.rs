mod container;
mod surface;
mod window_sizing;

pub use crate::playback::{
    MediaPlayer as VideoPlayer, MediaPlayerBuilder as VideoPlayerBuilder,
    MediaPlayerEvent as VideoPlayerEvent, MediaPlayerOptions as VideoPlayerOptions,
    SystemPlaybackOptions as VideoSystemMediaOptions,
};
pub use container::{VideoContainer, video_container};
pub use surface::VideoSurface;
pub use window_sizing::{fit_video_window_bounds, fit_video_window_size};
