use std::{sync::Arc, time::Duration};

use crate::{MediaBackend, MediaPlayer, MediaPlayerEvent, PlaybackWakeMode, PreparedMediaPlayer};
use gpui::{AppContext, Context, Entity, EventEmitter, SharedString, Subscription};

use super::{
    source::AudioSource,
    worker::{AudioSession, AudioWorkerEvent},
};
use crate::{MediaError, MediaResult, PlaybackState, PlaybackTimeline, SeekMode};

/// Properties of the active decoded audio stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioInfo {
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) duration: Option<Duration>,
    pub(crate) seekable: bool,
    pub(crate) codec: SharedString,
}

impl AudioInfo {
    /// Sample rate, or zero when the backend does not expose it.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Channel count, or zero when the backend does not expose it.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn is_seekable(&self) -> bool {
        self.seekable
    }

    pub fn codec(&self) -> &str {
        &self.codec
    }
}

/// Initial behavior for an audio player.
pub use crate::MediaPlayerOptions as AudioPlayerOptions;

/// Configures an [`AudioPlayer`] before its playback session is opened.
pub struct AudioPlayerBuilder {
    source: AudioSource,
    options: AudioPlayerOptions,
    backend: Option<Arc<dyn MediaBackend>>,
}

impl AudioPlayerBuilder {
    pub fn options(mut self, options: AudioPlayerOptions) -> Self {
        self.options = options;
        self
    }

    /// Selects a media backend. Omitting this retains the built-in audio decoder.
    pub fn backend(mut self, backend: impl MediaBackend) -> Self {
        self.backend = Some(Arc::new(backend));
        self
    }

    pub fn shared_backend(mut self, backend: Arc<dyn MediaBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Opens a session without starting playback or creating a GPUI entity.
    /// Reopenable sources may use a backend; caller-fed streams use the built-in decoder.
    pub fn prepare(self) -> MediaResult<PreparedAudioPlayer> {
        let kind = if let Some(backend) = self.backend {
            PreparedAudioKind::Backend(
                MediaPlayer::shared_builder(self.source.media_source()?, backend)
                    .audio_only()
                    .options(self.options)
                    .prepare()?,
            )
        } else {
            let (sender, events) = async_channel::unbounded();
            let session = AudioSession::open(
                self.source.clone(),
                false,
                normalize_volume(self.options.volume),
                self.options.muted,
                sender,
            )?;
            PreparedAudioKind::Decoded { session, events }
        };
        Ok(PreparedAudioPlayer {
            source: self.source,
            options: self.options,
            kind,
        })
    }

    pub fn build(self, cx: &mut Context<AudioPlayer>) -> MediaResult<AudioPlayer> {
        Ok(self.prepare()?.build(cx))
    }
}

/// An audio session ready to attach to a GPUI entity. Dropping it releases the session.
pub struct PreparedAudioPlayer {
    source: AudioSource,
    options: AudioPlayerOptions,
    kind: PreparedAudioKind,
}

enum PreparedAudioKind {
    Decoded {
        session: AudioSession,
        events: async_channel::Receiver<AudioWorkerEvent>,
    },
    Backend(PreparedMediaPlayer),
}

impl PreparedAudioPlayer {
    pub fn build(self, cx: &mut Context<AudioPlayer>) -> AudioPlayer {
        AudioPlayer::from_prepared(self, cx)
    }
}

enum AudioPlayback {
    Decoded(AudioSession),
    Backend {
        player: Entity<MediaPlayer>,
        _subscription: Subscription,
    },
}

/// Events emitted by [`AudioPlayer`] for application-provided controls.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum AudioPlayerEvent {
    StateChanged(PlaybackState),
    TimelineChanged(PlaybackTimeline),
    InfoChanged(Arc<AudioInfo>),
    VolumeChanged { volume: f64, muted: bool },
    SystemMediaAction(gpui::gpui_notifications::MediaCommand),
    SystemMediaError(SharedString),
}

/// A cross-platform, presentation-free audio player.
///
/// By default, Symphonia owns demuxing/decoding and CPAL owns native device
/// output; Web uses browser audio playback. A selected backend uses the shared
/// media session layer with video disabled. The component
/// renders no UI; applications compose controls from the
/// public state, methods and events.
pub struct AudioPlayer {
    source: AudioSource,
    session: AudioPlayback,
    state: PlaybackState,
    timeline: PlaybackTimeline,
    info: Option<Arc<AudioInfo>>,
    play_when_ready: bool,
    volume: f64,
    muted: bool,
    playback_rate: f64,
}

