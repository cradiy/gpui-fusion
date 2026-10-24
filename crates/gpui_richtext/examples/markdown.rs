use gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*, px, rgb};
use gpui_richtext::{CodeBlockLabels, RichTextTheme, markdown};
use std::{sync::Arc, time::Duration};

struct Preview {
    theme: RichTextTheme,
    stream: markdown::MarkdownStream,
    source: Arc<str>,
    scroll: gpui::ScrollHandle,
    task: Option<gpui::Task<()>>,
    error: Option<String>,
    offset: usize,
    paused: bool,
}

impl Preview {
    fn replay(&mut self, cx: &mut Context<Self>) {
        self.stream.clear();
        self.scroll.set_offset(Default::default());
        self.error = None;
        self.offset = 0;
        self.resume(cx);
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        self.task.take();
        self.paused = true;
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        self.paused = false;
        let source = self.source.clone();
        let offset = self.offset;
        self.task = Some(cx.spawn(async move |view, cx| {
            let mut start = offset;
            for end in source[offset..]
                .char_indices()
                .map(|(index, _)| index + offset)
                .skip(1)
                .step_by(8)
                .chain(std::iter::once(source.len()))
            {
                cx.background_executor()
                    .timer(Duration::from_millis(45))
                    .await;
                let result = view.update(cx, |view, cx| {
                    let follow =
                        (view.scroll.offset().y + view.scroll.max_offset().y).abs() < px(2.);
                    let result = view
                        .stream
                        .push_str(&source[start..end])
                        .and_then(|_| view.stream.flush());
                    view.offset = end;
                    view.error = result.err().map(|error| error.to_string());
                    if end == source.len() {
                        if let Err(error) = view.stream.finish() {
                            view.error = Some(error.to_string());
                        }
                    }
                    if follow {
                        view.scroll.scroll_to_bottom();
                    }
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
                start = end;
            }
        }));
        cx.notify();
    }
}
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let at_bottom = (self.scroll.offset().y + self.scroll.max_offset().y).abs() < px(2.);
        if at_bottom {
            self.scroll.scroll_to_bottom();
        }
        div()
            .size_full()
            .bg(rgb(0x0d1117))
            .p_6()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .mx_auto()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .text_color(rgb(0x9daec2))
                    .child(self.error.clone().unwrap_or_else(|| {
                        if self.paused {
                            "Paused"
                        } else if self.stream.is_finished() {
                            "Complete"
                        } else {
                            "Streaming Markdown…"
                        }
                        .into()
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .when(!at_bottom, |controls| {
                                controls.child(
                                    div()
                                        .id("latest")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(0x263344))
                                        .hover(|style| style.bg(rgb(0x34475e)))
                                        .active(|style| style.bg(rgb(0x405570)))
                                        .cursor_pointer()
                                        .child("Latest")
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.scroll.scroll_to_bottom();
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(!self.stream.is_finished(), |controls| {
                                controls.child(
                                    div()
                                        .id("pause")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(0x263344))
                                        .hover(|style| style.bg(rgb(0x34475e)))
                                        .active(|style| style.bg(rgb(0x405570)))
                                        .cursor_pointer()
                                        .child(if self.paused { "Resume" } else { "Pause" })
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            if view.paused {
                                                view.resume(cx);
                                            } else {
                                                view.pause(cx);
                                            }
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .id("replay")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x263344))
                                    .hover(|style| style.bg(rgb(0x34475e)))
                                    .active(|style| style.bg(rgb(0x405570)))
                                    .cursor_pointer()
                                    .child("Replay")
                                    .on_click(cx.listener(|view, _, _, cx| view.replay(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .id("scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                    .child(
                        self.stream
                            .view("markdown")
                            .on_link_click(|link, _, cx| cx.open_url(&link.target))
                            .image_source(|image| {
                                std::path::PathBuf::from(image.source.as_ref()).into()
                            })
                            .theme(&self.theme)
                            .code_block_labels(CodeBlockLabels {
                                copy: "Copy code".into(),
                                copied: "Copied".into(),
                            })
                            .w_full()
                            .max_w(px(760.))
                            .mx_auto()
                            .p_6()
                            .rounded_lg()
                            .text_size(px(16.))
                            .line_height(px(26.)),
                    ),
            )
    }
}

fn main() {
    let source = include_str!("markdown.md").replace(
        "IMAGE_SOURCE",
        &format!(
            "{}/../gpui/examples/image/app-icon.png",
            env!("CARGO_MANIFEST_DIR")
        ),
    );
    let mut theme = RichTextTheme::dark();
    for (kind, title) in [
        (gpui_richtext::AlertKind::Note, "Note"),
        (gpui_richtext::AlertKind::Tip, "Tip"),
        (gpui_richtext::AlertKind::Important, "Important"),
        (gpui_richtext::AlertKind::Warning, "Warning"),
        (gpui_richtext::AlertKind::Caution, "Caution"),
    ] {
        theme.alert_mut(kind).title = Some(title.into());
    }
    let registry = theme.registry();
    let stream = markdown::MarkdownStream::new(registry);
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets::new())
        .run(move |cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    let mut view = Preview {
                        theme,
                        stream,
                        source: source.into(),
                        scroll: gpui::ScrollHandle::new(),
                        task: None,
                        error: None,
                        offset: 0,
                        paused: false,
                    };
                    view.replay(cx);
                    view
                })
            })
            .unwrap();
            cx.activate(true);
        });
}

#[gpui::test]
fn pause_and_resume_do_not_drop_or_duplicate_streamed_text(cx: &mut gpui::TestAppContext) {
    let source: Arc<str> = "## Telemetry\n\n**温度** arrives in several packets.\n".into();
    let expected = source.clone();
    let preview = cx.new(|_| Preview {
        theme: RichTextTheme::dark(),
        stream: markdown::MarkdownStream::new(RichTextTheme::dark().registry()),
        source,
        scroll: gpui::ScrollHandle::new(),
        task: None,
        error: None,
        offset: 0,
        paused: false,
    });
    preview.update(cx, |view, cx| view.replay(cx));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(45));
    cx.run_until_parked();
    let paused_at = preview.update(cx, |view, cx| {
        assert!(view.offset > 0 && view.offset < view.source.len());
        view.pause(cx);
        view.stream.source().to_owned()
    });
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    preview.update(cx, |view, cx| {
        assert_eq!(view.stream.source(), paused_at);
        view.resume(cx);
    });
    cx.run_until_parked();
    for _ in 0..expected.chars().count() {
        cx.executor().advance_clock(Duration::from_millis(45));
        cx.run_until_parked();
    }
    preview.update(cx, |view, _| {
        assert_eq!(view.stream.source(), expected.as_ref());
        assert!(view.stream.is_finished());
        assert!(view.error.is_none());
    });
}
