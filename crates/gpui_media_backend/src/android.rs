use gpui_media_core::*;
use gpui_util::android::AndroidRuntime;
use jni::{
    JNIEnv, NativeMethod,
    objects::{GlobalRef, JByteBuffer, JClass, JString, JValue},
    sys::{jboolean, jint, jlong},
};
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicI64, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};

const CLASS: &str = "dev.gpui.android.MediaSession";
static REGISTERED: Mutex<bool> = Mutex::new(false);
static NEXT_ID: AtomicI64 = AtomicI64::new(1);
static SESSIONS: OnceLock<Mutex<HashMap<i64, Weak<Mutex<State>>>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<i64, Weak<Mutex<State>>>> {
    SESSIONS.get_or_init(Default::default)
}

fn error(error: impl std::fmt::Display) -> MediaError {
    MediaError::backend(format!("Android media: {error}"))
}

pub(super) fn initialize() -> MediaResult<()> {
    let runtime = AndroidRuntime::get().map_err(error)?;
    let mut registered = REGISTERED.lock().map_err(error)?;
    if !*registered {
        runtime
            .with_env(|env| {
                let class = runtime.load_class(env, CLASS)?;
                env.register_native_methods(
                    class,
                    &[
                        method(
                            "nativeState",
                            "(JJJJZIIIZZZ)V",
                            state_changed as *mut c_void,
                        ),
                        method(
                            "nativeFrame",
                            "(JJLjava/nio/ByteBuffer;IIJ)V",
                            frame as *mut c_void,
                        ),
                        method(
                            "nativeError",
                            "(JJILjava/lang/String;)V",
                            failed as *mut c_void,
                        ),
                        method(
                            "nativeSystemCommand",
                            "(JIJ)V",
                            system_command as *mut c_void,
                        ),
                    ],
                )?;
                Ok(())
            })
            .map_err(error)?;
        *registered = true;
    }
    Ok(())
}

fn method(name: &str, signature: &str, fn_ptr: *mut c_void) -> NativeMethod {
    NativeMethod {
        name: name.into(),
        sig: signature.into(),
        fn_ptr,
    }
}

pub(super) fn open_playback(
    request: MediaPlaybackRequest,
    output: MediaOutputSink,
) -> MediaResult<Box<dyn MediaPlaybackSession>> {
    Ok(Box::new(open_session(&request.source, Some(output), None)?))
}

fn validate_source(source: &MediaSource) -> MediaResult<()> {
    let network = source.network_options();
    if network.user_id().is_some()
        || network.user_password().is_some()
        || network.proxy().is_some()
        || network.retry_count().is_some()
        || network.retry_backoff_factor().is_some()
        || network.retry_backoff_max().is_some()
        || network.automatic_redirect().is_some()
        || network.keep_alive().is_some()
        || network.strict_tls().is_some()
        || network.buffer_duration().is_some()
        || network.buffer_size().is_some()
        || network.connection_speed_kbps().is_some()
        || network.progressive_download().is_some()
    {
        return Err(MediaError::unsupported(
            "Android media supports HTTP headers, user agent and timeout options",
        ));
    }
    Ok(())
}

struct ExtractionRequest {
    position: Duration,
    handle: FrameHandle,
    sequence: u64,
    response: SyncSender<MediaResult<Arc<VideoFrame>>>,
}

