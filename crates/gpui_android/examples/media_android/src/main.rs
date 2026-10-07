use gpui::gpui_notifications::MediaArtwork;
use gpui::{prelude::*, *};
use gpui_media::{
    MediaSource, MediaStreamId, PlaybackBuffer, PlaybackWakeMode, SeekMode, SubtitleCue,
    SubtitleEvent, VideoFrameExtractor, VideoPlayer, VideoPlayerEvent, VideoSurface,
    video_container,
};
use std::{sync::Arc, time::Duration};

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|_| MediaDemo {
                player: None,
                file: None,
                subscription: None,
                status: "Choose a video or audio file.".into(),
                extractor: None,
                thumbnail: VideoSurface::new(),
                thumbnail_position: Duration::ZERO,
                thumbnail_request: 0,
                system_controls: false,
                system_controls_pending: false,
                background_task: None,
                wake_mode: PlaybackWakeMode::None,
                artwork: None,
                artwork_request: 0,
                subtitles: Vec::new(),
                picture_in_picture_pending: false,
                scroll_handle: ScrollHandle::new(),
                video_bounds: ElementBounds::default(),
            });
            window.set_picture_in_picture_source(Some(view.read(cx).video_bounds.clone()));
            window.on_system_back(
                cx,
                window.handler_for(&view, |_, window, cx| {
                    if window.is_fullscreen() {
                        window.toggle_fullscreen();
                    }
                    window.set_back_enabled(false);
                    cx.notify();
                }),
            );
            let weak = view.downgrade();
            window.on_picture_in_picture_changed(cx, move |enabled, _, cx| {
                eprintln!("picture-in-picture: {enabled}");
                let _ = weak.update(cx, |_, cx| cx.notify());
            });
            view
        })
        .expect("open media window");
    });
}

struct MediaDemo {
    player: Option<Entity<VideoPlayer>>,
    file: Option<gpui::gpui_io::FileHandle>,
    subscription: Option<Subscription>,
    status: String,
    extractor: Option<VideoFrameExtractor>,
    thumbnail: VideoSurface,
    thumbnail_position: Duration,
    thumbnail_request: u64,
    system_controls: bool,
    system_controls_pending: bool,
    background_task: Option<Task<()>>,
    wake_mode: PlaybackWakeMode,
    artwork: Option<MediaArtwork>,
    artwork_request: u64,
    subtitles: Vec<(MediaStreamId, Arc<SubtitleCue>)>,
    picture_in_picture_pending: bool,
    scroll_handle: ScrollHandle,
    video_bounds: ElementBounds,
}

impl MediaDemo {
    fn video_size(&self, cx: &App) -> Size<u32> {
        self.player
            .as_ref()
            .and_then(|player| player.read(cx).media_info())
            .and_then(|info| info.video_streams.iter().find(|stream| stream.selected))
            .and_then(|stream| stream.display_size.or(stream.coded_size))
            .filter(|size| size.width > 0 && size.height > 0)
            .map(|size| gpui::size(size.width as u32, size.height as u32))
            .unwrap_or(size(16, 9))
    }

