use gpui::{
    App, Context, FontWeight, HighlightStyle, Render, Window, WindowOptions, div, prelude::*, px,
    rgb,
};
use gpui_richtext::{
    Contribution, Document, Node, NodeId, Paragraph, Projection, Registry, RichText, Span, Text,
};
use std::sync::Arc;

struct Label(&'static str);
struct Preview {
    document: Arc<Projection>,
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
    let nodes = [
        Node::new(NodeId(1), Paragraph, vec![NodeId(2), NodeId(3), NodeId(5)]),
        Node::new(
            NodeId(2),
            Text("The restoration log records ".into()),
            vec![],
        ),
        Node::new(
            NodeId(3),
            Span(HighlightStyle::from(FontWeight::BOLD)),
            vec![NodeId(4)],
        ),
        Node::new(NodeId(4), Text("each layer of pigment".into()), vec![]),
        Node::new(
            NodeId(5),
            Text(" without changing the original surface.".into()),
            vec![],
        ),
        Node::new(NodeId(6), Paragraph, vec![NodeId(7), NodeId(8)]),
        Node::new(NodeId(7), Label("Inspection complete. "), vec![]),
        Node::new(
            NodeId(8),
            Text("颜料样本已归档。 Notes: café, e\u{301}, 🪁. שלום עולם.".into()),
            vec![],
        ),
        Node::new(NodeId(9), Paragraph, vec![NodeId(10)]),
        Node::new(
            NodeId(10),
            Text(
                concat!(
                    "Select across the styled fragments and paragraphs, then copy the passage. ",
                    "Resize the window to reflow the document.",
                )
                .into(),
            ),
            vec![],
        ),
    ];
    let document = Document::new(nodes, vec![NodeId(1), NodeId(6), NodeId(9)]).unwrap();
    Arc::new(registry.compile(&document).unwrap())
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xf1f3f5)).p_8().child(
            div()
                .id("document-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(
                    RichText::new("document", self.document.clone())
                        .w_full()
                        .max_w(px(720.))
                        .mx_auto()
                        .p_6()
                        .rounded_lg()
                        .bg(rgb(0xffffff))
                        .text_color(rgb(0x28313b))
                        .text_size(px(20.))
                        .line_height(px(34.)),
                ),
        )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| Preview {
                document: document(),
            })
        })
        .unwrap();
        cx.activate(true);
    });
}
