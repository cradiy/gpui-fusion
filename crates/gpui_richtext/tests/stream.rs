#![cfg(feature = "markdown")]
use gpui_richtext::{
    Registry,
    markdown::{self, MarkdownStream},
};
use std::sync::Arc;

#[test]
fn chunk_boundaries_do_not_change_final_markdown_or_reference_resolution() {
    let source = "## Δ values\n\nRead **carefully** at [index].\n\n| Name | Value |\n| --- | ---: |\n| λ | 8 |\n\n```rust\nlet value = \"海\";\n```\n\n[index]: https://example.org/catalog\n";
    let expected = Registry::with_standard_nodes()
        .compile(&markdown::parse(source).unwrap())
        .unwrap();
    for chunk_size in [1, 7, 31] {
        let mut stream = MarkdownStream::new(Registry::with_standard_nodes());
        let mut pending = String::new();
        for (index, character) in source.chars().enumerate() {
            pending.push(character);
            if (index + 1) % chunk_size == 0 {
                stream.push_str(&pending).unwrap();
                pending.clear();
                stream.flush().unwrap();
            }
        }
        stream.push_str(&pending).unwrap();
        let final_view = stream.finish().unwrap();
        assert_eq!(final_view.text(), expected.text());
        assert_eq!(
            format!("{:?}", final_view.blocks()),
            format!("{:?}", expected.blocks())
        );
        assert_eq!(
            format!("{:?}", final_view.spans()),
            format!("{:?}", expected.spans())
        );
        assert_eq!(
            format!("{:?}", final_view.links()),
            format!("{:?}", expected.links())
        );
        assert!(Arc::ptr_eq(&final_view, &stream.flush().unwrap()));
        assert!(stream.push_str("more").is_err());
        stream.clear();
        stream.push_str("Fresh document").unwrap();
        assert_eq!(stream.finish().unwrap().text().as_ref(), "Fresh document");
    }
}

#[test]
fn failed_flush_keeps_published_content_and_pending_source() {
    let mut registry = Registry::with_standard_nodes();
    registry.register::<gpui_richtext::Text>(|text| {
        anyhow::ensure!(text.0.as_ref() != "reject", "rejected payload");
        Ok(gpui_richtext::Contribution::Text(
            text.0.clone(),
            Default::default(),
        ))
    });
    let mut stream = MarkdownStream::new(registry);
    stream.push_str("Ready\n\n").unwrap();
    let published = stream.flush().unwrap();
    stream.push_str("reject").unwrap();
    assert!(stream.finish().is_err());
    assert!(!stream.is_finished());
    assert!(Arc::ptr_eq(&published, stream.projection()));
    assert_eq!(stream.source(), "Ready\n\nreject");
    stream.push_str("ed input").unwrap();
    assert_eq!(
        stream.finish().unwrap().text().as_ref(),
        "Ready\n\nrejected input"
    );
}

#[gpui::test]
fn published_suffix_updates_preserve_selection_but_prefix_changes_clear_it(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::{Context, Render, VisualTestContext, Window, div, point, prelude::*, px, size};
    struct Page(MarkdownStream);
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                self.0
                    .view("stream")
                    .w_full()
                    .text_size(px(16.))
                    .line_height(px(24.)),
            )
        }
    }
    let mut stream = MarkdownStream::new(Registry::with_standard_nodes());
    stream.push_str("[chapter]").unwrap();
    stream.flush().unwrap();
    let window = cx.open_window(size(px(300.), px(300.)), |_, _| Page(stream));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(5.), px(10.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    window
        .update(&mut visual.cx, |page, _, cx| {
            page.0
                .push_str("\n\nMore text that wraps when the window narrows.")
                .unwrap();
            page.0.flush().unwrap();
            cx.notify();
        })
        .unwrap();
    visual.simulate_resize(size(px(120.), px(300.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some("[chapter]")
    );
    window
        .update(&mut visual.cx, |page, _, cx| {
            page.0
                .push_str("\n\n[chapter]: https://example.org/chapter\n")
                .unwrap();
            page.0.finish().unwrap();
            cx.notify();
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("unchanged".into()));
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some("unchanged")
    );
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_ne!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some("unchanged")
    );
    window
        .update(&mut visual.cx, |page, _, cx| {
            let source = page.0.source().to_owned();
            page.0.clear();
            page.0.push_str(&source).unwrap();
            page.0.finish().unwrap();
            cx.notify();
        })
        .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("unchanged".into()));
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some("unchanged")
    );
}
