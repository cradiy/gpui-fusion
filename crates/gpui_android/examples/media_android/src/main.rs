use gpui::{prelude::*, *};
use gpui_media::{MediaSource, SeekMode, VideoPlayer, VideoPlayerEvent};
use std::time::Duration;

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| MediaDemo {
                player: None,
                file: None,
                subscription: None,
                status: "Choose a video or audio file.".into(),
            })
        })
        .expect("open media window");
    });
}

struct MediaDemo {
    player: Option<Entity<VideoPlayer>>,
    file: Option<gpui::gpui_io::FileHandle>,
    subscription: Option<Subscription>,
    status: String,
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
                    let player = cx.new(|cx| {
                        VideoPlayer::builder(source, gpui_media_backend::SystemBackend)
                            .build(cx)
                            .expect("create media session")
                    });
                    this.subscription = Some(cx.subscribe(&player, |_, _, event, cx| {
                        if let VideoPlayerEvent::StateChanged(state) = event {
                            log_state(state);
                        }
                        cx.notify();
                    }));
                    this.player = Some(player);
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

    fn control(&mut self, action: usize, cx: &mut Context<Self>) {
        if action == 5 {
            self.subscription = None;
            self.player = None;
            self.file = None;
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
}

fn log_state(state: &gpui_media::PlaybackState) {
    eprintln!("media playback: {state:?}");
}

impl Render for MediaDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        column.child(
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
                            .on_click(cx.listener(move |this, _, _, cx| this.control(index, cx)))
                    }),
            ),
        )
    }
}
