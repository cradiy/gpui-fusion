use super::*;
use gpui::{
    Context, MouseButton, PlatformInput, Render, TestAppContext, TouchEvent, TouchId, TouchPhase,
    point, size,
};

struct Row {
    state: Entity<SwipeActionsState>,
    clicks: usize,
    dismissed: Option<SwipeDirection>,
    bounds: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    actions: Vec<SwipeDirection>,
    _subscription: gpui::Subscription,
}

impl Row {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| SwipeActionsState::new(window, cx));
        let subscription = cx.subscribe(&state, |this, _, event: &SwipeTriggered, _| {
            this.actions.push(event.direction);
        });
        Self {
            state,
            clicks: 0,
            dismissed: None,
            bounds: Default::default(),
            actions: Vec::new(),
            _subscription: subscription,
        }
    }
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.bounds.clone();
        SwipeActions::new(
            "row",
            &self.state,
            div()
                .id("content")
                .relative()
                .h(px(60.))
                .w_full()
                .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
                .child("Document")
                .child(
                    canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                ),
        )
        .mb(px(12.))
        .dismissed(self.dismissed)
    }
}

struct Rows([Entity<Row>; 2]);
impl Render for Rows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().children(self.0.iter().cloned())
    }
}

#[gpui::test]
fn swipes_trigger_once_on_release_and_short_or_reversed_drags_cancel(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(400.), px(200.)), Row::new);
    let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear());
    // Short, left, retract, right, and full-width swipes beyond either row edge.
    for (end, trigger) in [
        (180., None),
        (40., Some(SwipeDirection::Left)),
        (170., None),
        (360., Some(SwipeDirection::Right)),
        (-240., Some(SwipeDirection::Left)),
        (640., Some(SwipeDirection::Right)),
    ] {
        let before = window
            .update(&mut cx.cx, |view, _, _| view.actions.len())
            .unwrap();
        cx.simulate_mouse_down(
            point(px(200.), px(30.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(
            point(px(if end == 180. { end } else { 40. }), px(30.)),
            Some(MouseButton::Left),
            Default::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear());
        window
            .update(&mut cx.cx, |view, _, _| {
                assert_eq!(view.actions.len(), before)
            })
            .unwrap();
        cx.simulate_mouse_move(
            point(px(end), px(30.)),
            Some(MouseButton::Left),
            Default::default(),
        );
        window
            .update(&mut cx.cx, |view, _, cx| {
                assert_eq!(
                    view.state.read(cx).visual(cx.background_executor().now()).0,
                    px(end - 200.)
                );
            })
            .unwrap();
        cx.simulate_mouse_up(
            point(px(end), px(30.)),
            MouseButton::Left,
            Default::default(),
        );
        window
            .update(&mut cx.cx, |view, _, _| {
                assert_eq!(&view.actions[before..], trigger.as_slice());
                assert_eq!(view.clicks, 0);
            })
            .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.update(|window, cx| window.draw(cx).clear());
        window
            .update(&mut cx.cx, |view, _, cx| {
                assert_eq!(
                    view.state.read(cx).visual(cx.background_executor().now()),
                    (px(0.), false)
                );
            })
            .unwrap();
    }
    cx.simulate_click(point(px(200.), px(30.)), Default::default());
    window
        .update(&mut cx.cx, |view, _, _| assert_eq!(view.clicks, 1))
        .unwrap();
}

#[gpui::test]
fn touch_direction_lock_and_cancellation_leave_scrolling_available(cx: &mut TestAppContext) {
    let (rows, cx) = cx.add_window_view(|window, cx| {
        Rows(std::array::from_fn(|_| cx.new(|cx| Row::new(window, cx))))
    });
    let (view, second) =
        cx.update(|_, cx| (rows.read(cx).0[0].clone(), rows.read(cx).0[1].clone()));
    cx.update(|window, cx| window.draw(cx).clear());
    let touch = |id, phase, x, y, cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            window.dispatch_event(
                PlatformInput::Touch(TouchEvent {
                    id: TouchId(id),
                    phase,
                    position: point(px(x), px(y)),
                    force: None,
                }),
                cx,
            )
        })
    };
    assert!(!touch(1, TouchPhase::Started, 200., 30., cx).default_prevented);
    assert!(!touch(1, TouchPhase::Moved, 202., 60., cx).default_prevented);
    // Once vertical scrolling wins, later horizontal motion cannot steal it.
    assert!(!touch(1, TouchPhase::Moved, 40., 60., cx).default_prevented);
    assert!(!touch(1, TouchPhase::Ended, 40., 60., cx).default_prevented);
    touch(2, TouchPhase::Started, 200., 30., cx);
    assert!(touch(2, TouchPhase::Moved, 40., 30., cx).default_prevented);
    assert!(touch(2, TouchPhase::Cancelled, 40., 30., cx).default_prevented);
    cx.update(|_, cx| assert!(view.read(cx).actions.is_empty()));
    touch(3, TouchPhase::Started, 200., 30., cx);
    touch(4, TouchPhase::Started, 220., 30., cx);
    assert!(!touch(3, TouchPhase::Moved, 40., 30., cx).default_prevented);
    touch(4, TouchPhase::Ended, 220., 30., cx);
    assert!(!touch(3, TouchPhase::Ended, 40., 30., cx).default_prevented);
    touch(5, TouchPhase::Started, 200., 30., cx);
    assert!(touch(5, TouchPhase::Moved, 360., 30., cx).default_prevented);
    assert!(touch(5, TouchPhase::Ended, 360., 30., cx).default_prevented);
    cx.update(|_, cx| assert_eq!(view.read(cx).actions, [SwipeDirection::Right]));
    // The first row consuming a drag must not strand contact tracking in its siblings.
    touch(6, TouchPhase::Started, 200., 90., cx);
    assert!(touch(6, TouchPhase::Moved, 40., 90., cx).default_prevented);
    assert!(touch(6, TouchPhase::Ended, 40., 90., cx).default_prevented);
    cx.update(|_, cx| assert_eq!(second.read(cx).actions, [SwipeDirection::Left]));
}

