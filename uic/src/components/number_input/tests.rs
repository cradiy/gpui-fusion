use super::*;
use crate::components::input::{self, TextInput};
use gpui::{Context, EntityInputHandler, Render, TestAppContext, VisualTestContext, point, size};

struct Example {
    number: Entity<NumberInputState>,
    other: Entity<TextInput>,
    events: Vec<f64>,
    _subscription: gpui::Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(NumberInput::new(&self.number).w(px(280.)))
            .child(Input::new(&self.other).w(px(280.)))
    }
}
fn setup(cx: &mut TestAppContext) -> (gpui::WindowHandle<Example>, VisualTestContext) {
    cx.update(input::init);
    let view = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let number = cx.new(|cx| {
            NumberInputState::new(
                1.5,
                NumberInputOptions {
                    min: Some(-2.),
                    max: Some(10.),
                    step: 0.25,
                    precision: 2,
                },
                window,
                cx,
            )
        });
        let other = cx.new(TextInput::new);
        let subscription = cx.subscribe(
            &number,
            |this: &mut Example, _, event: &NumberInputChanged, _| this.events.push(event.value),
        );
        Example {
            number,
            other,
            events: vec![],
            _subscription: subscription,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    draw(&mut visual);
    visual.simulate_click(point(px(120.), px(38.)), Default::default());
    draw(&mut visual);
    (view, visual)
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn draft(view: gpui::WindowHandle<Example>, cx: &mut VisualTestContext, text: &str) {
    view.update(&mut cx.cx, |this, _, cx| {
        let input = this.number.read(cx).input.clone();
        input.update(cx, |input, cx| input.set_value(text.to_owned(), cx));
    })
    .unwrap();
    draw(cx);
}
#[gpui::test]
fn drafts_commit_cancel_and_focus_loss_keep_numeric_events_consistent(cx: &mut TestAppContext) {
    let (view, mut visual) = setup(cx);
    for text in ["-", "0."] {
        draft(view, &mut visual, text);
        view.update(&mut visual.cx, |this, _, cx| {
            assert_eq!(this.number.read(cx).draft(cx).as_ref(), text);
            assert_eq!(this.number.read(cx).value(), 1.5);
            assert!(this.events.is_empty());
        })
        .unwrap();
    }
    visual.simulate_keystrokes("enter up up");
    draw(&mut visual);
    draft(view, &mut visual, "200");
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    for invalid in ["NaN", "inf", "nonsense"] {
        draft(view, &mut visual, invalid);
        visual.simulate_keystrokes("enter");
        draw(&mut visual);
    }
    draft(view, &mut visual, "3");
    visual.simulate_keystrokes("escape");
    draw(&mut visual);
    draft(view, &mut visual, "2.345");
    visual.simulate_click(point(px(120.), px(100.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(
            this.other.focus_handle(cx).is_focused(window),
            "other must gain focus"
        );
        assert_eq!(this.events, vec![0., 0.25, 0.5, 10., 2.35]);
        assert_eq!(this.number.read(cx).draft(cx).as_ref(), "2.35");
        this.number.update(cx, |state, cx| {
            assert!(state.set_value(4., cx));
            assert!(!state.set_value(f64::NAN, cx));
        });
    })
    .unwrap();
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.events.len(), 5);
        assert_eq!(this.number.read(cx).value(), 4.);
    })
    .unwrap();
    visual.simulate_click(point(px(120.), px(38.)), Default::default());
    draw(&mut visual);
    draft(view, &mut visual, "160sdd");
    visual.simulate_click(point(px(350.), px(250.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(this.number.read(cx).draft(cx).as_ref(), "4");
        assert_eq!(this.events.len(), 5);
    })
    .unwrap();
}
#[gpui::test]
fn step_controls_preserve_composition_and_respect_bounds_and_disabled_state(
    cx: &mut TestAppContext,
) {
    let (view, mut visual) = setup(cx);
    let input = view
        .update(&mut visual.cx, |this, _, cx| {
            this.number.read(cx).input.clone()
        })
        .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(Some(0..3), "ni", Some(2..2), window, cx)
        })
    });
    visual.simulate_keystrokes("up enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(this.events.is_empty());
        assert!(this.number.read(cx).input.read(cx).is_composing());
    })
    .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, "9.9", window, cx)
        })
    });
    draw(&mut visual);
    visual.simulate_click(point(px(278.), px(29.)), Default::default());
    draw(&mut visual);
    visual.simulate_click(point(px(278.), px(29.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert_eq!(this.events, vec![10.]);
        assert_eq!(this.number.read(cx).value(), 10.);
        assert!(this.number.focus_handle(cx).is_focused(window));
        this.number
            .update(cx, |state, cx| state.set_disabled(true, cx));
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_click(point(px(278.), px(49.)), Default::default());
    visual.simulate_keystrokes("down");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.events, vec![10.]);
        assert_eq!(this.number.read(cx).value(), 10.);
    })
    .unwrap();
}