fn open_session(
    source: &MediaSource,
    output: Option<MediaOutputSink>,
    extraction: Option<ExtractionRequest>,
) -> MediaResult<AndroidSession> {
    initialize()?;
    validate_source(source)?;
    let network = source.network_options();
    let position = extraction.as_ref().map_or(-1, |request| {
        request.position.as_millis().min(i64::MAX as u128) as i64
    });
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let state = Arc::new(Mutex::new(State {
        output,
        handle: extraction
            .as_ref()
            .map_or_else(FrameHandle::new, |request| request.handle),
        sequence: extraction.as_ref().map_or(0, |request| request.sequence),
        extraction: extraction.map(|request| request.response),
        generation: 0,
        timeline: PlaybackTimeline::default(),
        playback: 0,
        reported_state: None,
        info: None,
        width: 0,
        height: 0,
        audio: false,
    }));
    sessions()
        .lock()
        .map_err(error)?
        .insert(id, Arc::downgrade(&state));
    let runtime = AndroidRuntime::get().map_err(error)?;
    let object = runtime.with_env(|env| {
        let class = runtime.load_class(env, CLASS)?;
        let uri = env.new_string(source.uri())?;
        let mut headers = network.headers().clone();
        if let Some(agent) = network.user_agent() {
            headers.retain(|key, _| !key.eq_ignore_ascii_case("User-Agent"));
            headers.insert("User-Agent".into(), agent.into());
        }
        let keys = env.new_object_array(headers.len() as i32, "java/lang/String", jni::objects::JObject::null())?;
        let values = env.new_object_array(headers.len() as i32, "java/lang/String", jni::objects::JObject::null())?;
        for (i, (key, value)) in headers.iter().enumerate() {
            let key = env.new_string(key)?;
            let value = env.new_string(value)?;
            env.set_object_array_element(&keys, i as i32, &key)?;
            env.set_object_array_element(&values, i as i32, &value)?;
            env.delete_local_ref(key)?;
            env.delete_local_ref(value)?;
        }
        let timeout = network.timeout().unwrap_or(Duration::from_secs(30)).as_millis().clamp(1, i32::MAX as u128) as i32;
        let object = env.new_object(class, "(Landroid/content/Context;JLjava/lang/String;[Ljava/lang/String;[Ljava/lang/String;IJ)V", &[
            JValue::Object(runtime.context()), JValue::Long(id), JValue::Object(&uri),
            JValue::Object(&keys), JValue::Object(&values), JValue::Int(timeout),
            JValue::Long(position),
        ])?;
        Ok(env.new_global_ref(object)?)
    });
    match object {
        Ok(object) => Ok(AndroidSession {
            id,
            state,
            object,
            volume: 1.,
            muted: false,
        }),
        Err(cause) => {
            if let Ok(mut sessions) = sessions().lock() {
                sessions.remove(&id);
            }
            Err(error(cause))
        }
    }
}

pub(super) fn open_frame_extractor(
    request: FrameExtractorBackendRequest,
) -> MediaResult<Box<dyn FrameExtractionSession>> {
    initialize()?;
    validate_source(&request.source)?;
    if request.video_decoder != VideoDecoderPolicy::Auto {
        return Err(MediaError::unsupported(
            "Android frame extraction requires automatic decoder selection",
        ));
    }
    if request.timeout.is_zero() {
        return Err(MediaError::invalid_input(
            "frame extraction timeout must be greater than zero",
        ));
    }
    Ok(Box::new(AndroidFrameExtractor {
        request,
        handle: FrameHandle::new(),
        sequence: 0,
    }))
}

struct AndroidFrameExtractor {
    request: FrameExtractorBackendRequest,
    handle: FrameHandle,
    sequence: u64,
}

impl FrameExtractionSession for AndroidFrameExtractor {
    fn initial_frame(&mut self) -> MediaResult<Arc<VideoFrame>> {
        self.frame_at(Duration::ZERO, SeekMode::Accurate)
    }

    fn frame_at(&mut self, position: Duration, mode: SeekMode) -> MediaResult<Arc<VideoFrame>> {
        if mode != SeekMode::Accurate {
            return Err(MediaError::unsupported(
                "Android frame extraction requires accurate seeking",
            ));
        }
        let started = Instant::now();
        let (response, receiver) = mpsc::sync_channel(1);
        self.sequence = self.sequence.wrapping_add(1);
        // A request owns its decoder, so late output after timeout cannot satisfy
        // another request, including a repeated request for the same timestamp.
        let _session = open_session(
            &self.request.source,
            None,
            Some(ExtractionRequest {
                position,
                response,
                handle: self.handle,
                sequence: self.sequence,
            }),
        )?;
        receiver
            .recv_timeout(self.request.timeout.saturating_sub(started.elapsed()))
            .map_err(|cause| match cause {
                mpsc::RecvTimeoutError::Timeout => {
                    MediaError::timeout("Android frame extraction timed out")
                }
                mpsc::RecvTimeoutError::Disconnected => {
                    MediaError::backend("Android frame extraction closed without a frame")
                }
            })?
    }
}

