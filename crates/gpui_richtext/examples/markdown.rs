use gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*, px, rgb};
use gpui_richtext::{
    CodeBlockLabels, CodeBlockStyle, Projection, Registry, RichText, TableStyle, TaskMarkerStyle,
    TextTheme, markdown,
};
use std::sync::Arc;

struct Preview(Arc<Projection>);
impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x0d1117)).p_6().child(
            div().id("scroll").size_full().overflow_y_scroll().child(
                RichText::new("markdown", self.0.clone())
                    .on_link_click(|link, _, cx| cx.open_url(&link.target))
                    .image_source(|image| std::path::PathBuf::from(image.source.as_ref()).into())
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
    let document = markdown::parse(&source).unwrap();
    let mut theme = TextTheme::default();
    theme.link.highlight.color = Some(rgb(0x79b8ff).into());
    theme.code.highlight.background_color = Some(rgb(0x283343).into());
    let registry = Registry::with_theme(theme);
    #[cfg(feature = "syntax-highlighting")]
    let registry =
        registry.syntax_highlighter(Some(Arc::new(gpui_richtext::syntax::SyntaxHighlighter {
            theme: gpui_richtext::syntax::SyntaxTheme::Dark,
        })));
    let projection = Arc::new(registry.compile(&document).unwrap());
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets::new())
        .run(move |cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| Preview(projection))
            })
            .unwrap();
            cx.activate(true);
        });
}
