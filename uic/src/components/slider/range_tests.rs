use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, PlatformInput, Render, Subscription,
    TestAppContext, TouchEvent, TouchId, TouchPhase, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};

use super::{RangeSlider, RangeSliderEvent, RangeSliderState, RangeSliderThumb};

#[gpui::test]
fn unchanged_drag_samples_do_not_notify_observers(cx: &mut TestAppContext) {
    use super::interaction::InteractionPhase;
    use std::{cell::Cell, rc::Rc};

    let state = cx.new(|cx| {
        RangeSliderState::new(80.0..=260.0, 0.0..=400.0, cx)
            .step(10.)
            .min_gap(40.)
    });
    let notifications = Rc::new(Cell::new(0));
    let count = notifications.clone();
    let _observer = cx.update(|cx| cx.observe(&state, move |_, _| count.set(count.get() + 1)));
    state.update(cx, |state, cx| {
        state.pointer(0.2, InteractionPhase::Start, cx);
    });
    notifications.set(0);
    for sample in 0..240 {
        let ratio = 0.2 + (sample % 10) as f32 * 0.0005;
        state.update(cx, |state, cx| {
            state.pointer(ratio, InteractionPhase::Preview, cx);
        });
    }
    assert_eq!(notifications.get(), 0);
    state.update(cx, |state, cx| {
        state.pointer(0.3, InteractionPhase::Preview, cx);
    });
    assert_eq!(notifications.get(), 1);
    state.update(cx, |state, cx| {
        state.pointer(0.3, InteractionPhase::Commit, cx);
    });
    assert_eq!(notifications.get(), 2);
}

#[gpui::test]
fn constraints_survive_snapping_replacements_and_endpoint_adjustments(cx: &mut TestAppContext) {
    cx.update(|cx| {
        for (domain, step, gap) in [
            (-5.0..=12.0, 3., 4.),
            (0.0..=1.0, 0.1, 0.25),
            (0.0..=10.0, 3., 50.),
            (2.0..=2.0, 1., 4.),
        ] {
            let state = cx.new(|cx| {
                RangeSliderState::new(domain.clone(), domain.clone(), cx)
                    .step(step)
                    .min_gap(gap)
            });
            state.update(cx, |state, cx| {
                for values in [100.0..=-100.0, 0.0..=0.0, f64::NAN..=f64::INFINITY] {
                    state.set_values(values, cx);
                    for thumb in [RangeSliderThumb::Lower, RangeSliderThumb::Upper] {
                        for target in [-100., 0., 0.5, 100.] {
                            state.commit(thumb, target, cx);
                            let values = state.values();
                            assert!(*values.start() >= *domain.start());
                            assert!(*values.end() <= *domain.end());
                            assert!(*values.end() - *values.start() + 1e-12 >= state.minimum_gap());
                        }
                    }
                }
            });
        }
        let state = cx.new(|cx| RangeSliderState::new(3.0..=9.0, 0.0..=10.0, cx).step(3.));
        state.update(cx, |state, cx| {
            state.commit(RangeSliderThumb::Upper, 10., cx);
            assert_eq!(state.values(), 3.0..=10.0);
        });
        let decimal = cx.new(|cx| {
            RangeSliderState::new(0.2..=0.3, 0.0..=1.0, cx)
                .step(0.1)
                .min_gap(0.1)
        });
        assert!((*decimal.read(cx).values().end() - 0.3).abs() < 1e-12);
    });
}

struct Example {
    state: Entity<RangeSliderState>,
    events: Vec<RangeSliderEvent>,
    _subscription: Subscription,
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().w(px(360.)).child(RangeSlider::new(&self.state))
    }
}

#[gpui::test]
fn capture_survives_redraw_and_keyboard_targets_the_focused_endpoint(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(450.), px(200.)), |_, cx| {
        let state = cx.new(|cx| {
            RangeSliderState::new(20.0..=80.0, 0.0..=100.0, cx)
                .step(5.)
                .min_gap(15.)
        });
        let subscription = cx.subscribe(
            &state,
            |this: &mut Example, _, event: &RangeSliderEvent, cx| {
                this.events.push(event.clone());
                cx.notify();
            },
        );
        Example {
            state,
            events: Vec::new(),
            _subscription: subscription,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let bounds = visual.debug_bounds("uic-range-slider").unwrap();
    let lower = visual
        .debug_bounds("uic-range-slider-lower")
        .unwrap()
        .center();
    let outside = point(bounds.right() + px(50.), lower.y);
    visual.simulate_mouse_down(lower, MouseButton::Left, Modifiers::default());
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_mouse_move(outside, MouseButton::Left, Modifiers::default());
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |this, window, cx| {
        assert_eq!(this.state.read(cx).values(), 65.0..=80.0);
        assert!(!this.state.read(cx).is_dragging());
        assert_eq!(
            this.events
                .iter()
                .filter(|event| matches!(event, RangeSliderEvent::Changed(_)))
                .count(),
            1
        );
        assert!(
            this.events
                .iter()
                .any(|event| matches!(event, RangeSliderEvent::Changing(_)))
        );
        assert!(
            this.state
                .read(cx)
                .thumb_focus_handle(RangeSliderThumb::Lower)
                .is_focused(window)
        );
    })
    .unwrap();
    visual.simulate_keystrokes("tab end");
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(
            this.state
                .read(cx)
                .thumb_focus_handle(RangeSliderThumb::Upper)
                .is_focused(window)
        );
        assert_eq!(this.state.read(cx).values(), 65.0..=100.0);
        this.state
            .update(cx, |state, cx| state.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_keystrokes("left home");
    visual.simulate_click(bounds.center(), Modifiers::default());
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).values(), 65.0..=100.0);
        this.state.update(cx, |state, cx| {
            state.set_disabled(false, cx);
            state.set_values(20.0..=80.0, cx);
        });
        this.events.clear();
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let origin = visual
        .debug_bounds("uic-range-slider-lower")
        .unwrap()
        .center();
    let mut touch = |phase, position| {
        visual.update(|window, cx| {
            let result = window.dispatch_event(
                PlatformInput::Touch(TouchEvent {
                    id: TouchId(7),
                    phase,
                    position,
                    force: None,
                }),
                cx,
            );
            window.draw(cx).clear();
            result.default_prevented
        })
    };
    assert!(!touch(TouchPhase::Started, origin));
    assert!(!touch(TouchPhase::Moved, origin + point(px(1.), px(30.))));
    assert!(!touch(TouchPhase::Ended, origin + point(px(1.), px(30.))));
    assert!(!touch(TouchPhase::Started, origin));
    assert!(touch(TouchPhase::Moved, outside));
    assert!(touch(TouchPhase::Ended, outside));
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).values(), 65.0..=80.0);
        assert_eq!(
            this.events
                .iter()
                .filter(|event| matches!(event, RangeSliderEvent::Changed(_)))
                .count(),
            1
        );
    })
    .unwrap();
}
