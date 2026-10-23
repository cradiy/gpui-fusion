use gpui::{
    App, Context, FontWeight, HighlightStyle, Render, Window, WindowOptions, div, prelude::*, px,
    rgb,
};
use gpui_richtext::{
    CodeBlock, Contribution, Document, HeadingLevel, LinkSpan, Projection, Registry, RichText,
    compose::{
        bullet_item, code, emphasis, group, hard_break, heading, link, mark, node, numbered_item,
        paragraph, quote, strikethrough, strong, task_item, text, underline,
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
            heading(HeadingLevel::H1).child(text("Observation journal")),
            quote().child(text("A useful observation records uncertainty as carefully as the result.")),
            heading(HeadingLevel::H3).child(text("Before the session")),
            bullet_item().child(text("Check the weather forecast.")),
            bullet_item().child(text("Allow the optics to reach ambient temperature.")),
            heading(HeadingLevel::H3).child(text("Capture sequence")),
            numbered_item(1).child(text("Align the mount.")),
            numbered_item(2).child(text("Record a reference frame.")),
            heading(HeadingLevel::H3).child(text("Equipment checklist")),
            task_item(true).child(text("Battery charged")),
            task_item(false).child(text("Storage verified")),
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
                        .max_w(px(760.))
                        .mx_auto()
                        .p_8()
                        .rounded_xl()
                        .border_1()
                        .border_color(rgb(0xe2e8f0))
                        .bg(rgb(0xffffff))
                        .text_color(rgb(0x28313b))
                        .text_size(px(17.))
                        .line_height(px(29.)),
                )
                .child(
                    div()
                        .max_w(px(760.))
                        .mx_auto()
                        .mt_4()
                        .text_size(px(12.))
                        .text_color(rgb(0x64748b))
                        .child(self.activation.clone()),
                ),
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
