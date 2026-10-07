use gpui::{prelude::*, *};
use gpui_media::{
    MediaSource, SeekMode, VideoFrameExtractor, VideoPlayer, VideoPlayerEvent, VideoSurface,
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
            });
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
}

impl MediaDemo {
    fn choose(&mut self, cx: &mut Context<Self>) {
        let selection = cx.prompt_for_files(FilePromptOptions::default());
        cx.spawn(async move |this, cx| {
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
                this.update(cx, |this, cx| -> anyhow::Result<()> {
                    gpui_media_backend::SystemBackend::initialize()?;
                    let extractor = VideoFrameExtractor::new(
                        source.clone(),
                        Arc::new(gpui_media_backend::SystemBackend),
                    )?;
                    let player = cx.new(|cx| {
                        VideoPlayer::builder(source.clone(), gpui_media_backend::SystemBackend)
                            .build(cx)
                            .expect("create media session")
                    });
                    this.system_controls = false;
                    this.subscription = Some(cx.subscribe(&player, |_, _, event, cx| {
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

    fn toggle_system_controls(&mut self, cx: &mut Context<Self>) {
        if self.system_controls_pending {
            return;
        }
        let Some(player) = self.player.clone() else {
            return;
        };
        if self.system_controls {
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
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.system_controls_pending = false;
                if this.player.as_ref() != Some(&player) {
                    return;
                }
                match result {
                    Ok(session) => {
                        let options = gpui_media::VideoSystemMediaOptions {
                            metadata: gpui::gpui_notifications::MediaMetadata {
                                track_id: this
                                    .file
                                    .as_ref()
                                    .map_or("Media", |file| file.name())
                                    .to_owned(),
                                title: this
                                    .file
                                    .as_ref()
                                    .map_or("Media", |file| file.name())
                                    .to_owned(),
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        player.update(cx, |player, cx| {
                            player.set_system_media_session(Some(session), options, cx)
                        });
                        this.system_controls = true;
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
                    .on_click(cx.listener(|this, _, _, cx| this.choose(cx))),
            )
            .child(div().text_sm().child(self.status.clone()));
        if let Some(player) = &self.player {
            let timeline = player.read(cx).timeline();
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
                        .h(px(240.))
                        .w_full()
                        .flex_shrink_0()
                        .bg(rgb(0x000000))
                        .child(player.clone()),
                )
                .child(format!(
                    "{:.1}s / {:.1}s · {:?}",
                    timeline.position().as_secs_f64(),
                    timeline.duration().unwrap_or_default().as_secs_f64(),
                    player.read(cx).state()
                ));
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
        column
    }
}
