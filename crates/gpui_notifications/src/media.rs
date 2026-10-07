use super::*;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[path = "media/linux.rs"]
mod linux;
#[cfg(target_os = "macos")]
#[path = "media/macos.rs"]
mod macos;
#[cfg(target_family = "wasm")]
#[path = "media/web.rs"]
mod web;
#[cfg(target_os = "windows")]
#[path = "media/windows.rs"]
mod windows;

/// Identity of an application publishing system transport controls.
#[derive(Clone, Debug)]
pub struct MediaSessionOptions {
    pub app_id: String,
    pub app_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    /// Stable identity of the current item; change it when switching tracks.
    pub track_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaPlayback {
    #[default]
    Stopped,
    Paused,
    Playing,
    Buffering,
}

/// A complete snapshot. Publish immediately after seeks and state changes, and
/// periodically during playback. Applications retain ownership of the player.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSessionState {
    pub metadata: MediaMetadata,
    pub playback: MediaPlayback,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub rate: f64,
    pub volume: f64,
    pub seekable: bool,
    pub can_next: bool,
    pub can_previous: bool,
    /// Android small drawable; unsupported kinds use the application icon.
    pub icon: Option<NotificationIcon>,
}
impl Default for MediaSessionState {
    fn default() -> Self {
        Self {
            metadata: MediaMetadata::default(),
            playback: MediaPlayback::Stopped,
            position: Duration::ZERO,
            duration: None,
            rate: 1.,
            volume: 1.,
            seekable: false,
            can_next: false,
            can_previous: false,
            icon: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "value", rename_all = "snake_case")]
pub enum MediaCommand {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    SeekTo(Duration),
    /// Signed seconds relative to the application's current position.
    SeekBy(f64),
    SetVolume(f64),
}

pub trait MediaSessionBackend {
    fn update(&self, state: MediaSessionState) -> Result<()>;
    fn seeked(&self, _position: Duration) -> Result<()> {
        Ok(())
    }
}

/// Owns system transport controls. Drop releases them and closes the command stream.
/// This does not create a background task, decode media, or manage an application queue.
pub struct SystemMediaSession {
    backend: Box<dyn MediaSessionBackend>,
    commands: Option<async_channel::Receiver<MediaCommand>>,
}
impl SystemMediaSession {
    pub fn from_backend(
        backend: Box<dyn MediaSessionBackend>,
        commands: async_channel::Receiver<MediaCommand>,
    ) -> Self {
        Self {
            backend,
            commands: Some(commands),
        }
    }
    pub async fn new(options: MediaSessionOptions) -> Result<Self> {
        ensure!(
            !options.app_id.is_empty() && !options.app_name.is_empty(),
            "media session application identity is empty"
        );
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        return linux::create(options).await;
        #[cfg(target_os = "windows")]
        return windows::create(options).await;
        #[cfg(target_os = "macos")]
        return macos::create(options).await;
        #[cfg(target_family = "wasm")]
        return web::create(options).await;
        #[cfg(not(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "windows",
            target_os = "macos",
            target_family = "wasm"
        )))]
        anyhow::bail!("system media sessions require a platform host");
    }
    pub fn update(&self, state: MediaSessionState) -> Result<()> {
        ensure!(
            state.volume.is_finite() && (0.0..=1.0).contains(&state.volume),
            "media volume must be in 0..=1"
        );
        ensure!(
            state.rate.is_finite() && state.rate > 0.,
            "media playback rate must be finite and positive"
        );
        ensure!(
            state.position.as_micros() <= i64::MAX as u128
                && state
                    .duration
                    .is_none_or(|d| d.as_micros() <= i64::MAX as u128),
            "media timeline exceeds the system range"
        );
        self.backend.update(state)
    }
    pub fn take_commands(&mut self) -> Option<async_channel::Receiver<MediaCommand>> {
        self.commands.take()
    }
    /// Publishes a discontinuous position change after a seek completes.
    pub fn seeked(&self, position: Duration) -> Result<()> {
        ensure!(
            position.as_micros() <= i64::MAX as u128,
            "media position exceeds the system range"
        );
        self.backend.seeked(position)
    }
}
