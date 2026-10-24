use super::*;
use crate::{
    Context, InteractiveText, IntoElement, Render, ScrollHandle, StyledText, TestAppContext,
    VisualTestContext, div, prelude::*,
};

struct Document {
    text: SharedString,
    handle: TextSelectionHandle,
    scroll: ScrollHandle,
    shown: bool,
    selectable: bool,
}

impl Render for Document {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p(px(12.)).child(
            div()
                .id("viewport")
                .w(px(170.))
                .h(px(54.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .text_size(px(16.))
                .line_height(px(24.))
                .child(div().w_full().when(self.shown, |content| {
                    content.child(
                        InteractiveText::new("document", StyledText::new(self.text.clone()))
                            .selection_handle(&self.handle)
                            .when(self.selectable, |text| text.selectable(crate::blue())),
                    )
                })),
        )
    }
}

#[crate::test]
fn selection_handle_tracks_scrolling_replacement_and_removal(cx: &mut TestAppContext) {
    let handle = TextSelectionHandle::default();
    let scroll = ScrollHandle::new();
    let text: SharedString =
        "Cedar branches sway beyond the old stone bridge.\n雨后的街道\nThe ferry returns at dusk."
            .into();
    let view = cx.open_window(size(px(260.), px(160.)), {
        let (handle, scroll, text) = (handle.clone(), scroll.clone(), text.clone());
        move |_, _| Document {
            text,
            handle,
            scroll,
            shown: true,
            selectable: true,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    let first = visual.update(|window, cx| {
        window.draw(cx).clear();
        let fragments = handle.range_geometry(0..text.len(), window).unwrap();
        assert!(fragments.len() > 3);
        fragments[0].visible_bounds.unwrap().center()
    });
    visual.simulate_click(first, Default::default());
    visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
    visual.update(|window, cx| {
        window.draw(cx).clear();
        let selected = handle.selection(window).unwrap();
        assert_eq!(selected.text, text);
        assert_eq!(selected.range(), 0..text.len());
        let focus = window.focused(cx);
        let before = handle.range_geometry(selected.range(), window).unwrap();
        scroll.set_offset(point(px(0.), px(-36.)));
        window.refresh();
        window.draw(cx).clear();
        let after = handle.range_geometry(selected.range(), window).unwrap();
        assert_eq!(before.len(), after.len());
        assert!(after[0].visible_bounds.is_none());
        let visible = after
            .iter()
            .filter_map(|fragment| fragment.visible_bounds)
            .collect::<Vec<_>>();
        assert!(!visible.is_empty());
        assert!(
            visible
                .iter()
                .all(|bounds| bounds.top() >= px(12.) && bounds.bottom() <= px(66.))
        );
        for (before, after) in before.iter().zip(&after) {
            assert_eq!(after.bounds.top(), before.bounds.top() - px(36.));
        }
        assert_eq!(window.focused(cx), focus);
        assert_eq!(handle.selection(window).unwrap(), selected);
        let unicode = text.find('雨').unwrap();
        assert!(
            handle
                .range_geometry(unicode + 1..unicode + 3, window)
                .is_none()
        );
        assert!(handle.range_geometry(0..text.len() + 1, window).is_none());
        assert!(handle.range_geometry(0..0, window).unwrap().is_empty());
    });
    view.update(&mut visual.cx, |view, _, cx| {
        view.text = "Short passage".into();
        view.scroll.set_offset(Point::default());
        cx.notify();
    })
    .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
        let selection = handle.selection(window).unwrap();
        assert!(selection.range().is_empty());
        assert_eq!(selection.text.as_ref(), "Short passage");
        assert!(handle.range_geometry(0..text.len(), window).is_none());
    });
    view.update(&mut visual.cx, |view, _, cx| {
        view.selectable = false;
        cx.notify();
    })
    .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
        assert!(handle.selection(window).is_none());
        assert!(!handle.range_geometry(0..5, window).unwrap().is_empty());
    });
    view.update(&mut visual.cx, |view, _, cx| {
        view.shown = false;
        cx.notify();
    })
    .unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear();
        assert!(handle.selection(window).is_none());
        assert!(handle.range_geometry(0..5, window).is_none());
    });
}