struct State {
    output: Option<MediaOutputSink>,
    extraction: Option<SyncSender<MediaResult<Arc<VideoFrame>>>>,
    handle: FrameHandle,
    sequence: u64,
    generation: i64,
    timeline: PlaybackTimeline,
    playback: i32,
    reported_state: Option<PlaybackState>,
    info: Option<Arc<MediaInfo>>,
    width: i32,
    height: i32,
    audio: bool,
}

impl State {
    fn emit(&mut self, event: MediaBackendEvent) {
        if let Some(output) = &self.output {
            output.emit(event);
        } else if let MediaBackendEvent::Error(error) = event
            && let Some(response) = self.extraction.take()
        {
            let _ = response.try_send(Err((*error).clone()));
        }
    }

    fn publish_frame(&mut self, frame: Arc<VideoFrame>) {
        if let Some(output) = &self.output {
            output.publish_video_frame(frame);
        } else if let Some(response) = self.extraction.take() {
            let _ = response.try_send(Ok(frame));
        }
    }
}

fn with_state(id: i64, generation: i64, operation: impl FnOnce(&mut State)) {
    let state = sessions()
        .lock()
        .ok()
        .and_then(|sessions| sessions.get(&id).and_then(Weak::upgrade));
    if let Some(state) = state {
        if let Ok(mut state) = state.lock() {
            if state.generation == generation {
                operation(&mut state);
            }
        }
    }
}

extern "system" fn system_command(
    _: JNIEnv,
    _: JClass,
    id: jlong,
    operation: jint,
    position: jlong,
) {
    let command = match operation {
        0 => SystemMediaCommand::Play,
        1 => SystemMediaCommand::Pause,
        2 => SystemMediaCommand::Stop,
        3 if position >= 0 => SystemMediaCommand::SeekTo(Duration::from_millis(position as u64)),
        _ => return,
    };
    let state = sessions()
        .lock()
        .ok()
        .and_then(|sessions| sessions.get(&id).and_then(Weak::upgrade));
    if let Some(state) = state {
        if let Ok(mut state) = state.lock() {
            state.emit(MediaBackendEvent::SystemCommand(command));
        }
    }
}

extern "system" fn state_changed(
    _: JNIEnv,
    _: JClass,
    id: jlong,
    generation: jlong,
    position: jlong,
    duration: jlong,
    seekable: jboolean,
    playback: jint,
    width: jint,
    height: jint,
    audio: jboolean,
    play_when_ready: jboolean,
    suppressed: jboolean,
) {
    with_state(id, generation, |state| {
        state.timeline = PlaybackTimeline::new(
            Duration::from_millis(position.max(0) as u64),
            (duration >= 0).then(|| Duration::from_millis(duration as u64)),
            seekable != 0,
        );
        if state.info.is_none()
            || state.width != width
            || state.height != height
            || state.audio != (audio != 0)
        {
            let mut info = MediaInfo::default();
            if width > 0 && height > 0 {
                info.video_streams.push(VideoStreamInfo {
                    id: "video".into(),
                    codec: None,
                    coded_size: Some(FrameSize::new(width, height)),
                    display_size: Some(FrameSize::new(width, height)),
                    frame_rate: None,
                    bitrate: None,
                    language: None,
                    selected: true,
                });
            }
            if audio != 0 {
                info.audio_streams.push(AudioStreamInfo {
                    id: "audio".into(),
                    codec: None,
                    channels: None,
                    sample_rate: None,
                    bitrate: None,
                    language: None,
                    title: None,
                    selected: true,
                });
            }
            let info = Arc::new(info);
            state.emit(MediaBackendEvent::MediaInfoChanged(info.clone()));
            state.info = Some(info);
            state.width = width;
            state.height = height;
            state.audio = audio != 0;
        }
        let playback_changed = state.playback != playback;
        if playback_changed {
            match playback {
                2 => {
                    state.emit(MediaBackendEvent::Buffering(0));
                }
                3 => {
                    state.emit(MediaBackendEvent::Buffering(100));
                    state.emit(MediaBackendEvent::Ready);
                }
                4 => {
                    state.emit(MediaBackendEvent::Ended);
                }
                _ => {}
            }
            state.playback = playback;
        }
        if state.output.is_some() && matches!(playback, 2 | 3) {
            let actual = if play_when_ready == 0 || suppressed != 0 {
                PlaybackState::Paused
            } else if playback == 2 {
                PlaybackState::Loading
            } else {
                PlaybackState::Playing
            };
            if playback_changed || state.reported_state.as_ref() != Some(&actual) {
                state.reported_state = Some(actual.clone());
                state.emit(MediaBackendEvent::PlaybackStateChanged(actual));
            }
        }
    });
}

