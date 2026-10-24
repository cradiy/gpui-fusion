use gpui::{
    Context, FontWeight, HighlightStyle, Render, TestAppContext, VisualTestContext, Window, div,
    point, prelude::*, px, size,
};
use gpui_richtext::{
    Document, Node, NodeId, Paragraph, Projection, Registry, RichText, Span, Text,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct Page(Arc<Projection>, Rc<RefCell<Vec<String>>>);

#[gpui::test]
fn structured_blocks_keep_markers_and_unicode_selection_after_reflow(cx: &mut TestAppContext) {
    use gpui_richtext::{HeadingLevel, compose::*};
    let document = Document::from_roots([
        heading(HeadingLevel::H2).child(text("Route overview")),
        quote().child(text("北岸")),
        bullet_item().child(link_with_text("port:beta", "Dock β")),
        numbered_item(4).child(text("Inspect ropes")),
        task_item(true).child(text("Confirm departure")),
    ])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected = "Route overview\n\n北岸\n\n• Dock β\n4. Inspect ropes\n[x] Confirm departure";
    assert_eq!(projection.text().as_ref(), expected);
    let link = &projection.links()[0];
    assert_eq!(&projection.text()[link.range.clone()], "Dock β");
    assert_eq!(
        projection.offset(link.node, "Dock ".len()),
        Some(link.range.start + "Dock ".len())
    );
    assert_eq!(projection.offset(link.node, "Dock ".len() + 1), None);
    let window = cx.open_window(size(px(400.), px(600.)), move |_, _| {
        Page(projection, Rc::default())
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(20.), px(20.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_resize(size(px(120.), px(600.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some(expected)
    );
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let activations = self.1.clone();
        div().size_full().child(
            RichText::new("passage", self.0.clone())
                .w_full()
                .on_link_click(move |link, _, _| {
                    activations.borrow_mut().push(link.target.to_string())
                })
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
    let window = cx.open_window(size(px(500.), px(600.)), move |_, _| {
        Page(projection, Rc::default())
    });
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

#[gpui::test]
fn link_click_dispatches_but_selection_drag_only_copies(cx: &mut TestAppContext) {
    use gpui::MouseButton;
    use gpui_richtext::compose::*;
    let document = Document::from_roots([paragraph()
        .child(link("app:reference").child(strong().child(text("Browse documentation"))))
        .child(text(" after reading."))])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected = projection.text().to_string();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let events = activations.clone();
    let window = cx.open_window(size(px(500.), px(300.)), move |_, _| {
        Page(projection, events)
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    let start = point(px(5.), px(10.));
    let end = point(px(100.), px(10.));
    visual.simulate_click(start, Default::default());
    assert_eq!(&*activations.borrow(), &["app:reference"]);
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
    assert_eq!(activations.borrow().len(), 1);
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    let selected = visual
        .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
        .unwrap();
    assert!(!selected.is_empty());
    assert!(expected.contains(&selected));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some(expected)
    );
}

#[gpui::test]
fn code_block_copies_exact_source_with_syntax_styles(cx: &mut TestAppContext) {
    use gpui_richtext::{CodeBlock, compose::node};
    let source = "let count = 12;\n\n    count / 3\n";
    let document = Document::from_roots([node(
        CodeBlock::new(source).highlights([(0..3, HighlightStyle::from(FontWeight::BOLD))]),
    )])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let window = cx.open_window(size(px(160.), px(400.)), move |_, _| {
        Page(projection, Rc::default())
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(20.), px(52.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some(source)
    );
    visual
        .update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".into())));
    visual.simulate_click(point(px(130.), px(28.)), Default::default());
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some(source)
    );
}

#[gpui::test]
fn code_headers_copy_individual_blocks_and_do_not_enter_document_selection(
    cx: &mut TestAppContext,
) {
    use gpui_richtext::{CodeBlock, compose::*};
    let source = "\tready()\n";
    let document = Document::from_roots([
        paragraph().child(text("Commands")),
        node(CodeBlock::new("雪 = 7;").language("text")),
        node(CodeBlock::new(source).language("shell")),
    ])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected = projection.text().to_string();
    let window = cx.open_window(size(px(500.), px(400.)), move |_, _| {
        Page(projection, Rc::default())
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_resize(size(px(240.), px(400.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(214.), px(142.)), Default::default());
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some(source)
    );
    visual.simulate_click(point(px(20.), px(20.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some(expected)
    );
}
