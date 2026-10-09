use super::*;
use crate::components::input;
use gpui::{Context, EntityInputHandler, Render, TestAppContext, VisualTestContext, size};

struct Example {
    tags: Entity<TagsInputState>,
    events: Vec<Vec<SharedString>>,
    _subscription: gpui::Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(TagsInput::new(&self.tags).w(px(320.)))
    }
}
fn setup(cx: &mut TestAppContext) -> (gpui::WindowHandle<Example>, VisualTestContext) {
    cx.update(input::init);
    let view = cx.open_window(size(px(440.), px(300.)), |window, cx| {
        let tags = cx.new(|cx| {
            TagsInputState::new(
                vec!["Alpha".into(), "Beta".into()],
                TagsInputOptions {
                    max_tags: Some(3),
                    ..Default::default()
                },
                window,
                cx,
            )
            .validator(|tag| {
                if tag.contains('!') {
                    Err("No exclamation marks".into())
                } else {
                    Ok(())
                }
            })
        });
        let subscription = cx.subscribe(
            &tags,
            |this: &mut Example, _, event: &TagsInputChanged, _| {
                this.events.push(event.tags.clone())
            },
        );
        Example {
            tags,
            events: Vec::new(),
            _subscription: subscription,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        this.tags.focus_handle(cx).focus(window, cx)
    })
    .unwrap();
    draw(&mut visual);
    (view, visual)
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn draft(view: gpui::WindowHandle<Example>, cx: &mut VisualTestContext, text: &str) {
    view.update(&mut cx.cx, |this, _, cx| {
        let input = this.tags.read(cx).input.clone();
        input.update(cx, |input, cx| input.set_value(text.to_owned(), cx));
    })
    .unwrap();
    draw(cx);
}

#[gpui::test]
fn validation_keeps_rejected_drafts_and_replacements_are_atomic(cx: &mut TestAppContext) {
    let (view, mut visual) = setup(cx);
    for (text, error) in [
        (
            "bad!",
            TagsInputError::Invalid("No exclamation marks".into()),
        ),
        (" alpha ", TagsInputError::Duplicate),
    ] {
        draft(view, &mut visual, text);
        visual.simulate_keystrokes("enter");
        draw(&mut visual);
        view.update(&mut visual.cx, |this, _, cx| {
            assert_eq!(this.tags.read(cx).draft(cx).as_ref(), text);
            assert_eq!(this.tags.read(cx).error(), Some(&error));
            assert!(this.events.is_empty());
        })
        .unwrap();
    }
    draft(view, &mut visual, " Gamma ");
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    draft(view, &mut visual, "Delta");
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(
            this.tags.read(cx).error(),
            Some(&TagsInputError::LimitReached)
        );
        assert_eq!(this.tags.read(cx).draft(cx).as_ref(), "Delta");
        assert_eq!(
            this.events,
            vec![vec![
                SharedString::from("Alpha"),
                "Beta".into(),
                "Gamma".into()
            ]]
        );
        this.tags.update(cx, |state, cx| {
            assert_eq!(
                state.set_tags(vec!["Alpha".into(), " alpha ".into()], cx),
                Err(TagsInputError::Duplicate)
            );
            assert_eq!(state.tags().len(), 3);
            assert_eq!(state.draft(cx).as_ref(), "Delta");
            state.set_tags(vec!["Seed".into()], cx).unwrap();
        });
    })
    .unwrap();
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.tags.read(cx).tags(), &[SharedString::from("Seed")]);
        assert!(this.tags.read(cx).draft(cx).is_empty());
        assert_eq!(this.events.len(), 1);
    })
    .unwrap();
}

#[gpui::test]
fn composition_keyboard_removal_and_outside_focus_preserve_tags(cx: &mut TestAppContext) {
    let (view, mut visual) = setup(cx);
    let input = view
        .update(&mut visual.cx, |this, _, cx| {
            this.tags.read(cx).input.clone()
        })
        .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "中文", Some(2..2), window, cx)
        })
    });
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.tags.read(cx).tags().len(), 2);
        assert!(this.events.is_empty());
    })
    .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, "中文", window, cx)
        })
    });
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    visual.simulate_keystrokes("backspace");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.tags.read(cx).selected.as_deref(), Some("中文"));
        assert_eq!(this.tags.read(cx).tags().len(), 3);
    })
    .unwrap();
    visual.simulate_keystrokes("backspace left");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.tags.read(cx).selected.as_deref(), Some("Alpha"));
        assert_eq!(this.events.len(), 2);
    })
    .unwrap();
    visual.simulate_keystrokes("right escape");
    draw(&mut visual);
    draft(view, &mut visual, "pending");
    visual.simulate_click(point(px(400.), px(250.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(this.tags.read(cx).draft(cx).as_ref(), "pending");
        assert_eq!(this.events.len(), 2);
        this.tags
            .update(cx, |state, cx| state.set_disabled(true, cx));
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_click(point(px(260.), px(40.)), Default::default());
    visual.simulate_keystrokes("enter backspace");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.tags.read(cx).tags().len(), 2);
        assert_eq!(this.events.len(), 2);
    })
    .unwrap();
}
