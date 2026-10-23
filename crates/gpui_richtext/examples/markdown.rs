use gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*, px, rgb};
use gpui_richtext::{
    CodeBlockLabels, CodeBlockStyle, Registry, TableStyle, TaskMarkerStyle, TextTheme, markdown,
};
use std::{sync::Arc, time::Duration};

struct Preview {
    stream: markdown::MarkdownStream,
    source: Arc<str>,
    scroll: gpui::ScrollHandle,
    task: Option<gpui::Task<()>>,
    error: Option<String>,
}

impl Preview {
    fn replay(&mut self, cx: &mut Context<Self>) {
        self.stream.clear();
        self.scroll.set_offset(Default::default());
        self.error = None;
        let source = self.source.clone();
        self.task = Some(cx.spawn(async move |view, cx| {
            let mut start = 0;
            for end in source
                .char_indices()
                .map(|(offset, _)| offset)
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
                    .items_center()
                    .justify_between()
                    .text_color(rgb(0x9daec2))
                    .child(self.error.clone().unwrap_or_else(|| {
                        if self.stream.is_finished() {
                            "Complete"
                        } else {
                            "Streaming Markdown…"
                        }
                        .into()
                    }))
                    .child(
                        div()
                            .id("replay")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x263344))
                            .cursor_pointer()
                            .child("Replay")
                            .on_click(cx.listener(|view, _, _, cx| view.replay(cx))),
                    ),
            )
            .child(
                div()
                    .id("scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        self.stream
                            .view("markdown")
                            .on_link_click(|link, _, cx| cx.open_url(&link.target))
                            .image_source(|image| {
                                std::path::PathBuf::from(image.source.as_ref()).into()
                            })
                            .quote_border_color(rgb(0x526176))
                            .selection_color(gpui::rgba(0x539bf555))
                            .table_style(TableStyle {
                                background: rgb(0x161b22).into(),
                                header_background: rgb(0x212936).into(),
                                alternate_background: Some(rgb(0x1b222c).into()),
                                border_color: rgb(0x303b4a).into(),
                                ..Default::default()
                            })
                            .code_block_style(CodeBlockStyle {
                                background: rgb(0x10161e).into(),
                                border_color: rgb(0x303b4a).into(),
                                header_color: rgb(0x9daec2).into(),
                                button_hover_background: rgb(0x263344).into(),
                                copy_success_color: rgb(0x56d6a0).into(),
                                ..Default::default()
                            })
                            .task_marker_style(TaskMarkerStyle {
                                border: rgb(0x6b7b90).into(),
                                checked_background: rgb(0x25866b).into(),
                                check: rgb(0xffffff).into(),
                            })
                            .code_block_labels(CodeBlockLabels {
                                copy: "Copy code".into(),
                                copied: "Copied".into(),
                            })
                            .w_full()
                            .max_w(px(760.))
                            .mx_auto()
                            .p_6()
                            .rounded_lg()
                            .bg(rgb(0x161b22))
                            .text_color(rgb(0xdce4ee))
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
    let mut theme = TextTheme::default();
    theme.link.highlight.color = Some(rgb(0x79b8ff).into());
    theme.code.highlight.background_color = Some(rgb(0x283343).into());
    let registry = Registry::with_theme(theme);
    #[cfg(feature = "syntax-highlighting")]
    let registry =
        registry.syntax_highlighter(Some(Arc::new(gpui_richtext::syntax::SyntaxHighlighter {
            theme: gpui_richtext::syntax::SyntaxTheme::Dark,
        })));
    let stream = markdown::MarkdownStream::new(registry);
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets::new())
        .run(move |cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    let mut view = Preview {
                        stream,
                        source: source.into(),
                        scroll: gpui::ScrollHandle::new(),
                        task: None,
                        error: None,
                    };
                    view.replay(cx);
                    view
                })
            })
            .unwrap();
            cx.activate(true);
        });
}