impl AudioPlayer {
    pub fn builder(source: impl Into<AudioSource>) -> AudioPlayerBuilder {
        AudioPlayerBuilder {
            source: source.into(),
            options: AudioPlayerOptions::default(),
            backend: None,
        }
    }

    pub fn new(
        source: impl Into<AudioSource>,
        options: AudioPlayerOptions,
        cx: &mut Context<Self>,
    ) -> MediaResult<Self> {
        Self::builder(source).options(options).build(cx)
    }

    fn from_prepared(prepared: PreparedAudioPlayer, cx: &mut Context<Self>) -> Self {
        let PreparedAudioPlayer {
            source,
            options,
            kind,
        } = prepared;
        let (session, events) = match kind {
            PreparedAudioKind::Decoded { session, events } => (session, events),
            PreparedAudioKind::Backend(prepared) => {
                return Self::from_backend(source, options, prepared, cx);
            }
        };
        let volume = normalize_volume(options.volume);
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let Some(this) = this.upgrade() else {
                    break;
                };
                this.update(cx, |player, cx| match event {
                    AudioWorkerEvent::Ready(info) => {
                        let info = Arc::new(info);
                        player.info = Some(info.clone());
                        player.refresh_timeline(cx);
                        cx.emit(AudioPlayerEvent::InfoChanged(info));
                        #[cfg(target_os = "android")]
                        {
                            player.play_when_ready = player.decoded_session().is_playing();
                        }
                        player.set_state(
                            if player.play_when_ready {
                                PlaybackState::Playing
                            } else {
                                PlaybackState::Paused
                            },
                            cx,
                        );
                    }
                    #[cfg(target_os = "android")]
                    AudioWorkerEvent::Playing => {
                        if !player.decoded_session().is_playing() {
                            return;
                        }
                        player.play_when_ready = true;
                        player.set_state(
                            if player.info.is_some() {
                                PlaybackState::Playing
                            } else {
                                PlaybackState::Loading
                            },
                            cx,
                        );
                    }
                    AudioWorkerEvent::Paused => {
                        #[cfg(target_os = "android")]
                        if player.decoded_session().is_playing() {
                            return;
                        }
                        player.play_when_ready = false;
                        player.refresh_timeline(cx);
                        player.set_state(PlaybackState::Paused, cx);
                    }
                    AudioWorkerEvent::Ended => {
                        player.play_when_ready = false;
                        player.refresh_timeline(cx);
                        player.set_state(PlaybackState::Ended, cx);
                    }
                    AudioWorkerEvent::Error(error) => {
                        player.play_when_ready = false;
                        player.set_state(PlaybackState::Error(error), cx);
                    }
                });
            }
        })
        .detach();

        let timeline_update_interval = options
            .timeline_update_interval
            .max(Duration::from_millis(16));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(timeline_update_interval)
                    .await;
                let Some(this) = this.upgrade() else {
                    break;
                };
                this.update(cx, |player, cx| player.refresh_timeline(cx));
            }
        })
        .detach();

        let initial_error = if options.autoplay {
            session.play().err()
        } else {
            None
        };
        Self {
            source,
            session: AudioPlayback::Decoded(session),
            state: if let Some(error) = initial_error {
                PlaybackState::Error(Arc::new(error))
            } else if options.autoplay {
                PlaybackState::Loading
            } else {
                PlaybackState::Paused
            },
            timeline: PlaybackTimeline::default(),
            info: None,
            play_when_ready: options.autoplay,
            volume,
            muted: options.muted,
            playback_rate: 1.,
        }
    }

    fn from_backend(
        source: AudioSource,
        options: AudioPlayerOptions,
        prepared: PreparedMediaPlayer,
        cx: &mut Context<Self>,
    ) -> Self {
        let player = cx.new(|cx| prepared.build(cx));
        let subscription = cx.subscribe(&player, |this, player, event, cx| {
            match event {
                MediaPlayerEvent::SystemMediaAction(command) => {
                    cx.emit(AudioPlayerEvent::SystemMediaAction(command.clone()))
                }
                MediaPlayerEvent::SystemMediaError(error) => {
                    cx.emit(AudioPlayerEvent::SystemMediaError(error.clone()))
                }
                _ => {}
            }
            this.sync_backend(&player, cx);
        });
        let mut audio = Self {
            source,
            session: AudioPlayback::Backend {
                player: player.clone(),
                _subscription: subscription,
            },
            state: PlaybackState::Paused,
            timeline: PlaybackTimeline::default(),
            info: None,
            play_when_ready: options.autoplay,
            volume: normalize_volume(options.volume),
            muted: options.muted,
            playback_rate: 1.,
        };
        audio.sync_backend(&player, cx);
        audio
    }

    fn backend_player(&self) -> Option<Entity<MediaPlayer>> {
        match &self.session {
            AudioPlayback::Backend { player, .. } => Some(player.clone()),
            AudioPlayback::Decoded(_) => None,
        }
    }

    fn decoded_session(&self) -> &AudioSession {
        match &self.session {
            AudioPlayback::Decoded(session) => session,
            AudioPlayback::Backend { .. } => {
                unreachable!("backend controls use the shared media player")
            }
        }
    }

    fn sync_backend(&mut self, entity: &Entity<MediaPlayer>, cx: &mut Context<Self>) {
        let player = entity.read(cx);
        let state = player.state().clone();
        let timeline = player.timeline();
        let volume = player.volume();
        let muted = player.is_muted();
        let rate = player.playback_rate();
        let info = player
            .media_info()
            .and_then(|info| info.selected_audio_stream())
            .map(|stream| AudioInfo {
                sample_rate: stream.sample_rate.unwrap_or(0),
                channels: stream.channels.unwrap_or(0).min(u16::MAX as u32) as u16,
                codec: stream.codec.as_deref().unwrap_or("").to_owned().into(),
                duration: timeline.duration(),
                seekable: timeline.is_seekable(),
            })
            .or_else(|| {
                (state == PlaybackState::Playing || timeline.duration().is_some()).then(|| {
                    AudioInfo {
                        sample_rate: 0,
                        channels: 0,
                        codec: player.backend_name().to_owned().into(),
                        duration: timeline.duration(),
                        seekable: timeline.is_seekable(),
                    }
                })
            });
        self.playback_rate = rate;
        match state {
            PlaybackState::Playing | PlaybackState::Loading => self.play_when_ready = true,
            PlaybackState::Paused | PlaybackState::Ended | PlaybackState::Error(_) => {
                self.play_when_ready = false
            }
            PlaybackState::Seeking => {}
        }
        if self.info.as_deref() != info.as_ref() {
            self.info = info.map(Arc::new);
            if let Some(info) = &self.info {
                cx.emit(AudioPlayerEvent::InfoChanged(info.clone()));
            }
        }
        if self.timeline != timeline {
            self.timeline = timeline;
            cx.emit(AudioPlayerEvent::TimelineChanged(timeline));
            cx.notify();
        }
        if self.volume != volume || self.muted != muted {
            self.volume = volume;
            self.muted = muted;
            self.emit_volume(cx);
        }
        self.set_state(state, cx);
    }

    pub fn playback_rate(&self) -> f64 {
        self.playback_rate
    }

    /// Controls focus when supported by the selected backend.
    pub fn set_audio_focus_enabled(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> MediaResult<()> {
        match self.backend_player() {
            Some(player) => player.update(cx, |player, _| player.set_audio_focus_enabled(enabled)),
            None => self.decoded_session().set_audio_focus_enabled(enabled),
        }
    }

    /// Controls playback power management on supported output platforms.
    pub fn set_wake_mode(
        &mut self,
        mode: PlaybackWakeMode,
        cx: &mut Context<Self>,
    ) -> MediaResult<()> {
        match self.backend_player() {
            Some(player) => player.update(cx, |player, _| player.set_wake_mode(mode)),
            None => self.decoded_session().set_wake_mode(mode),
        }
    }

    pub fn source(&self) -> &AudioSource {
        &self.source
    }

    pub fn state(&self) -> &PlaybackState {
        &self.state
    }

    pub fn timeline(&self) -> PlaybackTimeline {
        self.timeline
    }

    pub fn position(&self) -> Duration {
        self.timeline.position()
    }

    pub fn duration(&self) -> Option<Duration> {
        self.timeline.duration()
    }

    pub fn is_seekable(&self) -> bool {
        match &self.session {
            AudioPlayback::Decoded(session) => session.is_seekable(),
            AudioPlayback::Backend { .. } => self.timeline.is_seekable(),
        }
    }

    pub fn info(&self) -> Option<&AudioInfo> {
        self.info.as_deref()
    }

    pub fn volume(&self) -> f64 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn play(&mut self, cx: &mut Context<Self>) -> MediaResult<()> {
        if let Some(player) = self.backend_player() {
            let result = player.update(cx, |player, cx| player.play(cx));
            self.sync_backend(&player, cx);
            return result;
        }
        if self.state == PlaybackState::Ended {
            if !self.is_seekable() {
                return Err(MediaError::unsupported(
                    "an ended live audio stream cannot be restarted",
                ));
            }
            self.decoded_session()
                .seek_to(Duration::ZERO, SeekMode::Accurate)?;
            self.timeline = PlaybackTimeline::new(
                Duration::ZERO,
                self.timeline.duration(),
                self.timeline.is_seekable(),
            );
            cx.emit(AudioPlayerEvent::TimelineChanged(self.timeline));
            self.set_state(PlaybackState::Seeking, cx);
        } else if self.info.is_none() {
            self.set_state(PlaybackState::Loading, cx);
        } else {
            self.set_state(PlaybackState::Playing, cx);
        }
        self.play_when_ready = true;
        if let Err(error) = self.decoded_session().play() {
            self.play_when_ready = false;
            self.set_state(PlaybackState::Paused, cx);
            return Err(error);
        }
        Ok(())
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.backend_player() {
            if let Err(error) = player.update(cx, |player, cx| player.pause(cx)) {
                self.set_state(PlaybackState::Error(Arc::new(error)), cx);
            } else {
                self.sync_backend(&player, cx);
            }
            return;
        }
        self.play_when_ready = false;
        self.decoded_session().pause();
        self.set_state(PlaybackState::Paused, cx);
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> MediaResult<()> {
        match self.state {
            PlaybackState::Playing | PlaybackState::Loading => {
                self.pause(cx);
                Ok(())
            }
            PlaybackState::Paused
            | PlaybackState::Seeking
            | PlaybackState::Ended
            | PlaybackState::Error(_) => self.play(cx),
        }
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) -> MediaResult<()> {
        if let Some(player) = self.backend_player() {
            let result = player.update(cx, |player, cx| player.stop(cx));
            self.sync_backend(&player, cx);
            return result;
        }
        self.play_when_ready = false;
        self.decoded_session().pause();
        self.seek_to(Duration::ZERO, SeekMode::Accurate, cx)?;
        Ok(())
    }

    pub fn seek_to(
        &mut self,
        position: Duration,
        mode: SeekMode,
        cx: &mut Context<Self>,
    ) -> MediaResult<()> {
        if let Some(player) = self.backend_player() {
            let result = player.update(cx, |player, cx| player.seek_to(position, mode, cx));
            self.sync_backend(&player, cx);
            return result;
        }
        let position = self
            .timeline
            .duration()
            .map_or(position, |duration| position.min(duration));
        self.decoded_session().seek_to(position, mode)?;
        self.timeline = PlaybackTimeline::new(
            position,
            self.timeline.duration(),
            self.timeline.is_seekable(),
        );
        self.set_state(PlaybackState::Seeking, cx);
        cx.emit(AudioPlayerEvent::TimelineChanged(self.timeline));
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f64, cx: &mut Context<Self>) {
        if let Some(player) = self.backend_player() {
            player.update(cx, |player, cx| player.set_volume(volume, cx));
            self.sync_backend(&player, cx);
            return;
        }
        self.volume = normalize_volume(volume);
        self.decoded_session().set_volume(self.volume);
        self.emit_volume(cx);
    }

    pub fn set_muted(&mut self, muted: bool, cx: &mut Context<Self>) {
        if let Some(player) = self.backend_player() {
            player.update(cx, |player, cx| player.set_muted(muted, cx));
            self.sync_backend(&player, cx);
            return;
        }
        self.muted = muted;
        self.decoded_session().set_muted(muted);
        self.emit_volume(cx);
    }

    pub fn toggle_muted(&mut self, cx: &mut Context<Self>) {
        self.set_muted(!self.muted, cx);
    }

    pub fn refresh_timeline(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.backend_player() {
            player.update(cx, |player, cx| player.refresh_timeline(cx));
            self.sync_backend(&player, cx);
            return;
        }
        let timeline = self.decoded_session().timeline();
        if timeline != self.timeline {
            self.timeline = timeline;
            cx.emit(AudioPlayerEvent::TimelineChanged(timeline));
            cx.notify();
        }
    }

    fn emit_volume(&self, cx: &mut Context<Self>) {
        cx.emit(AudioPlayerEvent::VolumeChanged {
            volume: self.volume,
            muted: self.muted,
        });
        cx.notify();
    }

    fn set_state(&mut self, state: PlaybackState, cx: &mut Context<Self>) {
        if self.state != state {
            self.state = state.clone();
            cx.emit(AudioPlayerEvent::StateChanged(state));
            cx.notify();
        }
    }
}

impl EventEmitter<AudioPlayerEvent> for AudioPlayer {}

fn normalize_volume(volume: f64) -> f64 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MediaBackendEvent, MediaCapabilities, MediaOutputSink, MediaPlaybackMode,
        MediaPlaybackRequest, MediaPlaybackSession,
    };
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct Backend {
        output: Arc<Mutex<Option<MediaOutputSink>>>,
        commands: Arc<Mutex<Vec<&'static str>>>,
        closed: Arc<std::sync::atomic::AtomicUsize>,
    }

    struct Session(Backend);
    impl Drop for Session {
        fn drop(&mut self) {
            self.0
                .closed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl MediaBackend for Backend {
        fn name(&self) -> &'static str {
            "audio-lifecycle"
        }
        fn open_playback(
            &self,
            request: MediaPlaybackRequest,
            output: MediaOutputSink,
        ) -> MediaResult<Box<dyn MediaPlaybackSession>> {
            assert_eq!(request.mode, MediaPlaybackMode::AudioOnly);
            assert!(request.output_capabilities.is_none());
            *self.output.lock().unwrap() = Some(output);
            Ok(Box::new(Session(self.clone())))
        }
    }
    impl MediaPlaybackSession for Session {
        fn capabilities(&self) -> MediaCapabilities {
            MediaCapabilities {
                audio: true,
                ..Default::default()
            }
        }
        fn play(&mut self) -> MediaResult<()> {
            self.0.commands.lock().unwrap().push("play");
            Ok(())
        }
        fn pause(&mut self) -> MediaResult<()> {
            self.0.commands.lock().unwrap().push("pause");
            Ok(())
        }
        fn timeline(&self) -> PlaybackTimeline {
            PlaybackTimeline::new(Duration::ZERO, Some(Duration::from_secs(60)), true)
        }
        fn seek_to(&mut self, _: Duration, _: SeekMode) -> MediaResult<()> {
            self.0.commands.lock().unwrap().push("seek");
            Ok(())
        }
    }

    #[gpui::test]
    fn backend_audio_prepares_without_playing_and_becomes_ready_without_video(
        cx: &mut gpui::TestAppContext,
    ) {
        let backend = Backend::default();
        let prepare = {
            let backend = backend.clone();
            std::thread::spawn(move || {
                AudioPlayer::builder(
                    AudioSource::from_uri("https://example.test/radio.opus").unwrap(),
                )
                .backend(backend)
                .prepare()
                .unwrap()
            })
        };
        let prepared = prepare.join().unwrap();
        assert!(backend.commands.lock().unwrap().is_empty());
        let output = backend.output.lock().unwrap().clone().unwrap();
        output.emit(MediaBackendEvent::Ready);
        let audio = cx.new(|cx| prepared.build(cx));
        cx.run_until_parked();
        assert_eq!(
            audio.read_with(cx, |p, _| p.state().clone()),
            PlaybackState::Playing
        );
        audio.update(cx, |p, cx| {
            p.pause(cx);
            p.seek_to(Duration::from_secs(4), SeekMode::Accurate, cx)
                .unwrap();
        });
        output.emit(MediaBackendEvent::Ready);
        cx.run_until_parked();
        assert_eq!(
            audio.read_with(cx, |p, _| p.state().clone()),
            PlaybackState::Paused
        );
        assert_eq!(*backend.commands.lock().unwrap(), ["play", "pause", "seek"]);
        output.emit(MediaBackendEvent::Error(Arc::new(MediaError::backend(
            "source closed",
        ))));
        cx.run_until_parked();
        assert!(audio.read_with(cx, |p, _| matches!(p.state(), PlaybackState::Error(_))));
    }

    #[test]
    fn discarded_audio_preparation_releases_the_session_without_starting_it() {
        let backend = Backend::default();
        let prepared =
            AudioPlayer::builder(AudioSource::from_uri("https://example.test/radio.opus").unwrap())
                .backend(backend.clone())
                .prepare()
                .unwrap();
        drop(prepared);
        assert!(backend.commands.lock().unwrap().is_empty());
        assert_eq!(backend.closed.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
