use gpui::{
    App, Context, FontWeight, HighlightStyle, Render, Window, WindowOptions, div, prelude::*, px,
    rgb,
};
use gpui_richtext::{
    CodeBlock, Contribution, Document, LinkSpan, Projection, Registry, RichText,
    compose::{
        code, emphasis, group, hard_break, link, mark, node, paragraph, strikethrough, strong,
        text, underline,
    },
};
use std::sync::Arc;

struct Label(&'static str);
struct Preview {
    document: Arc<Projection>,
    activation: String,
}

fn document() -> Arc<Projection> {
    let mut registry = Registry::with_standard_nodes();
    registry.register::<Label>(|label| {
        Ok(Contribution::Text(
            label.0.into(),
            HighlightStyle {
                color: Some(rgb(0x327b66).into()),
                font_weight: Some(FontWeight::BOLD),
                ..Default::default()
            },
        ))
    });
    let document =
        Document::from_roots([
            node(CodeBlock::new("const EXPOSURE: f32 = 0.8;\n\nfn calibrate(value: f32) -> f32 {\n    value * EXPOSURE\n}")
                .language("rust")
                .highlights([(0..5, HighlightStyle::color(rgb(0x8745a8).into()))])),
            paragraph()
                .child(text("Regular  ·  "))
                .child(strong().child(text("Bold")))
                .child(text("  ·  "))
                .child(emphasis().child(text("Italic")))
                .child(text("  ·  "))
                .child(strong().child(emphasis().child(text("Bold italic"))))
                .child(text("  ·  "))
                .child(code().child(text("Inline code"))),
            paragraph()
                .child(text("The restoration log records "))
                .child(strong().child(emphasis().child(text("each layer of pigment"))))
                .child(text(" without changing the original surface.")),
            paragraph()
                .child(node(Label("Inspection complete. ")))
                .child(mark().child(text("颜料样本已归档。")))
                .child(hard_break())
                .child(text("Reference: "))
                .child(code().child(text("pigment.sample_id")))
                .child(text(" — "))
                .child(underline().child(text("verified")))
                .child(text("; "))
                .child(strikethrough().child(text("pending"))),
            paragraph().child(text(
                "Select across the styled fragments and paragraphs, then copy the passage. \
             Resize the window to reflow the document.",
            )),
            group().child(paragraph().child(text("Related record: ")).child(
                link("record:conservation").child(strong().child(text("Conservation notes"))),
            )),
        ])
        .unwrap();
    Arc::new(registry.compile(&document).unwrap())
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xf1f3f5)).p_8().child(
            div()
                .id("document-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(
                    RichText::new("document", self.document.clone())
                        .on_link_click(cx.listener(|view, link: &LinkSpan, _, cx| {
                            view.activation = format!("Activated: {}", link.target);
                            cx.notify();
                        }))
                        .w_full()
                        .max_w(px(720.))
                        .mx_auto()
                        .p_6()
                        .rounded_lg()
                        .bg(rgb(0xffffff))
                        .text_color(rgb(0x28313b))
                        .text_size(px(20.))
                        .line_height(px(34.)),
                )
                .child(div().mt_4().child(self.activation.clone())),
        )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets::new())
        .run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| Preview {
                    document: document(),
                    activation: "Link activation is handled locally.".into(),
                })
            })
            .unwrap();
            cx.activate(true);
        });
}