#[gpui::test]
fn dismissal_slides_out_then_collapses_spacing_and_can_be_restored(cx: &mut TestAppContext) {
    let (rows, cx) = cx.add_window_view(|window, cx| {
        Rows(std::array::from_fn(|_| cx.new(|cx| Row::new(window, cx))))
    });
    let (first, second) =
        cx.update(|_, cx| (rows.read(cx).0[0].clone(), rows.read(cx).0[1].clone()));
    cx.update(|window, cx| window.draw(cx).clear());
    let initial_y = cx.update(|_, cx| second.read(cx).bounds.get().origin.y);
    assert_eq!(initial_y, px(72.));
    cx.simulate_mouse_down(
        point(px(200.), px(30.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(px(60.), px(30.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(px(60.), px(30.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.update(|_, cx| {
        first.update(cx, |row, cx| {
            row.dismissed = Some(SwipeDirection::Left);
            cx.notify();
        })
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
        assert_eq!(
            first
                .read(cx)
                .state
                .read(cx)
                .visual(cx.background_executor().now())
                .0,
            px(-140.)
        );
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.update(|window, cx| {
        window.draw(cx).clear();
        let state = first.read(cx).state.read(cx);
        let offset = state.visual(cx.background_executor().now()).0;
        assert!(offset < px(-140.) && offset > -first.read(cx).bounds.get().size.width);
        assert_eq!(second.read(cx).bounds.get().origin.y, initial_y);
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(120));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.update(|window, cx| {
        window.draw(cx).clear();
        let y = second.read(cx).bounds.get().origin.y;
        assert!(y > px(0.) && y < initial_y);
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(120));
    cx.update(|window, cx| {
        window.draw(cx).clear();
        assert_eq!(second.read(cx).bounds.get().origin.y, px(0.));
        first.update(cx, |row, cx| {
            row.dismissed = None;
            cx.notify();
        });
        window.draw(cx).clear();
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(250));
    cx.update(|window, cx| {
        window.draw(cx).clear();
        assert_eq!(second.read(cx).bounds.get().origin.y, initial_y);
        assert_eq!(
            first
                .read(cx)
                .state
                .read(cx)
                .visual(cx.background_executor().now())
                .0,
            px(0.)
        );
    });
}
