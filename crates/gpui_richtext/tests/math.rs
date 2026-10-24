#![cfg(feature = "math")]

use gpui::{
    Context, Render, StyledText, TestAppContext, TextInlineBox, TextLayout, VisualTestContext,
    Window, div, point, prelude::*, px, size,
};
use gpui_richtext::{Document, MathInline, Registry, RichText, compose::*};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct InlineLayoutPage(Rc<RefCell<Option<TextLayout>>>);

impl Render for InlineLayoutPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let text = StyledText::new("Lead $object$ ending").with_inline_boxes(vec![TextInlineBox {
            range: 5..13,
            size: size(px(78.), px(46.)),
            baseline: px(34.),
        }]);
        *self.0.borrow_mut() = Some(text.layout().clone());
        div()
            .w_full()
            .text_size(px(16.))
            .line_height(px(24.))
            .child(text)
    }
}

#[gpui::test]
fn inline_object_reserves_width_baseline_and_atomic_wrap_boundaries(cx: &mut TestAppContext) {
    let layout = Rc::new(RefCell::new(None));
    let shared = layout.clone();
    let window = cx.open_window(size(px(400.), px(400.)), move |_, _| {
        InlineLayoutPage(shared)
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    for width in [400., 110.] {
        visual.simulate_resize(size(px(width), px(400.)));
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let layout = layout.borrow();
        let layout = layout.as_ref().unwrap();
        let line = layout.line_layout_for_index(5).unwrap();
        let unwrapped = &line.unwrapped_layout;
        assert_eq!(
            unwrapped.x_for_index(13) - unwrapped.x_for_index(5),
            px(78.)
        );
        for boundary in &line.wrap_boundaries {
            let offset = unwrapped.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index;
            assert!(!(6..13).contains(&offset));
        }
        let top = layout.position_for_index(5).unwrap();
        let baseline = layout.baseline_for_index(5).unwrap();
        assert!(baseline.y - px(34.) >= top.y);
        let hit = layout.index_for_position(top + point(px(10.), px(10.)));
        assert!(matches!(hit, Ok(5) | Ok(13)));
        if width < 200. {
            assert!(!line.wrap_boundaries.is_empty());
            let selected = line.selection_bounds(0..line.len(), px(24.));
            assert!(selected.iter().any(|bounds| bounds.size.height == px(24.)));
            assert_eq!(
                selected
                    .iter()
                    .filter(|bounds| bounds.size.height >= px(46.))
                    .count(),
                1
            );
            for bounds in selected {
                let hit = layout.index_for_position(
                    layout.bounds().origin + bounds.origin + point(px(1.), px(1.)),
                );
                let index = hit.unwrap_or_else(|index| index);
                assert_eq!(
                    layout.position_for_index(index).unwrap().y,
                    layout.bounds().top() + bounds.top()
                );
            }
        }
    }
}

struct MathPage(Arc<gpui_richtext::Projection>);

impl Render for MathPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            RichText::new("math-copy", self.0.clone())
                .w_full()
                .text_size(px(18.))
                .line_height(px(26.)),
        )
    }
}

#[gpui::test]
fn rendered_inline_formula_preserves_source_copy_after_resize(cx: &mut TestAppContext) {
    let document = Document::from_roots([paragraph()
        .child(text("Energy "))
        .child(node(MathInline("\\hbar\n\\omega".into())))
        .child(text(" remains bounded."))])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected = "Energy \\hbar\n\\omega remains bounded.".to_owned();
    let window = cx.open_window(size(px(400.), px(400.)), move |_, _| MathPage(projection));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.cx.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(5.), px(10.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_resize(size(px(130.), px(400.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some(expected)
    );
}
