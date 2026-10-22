use gpui::{
    Context, FontWeight, HighlightStyle, Render, TestAppContext, VisualTestContext, Window, div,
    point, prelude::*, px, size,
};
use gpui_richtext::{
    Document, Node, NodeId, Paragraph, Projection, Registry, RichText, Span, Text,
};
use std::sync::Arc;

struct Page(Arc<Projection>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            RichText::new("passage", self.0.clone())
                .w_full()
                .text_size(px(16.))
                .line_height(px(24.)),
        )
    }
}

#[gpui::test]
fn selected_document_copies_across_nodes_after_reflow_and_ignores_typing(cx: &mut TestAppContext) {
    let document = Document::new(
        [
            Node::new(NodeId(1), Paragraph, vec![NodeId(2), NodeId(3)]),
            Node::new(NodeId(2), Text("Quartz samples: ".into()), vec![]),
            Node::new(
                NodeId(3),
                Span(HighlightStyle::from(FontWeight::BOLD)),
                vec![NodeId(4)],
            ),
            Node::new(NodeId(4), Text("紫色 🔬".into()), vec![]),
            Node::new(NodeId(5), Paragraph, vec![NodeId(6)]),
            Node::new(
                NodeId(6),
                Text("Stored in the northern cabinet.".into()),
                vec![],
            ),
        ],
        vec![NodeId(1), NodeId(5)],
    )
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected = projection.text().to_string();
    let window = cx.open_window(size(px(500.), px(600.)), move |_, _| Page(projection));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(5.), px(10.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_resize(size(px(140.), px(600.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some(expected.clone())
    );

    visual.simulate_input("discarded mutation");
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some(expected.as_str())
    );
}