    fn enter_picture_in_picture(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.picture_in_picture_pending {
            return;
        }
        let request = window.enter_picture_in_picture(self.video_size(cx), cx);
        self.picture_in_picture_pending = true;
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.picture_in_picture_pending = false;
                if let Err(error) = result {
                    this.status = format!("Picture-in-picture: {error:#}");
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, window: &Window, cx: &mut Context<Self>) {
        let selection = cx.prompt_for_files(FilePromptOptions::default());
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let Some(files) = selection.await?? else {
                    return Ok::<_, anyhow::Error>(());
                };
                let Some(file) = files.into_iter().next() else {
                    return Ok(());
                };
                let source = if let Some(path) = file.path() {
                    MediaSource::from_path(path)?
                } else if let Some(uri) = file.url() {
                    MediaSource::parse(uri)?
                } else {
                    anyhow::bail!("The file provider does not expose a playable URL");
                };
                this.update_in(cx, |this, window, cx| -> anyhow::Result<()> {
                    gpui_media_backend::SystemBackend::initialize()?;
                    let extractor = VideoFrameExtractor::new(
                        source.clone(),
                        Arc::new(gpui_media_backend::SystemBackend),
                    )?;
                    let player = cx.new(|cx| {
                        VideoPlayer::builder(source.clone(), gpui_media_backend::SystemBackend)
                            .build_in_window(window, cx)
                            .expect("create media session")
                    });
                    this.system_controls = false;
                    this.wake_mode = PlaybackWakeMode::None;
                    this.artwork = None;
                    this.artwork_request += 1;
                    this.background_task = None;
                    this.subtitles.clear();
                    this.subscription = Some(cx.subscribe(&player, |this, _, event, cx| {
                        if let VideoPlayerEvent::Subtitle(event) = event {
                            match event {
                                SubtitleEvent::Reset => this.subtitles.clear(),
                                SubtitleEvent::Cue { stream_id, cue } => {
                                    this.subtitles.push((stream_id.clone(), cue.clone()))
                                }
                                _ => {}
                            }
                        }
                        if let VideoPlayerEvent::StateChanged(state) = event {
                            log_state(state);
                        }
                        cx.notify();
                    }));
                    this.player = Some(player);
                    this.extractor = Some(extractor);
                    this.thumbnail.clear();
                    this.thumbnail_position = Duration::ZERO;
                    this.thumbnail_request += 1;
                    this.status = file.name().to_owned();
                    this.file = Some(file);
                    cx.notify();
                    Ok(())
                })??;
                Ok(())
            }
            .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.status = error.to_string();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn extract(&mut self, first: bool, cx: &mut Context<Self>) {
        let Some(extractor) = self.extractor.clone() else {
            return;
        };
        let file = self.file.clone();
        let position = if first {
            Duration::ZERO
        } else {
            self.thumbnail_position + Duration::from_secs(5)
        };
        self.thumbnail_request += 1;
        let request = self.thumbnail_request;
        self.status = format!("Extracting a frame at {:.1}s…", position.as_secs_f64());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = extractor.frame_at_latest(position).await;
            // Keep the document's access alive until its independent request completes.
            drop(file);
            let _ = this.update(cx, |this, cx| {
                if this.thumbnail_request != request {
                    return;
                }
                let result = result.and_then(|frame| {
                    this.thumbnail.set_frame(&frame)?;
                    this.thumbnail_position = position;
                    Ok(frame.timestamp().unwrap_or_default())
                });
                this.status = match result {
                    Ok(timestamp) => format!(
                        "Thumbnail at {:.3}s; playback position unchanged",
                        timestamp.as_secs_f64()
                    ),
                    Err(error) => error.to_string(),
                };
                eprintln!("media extraction: {}", this.status);
                cx.notify();
            });
        })
        .detach();
    }

    fn control(&mut self, action: usize, cx: &mut Context<Self>) {
        if action == 5 {
            self.artwork = None;
            self.artwork_request += 1;
            self.background_task = None;
            self.system_controls = false;
            self.subscription = None;
            self.player = None;
            self.file = None;
            self.extractor = None;
            self.thumbnail.clear();
            self.thumbnail_request += 1;
            self.status = "Choose a video or audio file.".into();
            cx.notify();
            return;
        }
        let Some(player) = &self.player else {
            return;
        };
        let result = player.update(cx, |player, cx| match action {
            0 => player.play(cx),
            1 => player.pause(cx),
            2 => player.seek_to(
                player.timeline().target_after(Duration::from_secs(2)),
                SeekMode::Accurate,
                cx,
            ),
            3 => player.seek_to(Duration::ZERO, SeekMode::Accurate, cx),
            _ => player.reload(true, cx),
        });
        if let Err(error) = result {
            self.status = error.to_string();
        }
        cx.notify();
    }

    fn select_audio(&mut self, id: &gpui_media::MediaStreamId, cx: &mut Context<Self>) {
        if let Some(player) = &self.player
            && let Err(error) = player.update(cx, |player, cx| player.select_audio_stream(id, cx))
        {
            self.status = error.to_string();
            cx.notify();
        }
    }

    fn cycle_subtitle(&mut self, cx: &mut Context<Self>) {
        let Some(player) = &self.player else {
            return;
        };
        let next = player.read(cx).media_info().and_then(|info| {
            let next = info
                .subtitle_streams
                .iter()
                .position(|track| track.selected)
                .map_or(0, |index| index + 1);
            info.subtitle_streams
                .get(next)
                .map(|track| track.id.clone())
        });
        if let Err(error) = player.update(cx, |player, cx| {
            player.select_subtitle_stream(next.as_ref(), cx)
        }) {
            self.status = error.to_string();
            cx.notify();
        }
    }

    fn choose_artwork(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.player.clone() else {
            return;
        };
        self.artwork_request += 1;
        let revision = self.artwork_request;
        let selection = cx.prompt_for_files(FilePromptOptions::default());
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<MediaArtwork>> = async {
                let Some(file) = selection.await??.and_then(|files| files.into_iter().next())
                else {
                    return Ok(None);
                };
                let bytes = file.read_limited(16 * 1024 * 1024).await?;
                executor
                    .spawn(async move { MediaArtwork::from_encoded(&bytes).map(Some) })
                    .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.artwork_request != revision || this.player.as_ref() != Some(&player) {
                    return;
                }
                match result {
                    Ok(Some(artwork)) => {
                        match player.update(cx, |player, _| {
                            player.set_system_media_artwork(Some(artwork.clone()))
                        }) {
                            Ok(()) => {
                                this.artwork = Some(artwork);
                                this.status = "Cover ready for system media controls.".into();
                            }
                            Err(error) => this.status = error.to_string(),
                        }
                    }
                    Ok(None) => {}
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clear_artwork(&mut self, cx: &mut Context<Self>) {
        self.artwork_request += 1;
        if let Some(player) = &self.player {
            match player.update(cx, |player, _| player.set_system_media_artwork(None)) {
                Ok(()) => {
                    self.artwork = None;
                    self.status = "Cover cleared.".into();
                }
                Err(error) => self.status = error.to_string(),
            }
        }
        cx.notify();
    }

    fn cycle_wake_mode(&mut self, cx: &mut Context<Self>) {
        let Some(player) = &self.player else { return };
        let mode = match self.wake_mode {
            PlaybackWakeMode::None => PlaybackWakeMode::Local,
            PlaybackWakeMode::Local => PlaybackWakeMode::Network,
            PlaybackWakeMode::Network => PlaybackWakeMode::None,
        };
        match player.update(cx, |player, _| player.set_wake_mode(mode)) {
            Ok(()) => self.wake_mode = mode,
            Err(error) => self.status = error.to_string(),
        }
        cx.notify();
    }

    fn toggle_system_controls(&mut self, cx: &mut Context<Self>) {
        if self.system_controls_pending {
            return;
        }
        let Some(player) = self.player.clone() else {
            return;
        };
        if self.system_controls {
            self.background_task = None;
            player.update(cx, |player, cx| {
                player.set_system_media_session(None, Default::default(), cx)
            });
            self.system_controls = false;
            cx.notify();
            return;
        }
        let request = cx.system_media_session(gpui::gpui_notifications::MediaSessionOptions {
            app_id: "dev.gpui.media".into(),
            app_name: "GPUI Media".into(),
        });
        self.system_controls_pending = true;
        let title = self
            .file
            .as_ref()
            .map_or("Media", |file| file.name())
            .to_owned();
        let artwork = self.artwork.clone();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<_> = async {
                let session = request.await?;
                let options = gpui_media::VideoSystemMediaOptions {
                    artwork,
                    metadata: gpui::gpui_notifications::MediaMetadata {
                        track_id: title.clone(),
                        title,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                #[cfg(target_os = "android")]
                let background = {
                    session.update(gpui::gpui_notifications::MediaSessionState {
                        metadata: options.metadata.clone(),
                        ..Default::default()
                    })?;
                    Some(session.start_background_playback().await?)
                };
                #[cfg(not(target_os = "android"))]
                let background: Option<
                    gpui::gpui_notifications::BackgroundPlayback,
                > = None;
                Ok((session, options, background))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.system_controls_pending = false;
                if this.player.as_ref() != Some(&player) {
                    return;
                }
                match result {
                    Ok((session, mut options, background)) => {
                        options.artwork = this.artwork.clone();
                        player.update(cx, |player, cx| {
                            player.set_system_media_session(Some(session), options, cx)
                        });
                        this.system_controls = true;
                        if let Some(mut background) = background {
                            let player_id = player.entity_id();
                            this.background_task = Some(cx.spawn(async move |this, cx| {
                                let reason = background.stopped().await;
                                let _ = this.update(cx, |this, cx| {
                                    if let Some(player) = this
                                        .player
                                        .clone()
                                        .filter(|player| player.entity_id() == player_id)
                                    {
                                        player.update(cx, |player, cx| {
                                            let _ = player.pause(cx);
                                            player.set_system_media_session(
                                                None,
                                                Default::default(),
                                                cx,
                                            );
                                        });
                                        this.system_controls = false;
                                        this.status =
                                            format!("Background playback stopped: {reason:?}");
                                        this.background_task = None;
                                        cx.notify();
                                    }
                                });
                            }));
                        }
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn log_state(state: &gpui_media::PlaybackState) {
    eprintln!("media playback: {state:?}");
}

impl Render for MediaDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fullscreen = window.is_fullscreen();
        let mut column = div()
            .id("media-demo")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .gap_4()
            .p_5()
            .bg(rgb(0x101923))
            .text_color(rgb(0xe6edf5))
            .child(div().text_2xl().child("Media playback"))
            .child(
                div()
                    .id("fullscreen")
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(0x30475c))
                    .child(if fullscreen {
                        "Exit fullscreen"
                    } else {
                        "Fullscreen"
                    })
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.toggle_fullscreen();
                        window.set_back_enabled(window.is_fullscreen());
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("choose")
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(0x30475c))
                    .child("Choose media")
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
            )
            .child(div().text_sm().child(self.status.clone()));
        if let Some(player) = &self.player {
            let timeline = player.read(cx).timeline();
            let selected_subtitle = player
                .read(cx)
                .media_info()
                .and_then(|info| info.selected_subtitle_stream());
            let subtitle_text = self
                .subtitles
                .iter()
                .filter(|(id, cue)| {
                    selected_subtitle.is_some_and(|track| &track.id == id)
                        && cue.start <= timeline.position()
                        && timeline.position() < cue.end
                })
                .map(|(_, cue)| cue.text.as_ref())
                .collect::<Vec<_>>()
                .join("\n");
            if window.is_picture_in_picture() {
                return div()
                    .size_full()
                    .bg(rgb(0x000000))
                    .child(video_with_subtitles(player.clone(), subtitle_text))
                    .into_any_element();
            }
            let video_size = self.video_size(cx).map(|value| DevicePixels(value as i32));
            let video_bounds_handle = self.video_bounds.clone();
            if player
                .read(cx)
                .media_info()
                .is_some_and(|info| !info.subtitle_streams.is_empty())
            {
                let label = selected_subtitle
                    .map(|track| {
                        track
                            .title
                            .as_deref()
                            .or(track.language.as_deref())
                            .unwrap_or("On")
                    })
                    .unwrap_or("Off");
                column = column.child(
                    div()
                        .id("subtitle-track")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x30475c))
                        .child(format!("Subtitles: {label}"))
                        .on_click(cx.listener(|this, _, _, cx| this.cycle_subtitle(cx))),
                );
            }
            if let Some(info) = player
                .read(cx)
                .media_info()
                .filter(|info| !info.audio_streams.is_empty())
            {
                column = column.child(div().text_sm().child("Audio tracks")).child(
                    div().flex().flex_wrap().gap_2().children(
                        info.audio_streams.iter().enumerate().map(|(index, track)| {
                            let id = track.id.clone();
                            let label = format!(
                                "{}{} · {} · {} Hz",
                                if track.selected { "✓ " } else { "" },
                                track.title.as_deref().unwrap_or("Audio"),
                                track.language.as_deref().unwrap_or("Unknown language"),
                                track.sample_rate.unwrap_or_default()
                            );
                            div()
                                .id(("audio-track", index))
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(if track.selected { 0x35628a } else { 0x30475c }))
                                .child(label)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.select_audio(&id, cx)),
                                )
                        }),
                    ),
                );
            }
            column = column
                .child(
                    div()
                        .id("system-controls")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x30475c))
                        .child(if self.system_controls {
                            "Disable system controls"
                        } else {
                            "Enable system controls"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_system_controls(cx))),
                )
                .child(
                    div()
                        .id("wake-mode")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x30475c))
                        .child(match self.wake_mode {
                            PlaybackWakeMode::None => "Wake: Off",
                            PlaybackWakeMode::Local => "Wake: CPU",
                            PlaybackWakeMode::Network => "Wake: CPU + Wi-Fi",
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.cycle_wake_mode(cx))),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("choose-cover")
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(0x30475c))
                                .child("Choose cover")
                                .on_click(cx.listener(|this, _, _, cx| this.choose_artwork(cx))),
                        )
                        .child(
                            div()
                                .id("clear-cover")
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(0x30475c))
                                .child("Clear cover")
                                .on_click(cx.listener(|this, _, _, cx| this.clear_artwork(cx))),
                        ),
                )
                .child(
                    div()
                        .h(px(240.))
                        .w_full()
                        .relative()
                        .flex_shrink_0()
                        .bg(rgb(0x000000))
                        .child(video_with_subtitles(player.clone(), subtitle_text))
                        .child(
                            canvas(
                                move |bounds, window, _| {
                                    let video_bounds =
                                        ObjectFit::Contain.get_bounds(bounds, video_size);
                                    window.track_element_bounds(&video_bounds_handle, video_bounds);
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                        ),
                )
                .child(format!(
                    "{:.1}s / {:.1}s · {:?}",
                    timeline.position().as_secs_f64(),
                    timeline.duration().unwrap_or_default().as_secs_f64(),
                    player.read(cx).state()
                ))
                .child(match player.read(cx).buffered() {
                    PlaybackBuffer::Unknown => "Buffered: unavailable".to_owned(),
                    PlaybackBuffer::Position(end) => {
                        format!("Buffered to {:.1}s", end.as_secs_f64())
                    }
                    PlaybackBuffer::Ranges(ranges) if ranges.is_empty() => {
                        "Buffered: none".to_owned()
                    }
                    PlaybackBuffer::Ranges(ranges) => format!(
                        "Buffered: {}",
                        ranges
                            .iter()
                            .map(|range| {
                                format!(
                                    "{:.1}–{:.1}s",
                                    range.start.as_secs_f64(),
                                    range.end.as_secs_f64()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            if window.supports_picture_in_picture() {
                column = column.child(
                    div()
                        .id("picture-in-picture")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x30475c))
                        .child(if self.picture_in_picture_pending {
                            "Opening picture-in-picture…"
                        } else {
                            "Picture-in-picture"
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.enter_picture_in_picture(window, cx)
                        })),
                );
            }
        }
        column = column
            .child(
                div().flex().flex_wrap().gap_2().children(
                    ["Play", "Pause", "Seek +2s", "Restart", "Reload", "Close"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            div()
                                .id(("control", index))
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(0x30475c))
                                .child(label)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.control(index, cx)),
                                )
                        }),
                ),
            )
            .child(
                div().flex().gap_2().children(
                    ["First frame", "Frame +5s"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            div()
                                .id(("extract", index))
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(0x30475c))
                                .child(label)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.extract(index == 0, cx)),
                                )
                        }),
                ),
            );
        if let Some(frame) = self.thumbnail.surface() {
            column = column.child(
                div()
                    .h(px(140.))
                    .w_full()
                    .flex_shrink_0()
                    .relative()
                    .child(surface(frame.clone()).absolute().size_full()),
            );
        }
        column.into_any_element()
    }
}

fn video_with_subtitles(player: Entity<VideoPlayer>, text: String) -> impl IntoElement {
    video_container(player).when(!text.is_empty(), |video| {
        video.child(
            div()
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .p_1()
                .rounded_md()
                .bg(rgba(0x000000b0))
                .text_sm()
                .text_color(rgb(0xffffff))
                .text_center()
                .child(text),
        )
    })
}
