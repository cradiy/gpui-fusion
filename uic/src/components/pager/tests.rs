use super::*;
use gpui::{
    MouseButton, PlatformInput, Render, TestAppContext, TouchEvent, TouchId, TouchPhase, point, px,
    size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

struct Example {
    state: Entity<PagerState>,
    clicks: Rc<Cell<usize>>,
    built: Rc<RefCell<Vec<usize>>>,
    changed: Vec<Option<usize>>,
    width: f32,
    scroll: gpui::ScrollHandle,
    child_claims: Rc<Cell<bool>>,
    _subscription: gpui::Subscription,
}
impl Example {
    fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        let state = cx.new(|cx| PagerState::new(20, window, cx));
        let subscription = cx.subscribe(&state, |this, _, event: &PageChanged, _| {
            this.changed.push(event.page)
        });
        Self {
            state,
            clicks: Default::default(),
            built: Default::default(),
            changed: vec![],
            width: 400.,
            scroll: gpui::ScrollHandle::new(),
            child_claims: Default::default(),
            _subscription: subscription,
        }
    }
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        let scroll = self.scroll.clone();
        let child_claims = self.child_claims.clone();
        let built = self.built.clone();
        built.borrow_mut().clear();
        Pager::new("pager", &self.state, move |index, _, _| {
            built.borrow_mut().push(index);
            let clicks = clicks.clone();
            let claim = child_claims.clone();
            div()
                .id("content")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                .child(
                    div().h(px(800.)).w_full().child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |_, bounds, window, _| {
                                window.on_mouse_event(
                                    move |event: &TouchEvent, phase, window, _| {
                                        if phase.bubble()
                                            && claim.get()
                                            && bounds.contains(&event.position)
                                        {
                                            window.prevent_default();
                                        }
                                    },
                                )
                            },
                        )
                        .absolute()
                        .inset_0(),
                    ),
                )
        })
        .w(px(self.width))
        .h(px(200.))
    }
}

#[gpui::test]
fn dragging_snaps_without_clicking_and_programmatic_navigation_is_interruptible(
    cx: &mut TestAppContext,
) {
    let view = cx.open_window(size(px(600.), px(300.)), Example::new);
    let mut cx = gpui::VisualTestContext::from_window(view.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_click(point(px(200.), px(100.)), Default::default());
    cx.simulate_mouse_down(
        point(px(300.), px(100.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(px(140.), px(100.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, cx| {
        assert_eq!(view.state.read(cx).current_page(), Some(0));
        assert_eq!(*view.built.borrow(), [0, 1]);
        assert_eq!(view.clicks.get(), 1);
    })
    .unwrap();
    cx.simulate_mouse_up(
        point(px(140.), px(100.)),
        MouseButton::Left,
        Default::default(),
    );
    view.update(&mut cx.cx, |view, _, cx| {
        assert_eq!(view.state.read(cx).current_page(), Some(1));
        assert_eq!(view.changed, [Some(1)]);
        assert_eq!(view.clicks.get(), 1);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, cx| {
        assert_eq!(*view.built.borrow(), [1]);
        view.state.update(cx, |state, cx| {
            state.scroll_to(12, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    view.update(&mut cx.cx, |view, _, cx| {
        view.state.update(cx, |state, cx| {
            let before = state.visual(cx.background_executor().now()).0;
            assert!(before > 1. && before < 12.);
            state.scroll_to(2, cx);
            assert_eq!(state.visual(cx.background_executor().now()).0, before);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, cx| {
        assert_eq!(*view.built.borrow(), [2]);
        view.state
            .update(cx, |state, cx| state.set_page_count(0, cx));
    })
    .unwrap();
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, cx| {
        assert!(view.built.borrow().is_empty());
        assert_eq!(view.state.read(cx).current_page(), None);
        assert_eq!(view.changed.last(), Some(&None));
    })
    .unwrap();
}

#[gpui::test]
fn touch_direction_multitouch_cancel_resize_and_fling(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(600.), px(300.)), Example::new);
    let mut cx = gpui::VisualTestContext::from_window(view.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear());
    let touch = |id, phase, x, y, cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            window
                .dispatch_event(
                    PlatformInput::Touch(TouchEvent {
                        id: TouchId(id),
                        phase,
                        position: point(px(x), px(y)),
                        force: None,
                    }),
                    cx,
                )
                .default_prevented
        })
    };
    touch(1, TouchPhase::Started, 300., 60., &mut cx);
    assert!(!touch(1, TouchPhase::Moved, 300., 110., &mut cx));
    assert!(!touch(1, TouchPhase::Moved, 120., 110., &mut cx));
    touch(1, TouchPhase::Ended, 120., 110., &mut cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
                position: point(px(200.), px(100.)),
                delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-80.))),
                ..Default::default()
            }),
            cx,
        );
    });
    view.update(&mut cx.cx, |view, _, _| {
        assert!(view.scroll.offset().y < px(0.));
        view.child_claims.set(true);
    })
    .unwrap();
    touch(9, TouchPhase::Started, 300., 60., &mut cx);
    touch(9, TouchPhase::Moved, 120., 60., &mut cx);
    touch(9, TouchPhase::Ended, 120., 60., &mut cx);
    view.update(&mut cx.cx, |view, _, _| {
        assert!(view.changed.is_empty());
        view.child_claims.set(false);
    })
    .unwrap();
    touch(2, TouchPhase::Started, 300., 60., &mut cx);
    assert!(touch(2, TouchPhase::Moved, 120., 60., &mut cx));
    touch(3, TouchPhase::Started, 200., 60., &mut cx);
    touch(3, TouchPhase::Ended, 200., 60., &mut cx);
    assert!(!touch(2, TouchPhase::Ended, 120., 60., &mut cx));
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, cx| {
        assert_eq!(view.state.read(cx).current_page(), Some(0))
    })
    .unwrap();
    touch(4, TouchPhase::Started, 300., 60., &mut cx);
    assert!(touch(4, TouchPhase::Moved, 120., 60., &mut cx));
    view.update(&mut cx.cx, |view, _, cx| {
        view.width = 500.;
        cx.notify();
    })
    .unwrap();
    cx.update(|window, cx| window.draw(cx).clear());
    touch(4, TouchPhase::Ended, 120., 60., &mut cx);
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, cx| window.draw(cx).clear());
    view.update(&mut cx.cx, |view, _, _| assert!(view.changed.is_empty()))
        .unwrap();
    touch(5, TouchPhase::Started, 300., 60., &mut cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(touch(5, TouchPhase::Moved, 260., 60., &mut cx));
    touch(5, TouchPhase::Ended, 260., 60., &mut cx);
    view.update(&mut cx.cx, |view, _, _| assert_eq!(view.changed, [Some(1)]))
        .unwrap();
}
