use super::*;
use crate::components::input;
use gpui::{
    Context, EntityInputHandler, Render, Subscription, TestAppContext, VisualTestContext, size,
};

struct Example {
    state: Entity<AutocompleteState>,
    events: Vec<AutocompleteEvent>,
    _subscription: Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            Autocomplete::new(&self.state)
                .key_binding("ctrl-n", AutocompleteAction::Next)
                .key_binding("ctrl-p", AutocompleteAction::Previous)
                .key_binding("tab", AutocompleteAction::Confirm)
                .w(px(240.)),
        )
    }
}
fn setup(cx: &mut TestAppContext, auto_highlight: bool) -> gpui::WindowHandle<Example> {
    cx.update(input::init);
    cx.open_window(size(px(500.), px(640.)), |window, cx| {
        let state = cx.new(|cx| {
            AutocompleteState::new(
                vec![
                    SelectOption::new("a", "Apple"),
                    SelectOption::new("b", "Banana").disabled(true),
                    SelectOption::new("c", "Cherry"),
                ],
                window,
                cx,
            )
            .auto_highlight(auto_highlight)
        });
        let subscription = cx.subscribe(
            &state,
            |this: &mut Example, _, event: &AutocompleteEvent, _| this.events.push(event.clone()),
        );
        Example {
            state,
            events: Vec::new(),
            _subscription: subscription,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}

#[gpui::test]
fn navigation_selection_free_text_and_outside_dismissal(cx: &mut TestAppContext) {
    let view = setup(cx, false);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    visual.simulate_click(point(px(70.), px(36.)), Default::default());
    draw(&mut visual);
    draw(&mut visual);
    visual.simulate_keystrokes("escape");
    draw(&mut visual);
    visual.simulate_click(point(px(70.), px(36.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(this.state.read(cx).is_open());
    })
    .unwrap();
    visual.simulate_keystrokes("down down enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        let state = this.state.read(cx);
        assert_eq!(state.value().as_ref(), "Cherry");
        assert!(!state.is_open());
        assert!(state.focus_handle(cx).is_focused(window));
        assert_eq!(this.events.len(), 2);
        assert!(matches!(&this.events[0], AutocompleteEvent::Change(value) if value == "Cherry"));
        assert!(matches!(&this.events[1], AutocompleteEvent::Selected(option) if option.id == "c"));
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("custom");
    draw(&mut visual);
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "custom");
        assert!(matches!(this.events.last(), Some(AutocompleteEvent::Submit(value)) if value == "custom"));
        this.state.update(cx, |state, cx| state.set_value("", cx));
    }).unwrap();
    visual.simulate_keystrokes("down");
    draw(&mut visual);
    visual.simulate_click(point(px(80.), px(84.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "Apple");
        assert!(this.state.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    visual.simulate_keystrokes("down");
    draw(&mut visual);
    visual.simulate_click(point(px(460.), px(560.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(!this.state.read(cx).is_open());
        assert!(!this.state.focus_handle(cx).is_focused(window));
        this.state.update(cx, |state, cx| {
            state.set_options(vec![SelectOption::new("late", "Late response")], cx)
        });
        assert!(!this.state.read(cx).is_open());
    })
    .unwrap();
}

#[gpui::test]
fn composition_and_loading_cannot_confirm_suggestions(cx: &mut TestAppContext) {
    let view = setup(cx, false);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    visual.simulate_click(point(px(70.), px(36.)), Default::default());
    draw(&mut visual);
    visual.simulate_keystrokes("down");
    let input = view
        .update(&mut visual.cx, |this, _, cx| {
            this.state.read(cx).input.clone()
        })
        .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx)
        })
    });
    draw(&mut visual);
    visual.simulate_keystrokes("ctrl-n ctrl-p down enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(this.events.is_empty());
        assert_eq!(this.state.read(cx).value().as_ref(), "");
    })
    .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, "你", window, cx)
        })
    });
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "你");
        assert_eq!(this.events.len(), 1);
        this.state.update(cx, |state, cx| {
            state.set_value("", cx);
            state.set_loading(true, cx);
        });
    })
    .unwrap();
    visual.simulate_keystrokes("down enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(
            !this
                .events
                .iter()
                .any(|event| matches!(event, AutocompleteEvent::Selected(_)))
        );
        this.state.update(cx, |state, cx| {
            state.set_loading(false, cx);
            state.set_options(vec![SelectOption::new("new", "New result")], cx);
        });
    })
    .unwrap();
    visual.simulate_keystrokes("down enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "New result")
    })
    .unwrap();
}

#[gpui::test]
fn custom_shortcuts_navigate_and_confirm_without_stealing_editing(cx: &mut TestAppContext) {
    let view = setup(cx, false);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    visual.simulate_click(point(px(70.), px(36.)), Default::default());
    draw(&mut visual);
    visual.simulate_keystrokes("ctrl-n ctrl-n ctrl-p tab");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "Apple");
        assert!(this.state.focus_handle(cx).is_focused(window));
        assert!(matches!(this.events.last(), Some(AutocompleteEvent::Selected(option)) if option.id == "a"));
    }).unwrap();
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("Custom");
    draw(&mut visual);
    visual.simulate_keystrokes("tab");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(matches!(this.events.last(), Some(AutocompleteEvent::Submit(value)) if value == "Custom"));
        assert!(this.state.focus_handle(cx).is_focused(window));
    }).unwrap();
}

#[gpui::test]
fn editing_highlights_first_enabled_result_without_changing_text(cx: &mut TestAppContext) {
    let view = setup(cx, true);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    visual.simulate_click(point(px(70.), px(36.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(this.state.read(cx).active.is_none());
        this.state.update(cx, |state, cx| {
            state.set_options(
                vec![
                    SelectOption::new("disabled", "Choice unavailable").disabled(true),
                    SelectOption::new("first", "Choice one"),
                    SelectOption::new("second", "Choice two"),
                ],
                cx,
            );
        });
    })
    .unwrap();
    visual.simulate_input("Choice");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        let state = this.state.read(cx);
        assert_eq!(state.active.as_deref(), Some("first"));
        assert_eq!(state.value().as_ref(), "Choice");
        assert!(this.events.iter().all(|event| matches!(event, AutocompleteEvent::Change(_))));
        assert!(matches!(this.events.last(), Some(AutocompleteEvent::Change(value)) if value == "Choice"));
    })
    .unwrap();
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).value().as_ref(), "Choice one");
        assert!(matches!(this.events.last(), Some(AutocompleteEvent::Selected(option)) if option.id == "first"));
        this.state.update(cx, |state, cx| state.set_loading(true, cx));
    }).unwrap();
    visual.simulate_input("!");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        this.state.update(cx, |state, cx| {
            state.set_options(vec![SelectOption::new("remote", "Choice one! remote")], cx);
            assert!(state.active.is_none());
            state.set_loading(false, cx);
            assert_eq!(state.active.as_deref(), Some("remote"));
        });
    })
    .unwrap();
    visual.simulate_keystrokes("escape");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        this.state.update(cx, |state, cx| {
            state.set_options(vec![SelectOption::new("late", "Choice one! late")], cx);
            assert!(!state.is_open());
            assert!(state.active.is_none());
        });
    })
    .unwrap();
}