extern "system" fn frame(
    env: JNIEnv,
    _: JClass,
    id: jlong,
    generation: jlong,
    bytes: JByteBuffer,
    width: jint,
    height: jint,
    timestamp: jlong,
) {
    with_state(id, generation, |state| {
        if state.output.is_none() && state.extraction.is_none() {
            return;
        }
        let result = (|| -> MediaResult<VideoFrame> {
            let len = (width as usize)
                .checked_mul(height as usize)
                .and_then(|n| n.checked_mul(4))
                .filter(|_| width > 0 && height > 0)
                .ok_or_else(|| MediaError::invalid_input("invalid Android video dimensions"))?;
            let capacity = env.get_direct_buffer_capacity(&bytes).map_err(error)?;
            if len > capacity {
                return Err(MediaError::invalid_input(
                    "Android video buffer is too short",
                ));
            }
            let pointer = env.get_direct_buffer_address(&bytes).map_err(error)?;
            // Java retains the direct buffer and does not reuse it until this callback returns.
            let pixels: Arc<[u8]> = unsafe { std::slice::from_raw_parts(pointer, len) }.into();
            state.sequence = state.sequence.wrapping_add(1);
            let size = FrameSize::new(width, height);
            let buffer = FrameBuffer::new(
                state.handle,
                state.sequence,
                size,
                FrameRect {
                    origin: Default::default(),
                    size,
                },
                size,
                PixelFormat::Rgba8,
                [FramePlane::new(pixels, width as u32 * 4)],
                Default::default(),
            )?;
            Ok(VideoFrame::new(
                Arc::new(buffer),
                Some(Duration::from_micros(timestamp.max(0) as u64)),
                None,
            ))
        })();
        match result {
            Ok(frame) => {
                state.publish_frame(Arc::new(frame));
            }
            Err(error) => {
                state.emit(MediaBackendEvent::Error(Arc::new(error)));
            }
        }
    });
}

extern "system" fn failed(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    generation: jlong,
    code: jint,
    message: JString,
) {
    let message = env
        .get_string(&message)
        .map(|s| String::from(s))
        .unwrap_or_else(|_| "media operation failed".into());
    with_state(id, generation, |state| {
        let kind = match code {
            10000 => MediaErrorKind::InvalidInput,
            10001 => MediaErrorKind::UnsupportedOperation,
            2001..=2004 | 2008 => MediaErrorKind::Network { status: None },
            2005 => MediaErrorKind::SourceNotFound,
            2006 => MediaErrorKind::Io {
                kind: std::io::ErrorKind::PermissionDenied,
            },
            2007 => MediaErrorKind::UnsupportedOperation,
            3001..=3004 => MediaErrorKind::UnsupportedContainer,
            4001..=4005 => MediaErrorKind::Decode,
            5001..=5004 => MediaErrorKind::AudioOutput,
            _ => MediaErrorKind::Backend,
        };
        let recovery = if code == 10000 || code == 10001 {
            MediaRecovery::None
        } else {
            MediaRecovery::ReloadSource
        };
        state.emit(MediaBackendEvent::Error(Arc::new(MediaError::new(
            kind, message, recovery,
        ))));
    });
}

struct AndroidSession {
    id: i64,
    state: Arc<Mutex<State>>,
    object: GlobalRef,
    volume: f64,
    muted: bool,
}

impl AndroidSession {
    fn command(&self, operation: i32, value: f64, advance: bool) -> MediaResult<()> {
        let generation = {
            let mut state = self.state.lock().map_err(error)?;
            state.reported_state = None;
            if advance {
                state.generation += 1;
                state.playback = 0;
            }
            state.generation
        };
        AndroidRuntime::get()
            .map_err(error)?
            .with_env(|env| {
                env.call_method(
                    self.object.as_obj(),
                    "command",
                    "(IDJ)V",
                    &[
                        JValue::Int(operation),
                        JValue::Double(value),
                        JValue::Long(generation),
                    ],
                )?;
                Ok(())
            })
            .map_err(error)
    }

