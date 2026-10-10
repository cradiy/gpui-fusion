use super::*;
use gpui::{
    Context, MouseButton, MouseDownEvent, PlatformInput, Render, Subscription, TestAppContext,
    TouchEvent, TouchId, TouchPhase, VisualTestContext, point, size,
};

struct Example {
    state: Entity<SplitPaneState>,
    axis: Axis,
    width: f32,
    events: Vec<SplitPaneEvent>,
    _subscription: Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SplitPane::new(&self.state, div().child("First"), div().child("Second"))
            .handle_size(px(12.))
            .axis(self.axis)
            .w(px(self.width))
            .h(px(320.))
            .first_min_size(px(160.))
            .first_max_size(px(360.))
            .second_min_size(px(140.))
            .second_max_size(px(500.))
    }
}
fn setup(cx: &mut TestAppContext) -> gpui::WindowHandle<Example> {
    cx.open_window(size(px(1300.), px(500.)), |window, cx| {
        let state = cx.new(|cx| SplitPaneState::new(0.5, window, cx));
        let subscription = cx.subscribe(
            &state,
            |this: &mut Example, _, event: &SplitPaneEvent, _| this.events.push(*event),
        );
        Example {
            state,
            axis: Axis::Horizontal,
            width: 600.,
            events: vec![],
            _subscription: subscription,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn touch(cx: &mut VisualTestContext, phase: TouchPhase, y: f32) {
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Touch(TouchEvent {
                id: TouchId(1),
                phase,
                position: point(px(100.), px(y)),
                force: None,
            }),
            cx,
        );
    });
}

#[gpui::test]
fn captured_drag_commits_once_and_reset_and_keyboard_respect_limits(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |this, _, cx| {
        this.state.update(cx, |state, cx| state.set_ratio(0.1, cx));
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_click(point(px(166.), px(100.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).ratio(), 0.1);
        assert!(this.events.is_empty());
        this.state.update(cx, |state, cx| state.set_ratio(0.5, cx));
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_mouse_down(
        point(px(300.), px(100.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(
        point(px(400.), px(100.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    draw(&mut visual);
    visual.simulate_mouse_move(
        point(px(460.), px(100.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    draw(&mut visual);
    visual.simulate_mouse_up(
        point(px(460.), px(100.)),
        MouseButton::Left,
        Default::default(),
    );
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).first, 360.);
        assert_eq!(
            this.events
                .iter()
                .filter(|event| matches!(event, SplitPaneEvent::Changed(_)))
                .count(),
            1
        );
    })
    .unwrap();
    visual.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(366.), px(100.)),
                modifiers: Default::default(),
                click_count: 2,
                first_mouse: false,
            }),
            cx,
        );
    });
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).ratio(), 0.5)
    })
    .unwrap();
    visual.simulate_keystrokes("right");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!((this.state.read(cx).first - 302.).abs() < 0.01)
    })
    .unwrap();
    visual.simulate_keystrokes("end");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).first, 360.)
    })
    .unwrap();
}

#[gpui::test]
fn resizing_preserves_preference_and_vertical_touch_cancel_does_not_commit(
    cx: &mut TestAppContext,
) {
    let view = setup(cx);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    for width in [200., 1200., 600.] {
        view.update(&mut visual.cx, |this, _, cx| {
            this.width = width;
            cx.notify();
        })
        .unwrap();
        draw(&mut visual);
        view.update(&mut visual.cx, |this, _, cx| {
            let state = this.state.read(cx);
            assert_eq!(state.ratio(), 0.5);
            let expected = if width == 200. {
                188. * 160. / 300.
            } else {
                (width - 12.) / 2.
            };
            assert!((state.first - expected).abs() < 0.01);
            assert!(this.events.is_empty());
        })
        .unwrap();
    }
    view.update(&mut visual.cx, |this, _, cx| {
        this.axis = Axis::Vertical;
        cx.notify();
    })
    .unwrap();
    draw(&mut visual);
    touch(&mut visual, TouchPhase::Started, 166.);
    touch(&mut visual, TouchPhase::Moved, 196.);
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).first, 168.)
    })
    .unwrap();
    touch(&mut visual, TouchPhase::Cancelled, 196.);
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).ratio(), 0.5);
        assert!(
            !this
                .events
                .iter()
                .any(|event| matches!(event, SplitPaneEvent::Changed(_)))
        );
    })
    .unwrap();
    touch(&mut visual, TouchPhase::Started, 166.);
    touch(&mut visual, TouchPhase::Moved, 196.);
    touch(&mut visual, TouchPhase::Ended, 196.);
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).first, 168.);
        assert_eq!(
            this.events
                .iter()
                .filter(|event| matches!(event, SplitPaneEvent::Changed(_)))
                .count(),
            1
        );
    })
    .unwrap();
}
