use super::*;
use gpui::{
    AppContext, Context, PlatformInput, Render, ScrollDelta, ScrollHandle, ScrollWheelEvent,
    TestAppContext, WindowHandle, canvas, size,
};
use std::{cell::Cell, rc::Rc};

struct Preview {
    scroll: ScrollHandle,
    once: bool,
    enabled: bool,
    painted: Rc<Cell<Option<Bounds<Pixels>>>>,
    next_top: Rc<Cell<Pixels>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let painted = self.painted.clone();
        let next_top = self.next_top.clone();
        div()
            .id("scroller")
            .w(px(200.))
            .h(px(100.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .child(div().h(px(120.)).flex_shrink_0())
            .child(
                scroll_reveal("card")
                    .w_full()
                    .h(px(100.))
                    .flex_shrink_0()
                    .threshold(0.5)
                    .duration(Duration::from_secs(1))
                    .delay(Duration::from_millis(100))
                    .once(self.once)
                    .enabled(self.enabled)
                    .child(
                        canvas(
                            move |bounds, _, _| painted.set(Some(bounds)),
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    ),
            )
            .child(
                canvas(
                    move |bounds, _, _| next_top.set(bounds.top()),
                    |_, _, _, _| {},
                )
                .h(px(100.))
                .flex_shrink_0(),
            )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| view.painted.set(None))
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn scroll_reveal_uses_clips_preserves_layout_and_rearms_only_after_exit(cx: &mut TestAppContext) {
    let scroll = ScrollHandle::default();
    let painted = Rc::new(Cell::new(None));
    let next_top = Rc::new(Cell::new(px(0.)));
    let handle = cx.open_window(size(px(400.), px(400.)), |_, _| Preview {
        scroll: scroll.clone(),
        once: true,
        enabled: true,
        painted: painted.clone(),
        next_top: next_top.clone(),
    });
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(handle, cx);
    assert!(
        painted.get().is_none(),
        "window visibility alone must not trigger content below the scroll clip"
    );
    assert_eq!(
        next_top.get(),
        px(220.),
        "hidden content retains its layout slot"
    );

    scroll.set_offset(point(px(0.), px(-60.)));
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(handle, cx);
    assert!(
        painted.get().is_none(),
        "40% visibility is below the threshold"
    );

    scroll.set_offset(point(px(0.), px(-80.)));
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    draw(handle, cx);
    assert!(
        painted.get().is_none(),
        "delay starts at entrance, not at mount"
    );
    cx.executor().advance_clock(Duration::from_millis(550));
    draw(handle, cx);
    assert!((painted.get().unwrap().top() - px(49.)).abs() < px(0.01));
    assert_eq!(
        next_top.get(),
        px(140.),
        "visual displacement must not move siblings"
    );

    cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position: point(px(50.), px(75.)),
                delta: ScrollDelta::Pixels(point(px(0.), px(20.))),
                ..Default::default()
            }),
            cx,
        );
    })
    .unwrap();
    assert_eq!(
        scroll.offset().y,
        px(-60.),
        "entrance must not block scrolling under the pointer"
    );
    draw(handle, cx);
    assert!(
        painted.get().is_some(),
        "dropping below the trigger threshold must not restart playback"
    );
    scroll.set_offset(point(px(0.), px(-220.)));
    draw(handle, cx);
    assert!(painted.get().is_none());
    cx.executor().advance_clock(Duration::from_secs(2));
    scroll.set_offset(point(px(0.), px(-80.)));
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap().top(),
        px(40.),
        "once mode keeps the completed entrance"
    );

    handle.update(cx, |view, _, _| view.once = false).unwrap();
    scroll.set_offset(point(px(0.), px(-220.)));
    draw(handle, cx);
    scroll.set_offset(point(px(0.), px(-80.)));
    draw(handle, cx);
    assert!(
        painted.get().is_none(),
        "repeat mode rearms after a full exit"
    );
    cx.executor().advance_clock(Duration::from_millis(600));
    draw(handle, cx);
    assert!((painted.get().unwrap().top() - px(49.)).abs() < px(0.01));

    handle
        .update(cx, |view, _, _| view.enabled = false)
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap().top(),
        px(40.),
        "reduced motion settles immediately"
    );
    handle.update(cx, |view, _, _| view.enabled = true).unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap().top(),
        px(40.),
        "reenabling motion does not replay visible content"
    );
}
