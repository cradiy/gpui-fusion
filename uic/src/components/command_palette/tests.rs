use super::*;
use gpui::{
    Context, EntityInputHandler, FocusHandle, Render, Subscription, TestAppContext,
    VisualTestContext, size,
};

struct Example {
    state: Entity<CommandPaletteState>,
    focus: FocusHandle,
    events: Vec<CommandPaletteEvent>,
    embedded: bool,
    _subscription: Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .when(self.embedded, |root| {
                root.child(
                    CommandPalette::new(&self.state)
                        .key_binding("ctrl-p", CommandPaletteAction::Previous)
                        .key_binding("ctrl-n", CommandPaletteAction::Next)
                        .key_binding("ctrl-j", CommandPaletteAction::Next)
                        .key_binding("ctrl-j", CommandPaletteAction::Previous)
                        .key_binding("ctrl-enter", CommandPaletteAction::Confirm)
                        .key_binding("alt-escape", CommandPaletteAction::Dismiss)
                        .w_full(),
                )
            })
            .child(modal::layer(cx))
    }
}
fn setup(cx: &mut TestAppContext, embedded: bool) -> gpui::WindowHandle<Example> {
    cx.update(crate::init);
    cx.open_window(size(px(640.), px(700.)), |window, cx| {
        let state = cx.new(|cx| {
            CommandPaletteState::new(
                vec![
                    CommandItem::new("new", "New document")
                        .group("Workspace")
                        .keywords("write create"),
                    CommandItem::new("sync", "Sync")
                        .group("Workspace")
                        .disabled(true),
                    CommandItem::new("theme", "Appearance")
                        .group("Settings")
                        .keywords("dark light"),
                ],
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(
            &state,
            |this: &mut Example, _, event: &CommandPaletteEvent, _| this.events.push(event.clone()),
        );
        Example {
            state,
            focus: cx.focus_handle(),
            events: Vec::new(),
            embedded,
            _subscription: subscription,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}

#[gpui::test]
fn search_navigation_and_composition(cx: &mut TestAppContext) {
    let view = setup(cx, true);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |this, window, cx| {
        window.focus(&this.state.focus_handle(cx), cx)
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_keystrokes("down enter");
    view.update(&mut visual.cx, |this, _, _| {
        assert!(matches!(this.events.last(), Some(CommandPaletteEvent::Invoked(item)) if item.id == "theme"));
    }).unwrap();
    visual.simulate_keystrokes("ctrl-p ctrl-enter ctrl-n ctrl-enter ctrl-j ctrl-enter alt-escape");
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.events.len(), 5);
        assert!(matches!(&this.events[1], CommandPaletteEvent::Invoked(item) if item.id == "new"));
        assert!(
            matches!(&this.events[2], CommandPaletteEvent::Invoked(item) if item.id == "theme")
        );
        assert!(matches!(&this.events[3], CommandPaletteEvent::Invoked(item) if item.id == "new"));
        assert!(matches!(this.events[4], CommandPaletteEvent::Dismissed));
        assert!(this.state.read(cx).query().is_empty());
    })
    .unwrap();
    visual.simulate_input("workspace write");
    draw(&mut visual);
    visual.simulate_keystrokes("enter");
    view.update(&mut visual.cx, |this, _, cx| {
        assert_eq!(this.state.read(cx).query().as_ref(), "workspace write");
        assert!(matches!(this.events.last(), Some(CommandPaletteEvent::Invoked(item)) if item.id == "new"));
        this.state.update(cx, |state, cx| state.set_query("", cx));
    }).unwrap();
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, _| this.events.clear())
        .unwrap();
    let input = view
        .update(&mut visual.cx, |this, _, cx| {
            this.state.read(cx).input.clone()
        })
        .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
        })
    });
    draw(&mut visual);
    visual.simulate_keystrokes("ctrl-n ctrl-p ctrl-enter alt-escape down enter escape");
    view.update(&mut visual.cx, |this, _, _| assert!(this.events.is_empty()))
        .unwrap();
    visual.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, "你", window, cx)
        })
    });
    draw(&mut visual);
    visual.simulate_keystrokes("enter");
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(
            !this
                .events
                .iter()
                .any(|e| matches!(e, CommandPaletteEvent::Invoked(_)))
        );
        this.state.update(cx, |state, cx| {
            state.set_query("", cx);
            state.set_loading(true, cx);
        });
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_keystrokes("down enter");
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(
            !this
                .events
                .iter()
                .any(|e| matches!(e, CommandPaletteEvent::Invoked(_)))
        );
        this.state.update(cx, |state, cx| {
            state.set_items(vec![CommandItem::new("remote", "Remote result")], cx);
            state.set_loading(false, cx);
        });
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_keystrokes("enter");
    view.update(&mut visual.cx, |this, _, _| {
        assert!(matches!(this.events.last(), Some(CommandPaletteEvent::Invoked(item)) if item.id == "remote"));
    }).unwrap();
}