    fn volume_changed(&self) {
        if let Err(error) = self.command(4, if self.muted { 0. } else { self.volume }, false) {
            if let Ok(mut state) = self.state.lock() {
                state.emit(MediaBackendEvent::Error(Arc::new(error)));
            }
        }
    }
}

impl MediaPlaybackSession for AndroidSession {
    fn manages_playback_state(&self) -> bool {
        true
    }

    fn capabilities(&self) -> MediaCapabilities {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        MediaCapabilities {
            video: state.width > 0,
            audio: state.audio,
            seeking: state.timeline.is_seekable(),
            accurate_seeking: true,
            frame_extraction: true,
            playback_rate: true,
            ..Default::default()
        }
    }
    fn play(&mut self) -> MediaResult<()> {
        self.command(0, 0., false)
    }
    fn pause(&mut self) -> MediaResult<()> {
        self.command(1, 0., false)
    }
    fn timeline(&self) -> PlaybackTimeline {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .timeline
    }
    fn reload(&mut self, autoplay: bool) -> MediaResult<()> {
        self.command(6, if autoplay { 1. } else { 0. }, true)
    }
    fn seek_to(&mut self, position: Duration, mode: SeekMode) -> MediaResult<()> {
        self.command(
            if mode == SeekMode::Accurate { 2 } else { 3 },
            position.as_millis().min(i64::MAX as u128) as f64,
            true,
        )
    }
    fn set_playback_rate(&mut self, rate: f64) -> MediaResult<()> {
        if !rate.is_finite() || rate <= 0. || rate > 8. {
            return Err(MediaError::invalid_input(
                "Android playback rate must be in (0, 8]",
            ));
        }
        self.command(5, rate, false)
    }
    fn set_volume(&mut self, volume: f64) {
        self.volume = if volume.is_finite() {
            volume.clamp(0., 1.)
        } else {
            0.
        };
        self.volume_changed();
    }
    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.volume_changed();
    }
    fn set_audio_focus_enabled(&mut self, enabled: bool) -> MediaResult<()> {
        self.command(7, if enabled { 1. } else { 0. }, false)
    }
    fn set_system_media_controls(
        &mut self,
        metadata: Option<SystemMediaMetadata>,
    ) -> MediaResult<()> {
        AndroidRuntime::get()
            .map_err(error)?
            .with_env(|env| {
                let title = metadata
                    .as_ref()
                    .map(|m| env.new_string(&m.title))
                    .transpose()?;
                let artist = metadata
                    .as_ref()
                    .and_then(|m| m.artist.as_ref())
                    .map(|s| env.new_string(s))
                    .transpose()?;
                let album = metadata
                    .as_ref()
                    .and_then(|m| m.album.as_ref())
                    .map(|s| env.new_string(s))
                    .transpose()?;
                let null = jni::objects::JObject::null();
                env.call_method(
                    self.object.as_obj(),
                    "setSystemControls",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
                    &[
                        JValue::Object(title.as_ref().map_or(&null, |s| s.as_ref())),
                        JValue::Object(artist.as_ref().map_or(&null, |s| s.as_ref())),
                        JValue::Object(album.as_ref().map_or(&null, |s| s.as_ref())),
                    ],
                )?;
                Ok(())
            })
            .map_err(error)
    }
    fn media_info(&self) -> Option<Arc<MediaInfo>> {
        self.state.lock().ok().and_then(|s| s.info.clone())
    }
    fn set_frame_transport_preference(
        &mut self,
        _: FrameTransportPreference,
    ) -> MediaResult<TransportChange> {
        Ok(TransportChange::Unchanged)
    }
}

impl Drop for AndroidSession {
    fn drop(&mut self) {
        if let Ok(mut sessions) = sessions().lock() {
            sessions.remove(&self.id);
        }
        if let Ok(runtime) = AndroidRuntime::get() {
            if let Err(error) = runtime.with_env(|env| {
                env.call_method(self.object.as_obj(), "close", "()V", &[])?;
                Ok(())
            }) {
                log::warn!("Android media release failed: {error}");
            }
        }
    }
}
