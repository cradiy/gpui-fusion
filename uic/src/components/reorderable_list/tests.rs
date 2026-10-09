use super::*;
use crate::components::swipe_actions::{SwipeActions, SwipeActionsState, SwipeTriggered};
use gpui::{
    Context, LongPressEvent, PlatformInput, Render, Subscription, TestAppContext, TouchEvent,
    TouchId, TouchPhase, VisualTestContext, size,
};
use std::time::Duration;

struct Example {
    state: Entity<ReorderState>,
    keys: Vec<usize>,
    swipes: Vec<Entity<SwipeActionsState>>,
    swipe_count: usize,
    moves: Vec<(usize, usize)>,
    _subscriptions: Vec<Subscription>,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = ReorderableList::new("list", &self.state)
            .w(px(300.))
            .h(px(220.))
            .gap(px(10.))
            .on_reorder(cx.listener(|this, event: &ReorderEvent, _, cx| {
                this.moves.push((event.from, event.to));
                let key = this.keys.remove(event.from);
                this.keys.insert(event.to, key);
                cx.notify();
            }));
        for &key in &self.keys {
            list = list.item(
                key,
                SwipeActions::new(
                    ("swipe", key),
                    &self.swipes[key],
                    div()
                        .w_full()
                        .h(px(if key == 1 { 80. } else { 60. }))
                        .bg(gpui::rgb(0xffffff))
                        .child(
                            ReorderHandle::new(&self.state, key, div().size_full())
                                .w(px(30.))
                                .h_full(),
                        ),
                ),
            );
        }
        list
    }
}
fn setup(cx: &mut TestAppContext) -> gpui::WindowHandle<Example> {
    cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let state = cx.new(|cx| ReorderState::new(window, cx));
        let swipes: Vec<_> = (0..10)
            .map(|_| cx.new(|cx| SwipeActionsState::new(window, cx)))
            .collect();
        let subscriptions = swipes
            .iter()
            .map(|state| {
                cx.subscribe(state, |this: &mut Example, _, _: &SwipeTriggered, _| {
                    this.swipe_count += 1
                })
            })
            .collect();
        Example {
            state,
            keys: (0..10).collect(),
            swipes,
            swipe_count: 0,
            moves: vec![],
            _subscriptions: subscriptions,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn touch(cx: &mut VisualTestContext, id: u64, phase: TouchPhase, x: f32, y: f32) {
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Touch(TouchEvent {
                id: TouchId(id),
                phase,
                position: point(px(x), px(y)),
                force: None,
            }),
            cx,
        );
    });
}
fn hold(cx: &mut VisualTestContext, y: f32) {
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::LongPress(LongPressEvent {
                position: point(px(180.), px(y)),
            }),
            cx,
        );
    });
}

#[gpui::test]
fn long_press_sort_and_horizontal_swipe_have_exclusive_ownership(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    touch(&mut cx, 1, TouchPhase::Started, 180., 30.);
    touch(&mut cx, 1, TouchPhase::Moved, 30., 30.);
    touch(&mut cx, 1, TouchPhase::Ended, 30., 30.);
    view.update(&mut cx.cx, |this, _, _| {
        assert_eq!(this.swipe_count, 1);
        assert!(this.moves.is_empty());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    draw(&mut cx);
    touch(&mut cx, 2, TouchPhase::Started, 180., 30.);
    hold(&mut cx, 30.);
    draw(&mut cx);
    touch(&mut cx, 2, TouchPhase::Moved, 30., 195.);
    draw(&mut cx);
    touch(&mut cx, 2, TouchPhase::Ended, 30., 195.);
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, _| {
        assert_eq!(this.moves, [(0, 2)]);
        assert_eq!(&this.keys[..3], &[1, 2, 0]);
        assert_eq!(this.swipe_count, 1);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    draw(&mut cx);
    // Movement before long press is left to scrolling, and cannot become a sort later.
    touch(&mut cx, 3, TouchPhase::Started, 180., 30.);
    touch(&mut cx, 3, TouchPhase::Moved, 180., 55.);
    hold(&mut cx, 55.);
    touch(&mut cx, 3, TouchPhase::Ended, 180., 180.);
    view.update(&mut cx.cx, |this, _, _| assert_eq!(this.moves.len(), 1))
        .unwrap();
}

#[gpui::test]
fn edge_scroll_advances_stationary_drag_and_cancellation_never_commits(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    cx.simulate_mouse_down(
        point(px(15.), px(30.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(px(15.), px(218.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    draw(&mut cx);
    for _ in 0..24 {
        cx.executor().advance_clock(Duration::from_millis(16));
        draw(&mut cx);
    }
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(this.state.read(cx).scroll_handle().offset().y < px(-100.));
        assert_eq!(
            this.state.read(cx).dragged_item(),
            Some(&ElementId::from(0usize))
        );
    })
    .unwrap();
    cx.simulate_mouse_up(
        point(px(15.), px(218.)),
        MouseButton::Left,
        Default::default(),
    );
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.moves.len(), 1);
        assert!(this.moves[0].1 >= 4);
        this.state
            .read(cx)
            .scroll_handle()
            .set_offset(point(px(0.), px(0.)));
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    draw(&mut cx);
    touch(&mut cx, 1, TouchPhase::Started, 180., 30.);
    hold(&mut cx, 30.);
    touch(&mut cx, 1, TouchPhase::Moved, 180., 190.);
    draw(&mut cx);
    touch(&mut cx, 2, TouchPhase::Started, 190., 180.);
    touch(&mut cx, 1, TouchPhase::Ended, 180., 190.);
    touch(&mut cx, 2, TouchPhase::Ended, 190., 180.);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.moves.len(), 1);
        assert!(this.state.read(cx).dragged_item().is_none());
        assert_eq!(this.swipe_count, 0);
    })
    .unwrap();
}