#[gpui::test]
fn modal_focus_and_replacement(cx: &mut TestAppContext) {
    let view = setup(cx, false);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |this, window, cx| {
        window.focus(&this.focus, cx);
        CommandPalette::new(&this.state)
            .surface(|content, _, _| div().child(content))
            .show(window, cx);
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_keystrokes("tab shift-tab");
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(this.state.focus_handle(cx).is_focused(window))
    })
    .unwrap();
    visual.simulate_keystrokes("escape");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(!modal::is_open(cx));
        assert!(this.focus.is_focused(window));
        assert!(matches!(
            this.events.last(),
            Some(CommandPaletteEvent::Dismissed)
        ));
        CommandPalette::new(&this.state).show(window, cx);
    })
    .unwrap();
    draw(&mut visual);
    let subscription = view
        .update(&mut visual.cx, |this, window, cx| {
            cx.subscribe_in(&this.state, window, |this, _, event, window, cx| {
                if matches!(event, CommandPaletteEvent::Invoked(_)) {
                    assert!(!modal::is_open(cx));
                    assert!(this.focus.is_focused(window));
                    modal::show(Modal::new(|_, _| "Next action"), window, cx);
                }
            })
        })
        .unwrap();
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |_, _, cx| assert!(modal::is_open(cx)))
        .unwrap();
    drop(subscription);
    view.update(&mut visual.cx, |this, window, cx| {
        modal::dismiss(window, cx);
        window.blur();
        CommandPalette::new(&this.state).show(window, cx);
        modal::show(Modal::new(|_, _| "Replacement"), window, cx);
        this.state.update(cx, |state, cx| state.dismiss(window, cx));
        assert!(modal::is_open(cx));
        modal::dismiss(window, cx);
        window.blur();
        CommandPalette::new(&this.state).show(window, cx);
    })
    .unwrap();
    draw(&mut visual);
    visual.simulate_click(gpui::point(px(10.), px(650.)), Default::default());
    draw(&mut visual);
    view.update(&mut visual.cx, |this, window, cx| {
        assert!(!modal::is_open(cx));
        assert!(window.focused(cx).is_none());
        this.state.update(cx, |state, cx| {
            state.set_items(vec![CommandItem::new("late", "Late result")], cx)
        });
        assert!(!modal::is_open(cx));
    })
    .unwrap();
}

#[gpui::test]
fn dialog_placement_limits_the_material_surface(cx: &mut TestAppContext) {
    use std::cell::Cell;
    let view = setup(cx, false);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    for (placement, expected_top, expected_height) in [
        (None, 24., 420.),
        (Some(ModalPlacement::Top { offset: px(80.) }), 80., 420.),
        (Some(ModalPlacement::Center), 140., 420.),
        (
            Some(ModalPlacement::Bottom {
                avoid_safe_area: true,
                drag_to_dismiss: false,
            }),
            280.,
            420.,
        ),
        (Some(ModalPlacement::Top { offset: px(600.) }), 600., 84.),
    ] {
        let bounds = Rc::new(Cell::new(None));
        let measured = bounds.clone();
        view.update(&mut visual.cx, |this, window, cx| {
            let palette = CommandPalette::new(&this.state).surface(move |content, _, _| {
                let measured = measured.clone();
                div()
                    .child(content)
                    .on_paint_before_children(move |bounds, _, _, _| measured.set(Some(bounds)))
            });
            let palette = if let Some(placement) = placement {
                palette.placement(placement)
            } else {
                palette
            };
            palette.show(window, cx);
        })
        .unwrap();
        draw(&mut visual);
        let bounds = bounds.get().expect("material surface painted");
        assert_eq!(bounds.origin.x, px(40.));
        assert_eq!(bounds.origin.y, px(expected_top));
        assert_eq!(bounds.size.height, px(expected_height));
        visual.simulate_keystrokes("escape");
        draw(&mut visual);
    }
}
