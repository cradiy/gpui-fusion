use super::{Input, InputPart, color, length};
use crate::{
    components::input::InputState,
    style::{MaterialMode, Styled as FusionStyled},
    theme::{ResolveContext, runtime::ThemeRuntime},
};
use crate::{default_theme, style::Token, theme::ThemeStore};
use gpui::{
    Context, Entity, EntityInputHandler, Focusable, Hsla, IntoElement, Render, TestAppContext,
    VisualTestContext, Window, div, prelude::*, px, size,
};

struct Harness {
    field: Entity<Input>,
    editor: Entity<InputState>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.field.clone()).child(
            div()
                .id("outside")
                .debug_selector(|| "outside".into())
                .h(px(30.))
                .child("Outside"),
        )
    }
}

#[gpui::test]
fn theme_material_and_parts_preserve_active_composition(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(500.), px(240.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(|cx| InputState::new(cx).initial_value("Parcel"));
        let clear = editor.downgrade();
        let field = cx.new(|cx| {
            Input::new(&editor, window, cx)
                .suffix(move |_, _| {
                    let editor = clear.clone();
                    div()
                        .id("clear")
                        .debug_selector(|| "clear".into())
                        .flex_none()
                        .size(px(24.))
                        .child("×")
                        .on_click(move |_, _, cx| {
                            let _ = editor.update(cx, |editor, cx| editor.clear(cx));
                        })
                        .into_any_element()
                })
                .w(px(360.))
                .part(InputPart::Caret, |style| style.w(px(2.)))
                .part(InputPart::Selection, |style| {
                    style.bg(Token::<Hsla>::new("accent"))
                })
        });
        Harness {
            field,
            editor,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |view, window, cx| {
        window.activate_window();
        window.focus(&view.editor.focus_handle(cx), cx);
        view.editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(1..3, window, cx);
            editor.replace_and_mark_text_in_range(None, "kk", Some(1..1), window, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let before = visual.debug_bounds("fusion-input").unwrap();
    view.update(&mut visual.cx, |view, _, cx| {
        view.theme.update(cx, |theme, cx| {
            theme.set_mode("dark", cx).unwrap();
            theme.set_material_mode(MaterialMode::Glass, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(visual.debug_bounds("fusion-input").unwrap(), before);
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.editor.focus_handle(cx).is_focused(window));
        assert!(view.field.read(cx).style_error().is_none());
        assert!(
            view.field.read(cx).resolved.as_ref().unwrap().3[0]
                .material
                .is_some()
        );
        let theme = view.theme.read(cx);
        let context =
            ResolveContext::new(theme.store().theme(), "dark", theme.style_set()).unwrap();
        let accent = context
            .resolve(&Token::<Hsla>::new("accent").into())
            .unwrap();
        assert_eq!(
            color(&view.editor.read(cx).selection_style.background),
            accent
        );
        assert_eq!(
            length(&view.editor.read(cx).caret_style.width, px(0.)),
            px(2.)
        );
        view.editor.update(cx, |editor, cx| {
            assert_eq!(editor.value().as_ref(), "Pkkcel");
            assert_eq!(editor.marked_text_range(window, cx), Some(1..3));
            assert_eq!(
                editor.selected_text_range(false, window, cx).unwrap().range,
                2..2
            );
        });
        view.field.update(cx, |field, cx| {
            field.set_material_mode(Some(MaterialMode::Plain), cx)
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, _, cx| {
        assert!(
            view.field.read(cx).resolved.as_ref().unwrap().3[0]
                .material
                .is_none()
        );
        assert!(view.editor.read(cx).is_composing());
    })
    .unwrap();
    let clear = visual.debug_bounds("clear").unwrap();
    assert!(before.contains(&clear.center()));
    visual.simulate_click(clear.center(), Default::default());
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.editor.focus_handle(cx).is_focused(window));
        assert_eq!(view.editor.read(cx).value().as_ref(), "");
        assert!(!view.editor.read(cx).is_composing());
    })
    .unwrap();
    visual.simulate_input("F");
    visual.simulate_keystrokes("ctrl-z");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(view.editor.read(cx).value().as_ref(), "")
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-shift-z");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(view.editor.read(cx).value().as_ref(), "F")
    })
    .unwrap();
    let outside = visual.debug_bounds("outside").unwrap().center();
    visual.simulate_click(outside, Default::default());
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(!view.editor.focus_handle(cx).is_focused(window))
    })
    .unwrap();
}

struct ScrollHarness {
    field: Entity<Input>,
    editor: Entity<InputState>,
    scroll: gpui::ScrollHandle,
}
impl Render for ScrollHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(div().h(px(24.)))
            .child(self.field.clone())
            .child(div().h(px(900.)))
    }
}

#[gpui::test]
fn focused_multiline_contains_scroll_including_edges_and_padding(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point};
    let view = cx.open_window(size(px(500.), px(300.)), |window, cx| {
        ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(|cx| {
            InputState::new(cx)
                .multiline()
                .initial_value((0..40).map(|i| format!("Entry {i}\n")).collect::<String>())
        });
        let field = cx.new(|cx| Input::new(&editor, window, cx).rows(3));
        ScrollHarness {
            field,
            editor,
            scroll: gpui::ScrollHandle::new(),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |view, window, cx| {
        window.activate_window();
        window.focus(&view.editor.focus_handle(cx), cx);
        view.editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(0..0, window, cx)
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let field = visual.debug_bounds("fusion-input").unwrap();
    let inside = field.center();
    let padding = point(field.left() + px(3.), field.top() + px(3.));
    for position in [inside, padding] {
        for (delta, phase) in [
            (-28., TouchPhase::Moved),
            (-4000., TouchPhase::Moved),
            (-20., TouchPhase::Ended),
            (4000., TouchPhase::Moved),
            (20., TouchPhase::Ended),
        ] {
            visual.simulate_event(ScrollWheelEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(1.), px(delta))),
                modifiers: Default::default(),
                touch_phase: phase,
            });
            visual.update(|window, cx| window.draw(cx).clear());
            view.update(&mut visual.cx, |view, _, cx| {
                assert_eq!(view.scroll.offset().y, px(0.));
                let scroll = &view.editor.read(cx).scroll_handle;
                if delta < 0. {
                    assert!(scroll.offset().y < px(0.));
                }
                assert!(scroll.offset().y >= -scroll.max_offset().y);
            })
            .unwrap();
        }
    }
    // A field too short to scroll still owns scrolling while focused.
    view.update(&mut visual.cx, |view, _, cx| {
        view.editor
            .update(cx, |editor, cx| editor.set_value("Brief", cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(ScrollWheelEvent {
        position: inside,
        delta: ScrollDelta::Lines(point(0., -2.)),
        modifiers: Default::default(),
        touch_phase: TouchPhase::Moved,
    });
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, _, _| {
        assert_eq!(view.scroll.offset().y, px(0.))
    })
    .unwrap();
    // Pointer location and focus are both required for containment.
    visual.simulate_event(ScrollWheelEvent {
        position: point(px(250.), px(270.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-30.))),
        modifiers: Default::default(),
        touch_phase: TouchPhase::Moved,
    });
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.scroll.offset().y < px(0.));
        view.scroll.set_offset(point(px(0.), px(0.)));
        window.blur();
        cx.notify();
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(ScrollWheelEvent {
        position: inside,
        delta: ScrollDelta::Pixels(point(px(0.), px(-30.))),
        modifiers: Default::default(),
        touch_phase: TouchPhase::Moved,
    });
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, _, _| {
        assert!(view.scroll.offset().y < px(0.))
    })
    .unwrap();
}

#[gpui::test]
fn password_visibility_toggle_keeps_selection_composition_and_keyboard_access(
    cx: &mut TestAppContext,
) {
    let view = cx.open_window(size(px(500.), px(240.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(|cx| {
            InputState::new(cx)
                .password()
                .password_ime(true)
                .initial_value("mø🦉")
        });
        let field = cx.new(|cx| Input::new(&editor, window, cx));
        Harness {
            field,
            editor,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |view, window, cx| {
        window.activate_window();
        window.focus(&view.editor.focus_handle(cx), cx);
        view.editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(1..2, window, cx);
            editor.replace_and_mark_text_in_range(None, "yo", Some(1..1), window, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let toggle = visual
        .debug_bounds("fusion-input-visibility")
        .unwrap()
        .center();
    visual.simulate_click(toggle, Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, window, cx| {
        let editor = view.editor.read(cx);
        assert!(editor.is_password_visible());
        assert!(editor.is_composing());
        assert_eq!(editor.value().as_ref(), "myo🦉");
        assert!(editor.focus_handle(cx).is_focused(window));
        view.editor.update(cx, |editor, cx| {
            assert_eq!(
                editor.selected_text_range(false, window, cx).unwrap().range,
                2..2
            );
            assert!(editor.surrounding_text(256, window, cx).is_none());
        });
        window.focus(&view.field.read(cx).visibility_focus.clone(), cx);
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, _, cx| {
        assert!(!view.editor.read(cx).is_password_visible());
        assert!(view.editor.read(cx).is_composing());
    })
    .unwrap();
    visual.simulate_event(gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
    });
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, _, cx| {
        assert!(view.editor.read(cx).is_password_visible());
        view.editor
            .update(cx, |editor, cx| editor.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(toggle, Default::default());
    view.update(&mut visual.cx, |view, _, cx| {
        assert!(!view.editor.read(cx).is_password_visible());
        assert_eq!(view.editor.read(cx).value().as_ref(), "mø🦉");
    })
    .unwrap();
}

#[gpui::test]
fn multiline_resize_clamps_preserves_composition_and_stops_on_release(cx: &mut TestAppContext) {
    use crate::components::input::{InputMode, InputResize};
    use gpui::{MouseButton, point};
    let view = cx.open_window(size(px(500.), px(400.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(|cx| {
            InputState::new(cx)
                .multiline()
                .initial_value("Filed\nAwaiting review")
        });
        let field = cx.new(|cx| {
            Input::new(&editor, window, cx)
                .rows(3)
                .resizable(true)
                .resize_limits(size(px(180.), px(80.)), size(px(480.), px(240.)))
        });
        Harness {
            field,
            editor,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |view, window, cx| {
        window.activate_window();
        window.focus(&view.editor.focus_handle(cx), cx);
        view.editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "ab", Some(1..1), window, cx)
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let before = visual.debug_bounds("fusion-input").unwrap();
    let handle = visual.debug_bounds("fusion-input-resize").unwrap().center();
    let outside = handle + point(px(500.), px(500.));
    visual.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(outside, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-input").unwrap().size,
        size(px(480.), px(240.))
    );
    visual.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(handle, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-input").unwrap().size,
        size(px(480.), px(240.))
    );
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.editor.read(cx).is_composing());
        assert_eq!(
            view.editor.read(cx).value().as_ref(),
            "Filed\nAwaiting reviewab"
        );
        assert!(view.editor.focus_handle(cx).is_focused(window));
        view.field.update(cx, |field, cx| {
            field.resize.axes = InputResize::Vertical;
            field.reset_size(cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-input").unwrap().size,
        before.size
    );
    let handle = visual.debug_bounds("fusion-input-resize").unwrap().center();
    visual.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        handle - point(px(1000.), px(1000.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-input").unwrap().size,
        size(before.size.width, px(80.))
    );
    view.update(&mut visual.cx, |view, _, cx| {
        view.field
            .update(cx, |field, cx| field.set_resizable(false, cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-resize").is_none());
    view.update(&mut visual.cx, |view, _, cx| {
        assert!(view.field.read(cx).resize.dragging.is_none());
        view.field
            .update(cx, |field, cx| field.set_resizable(true, cx));
        view.editor
            .update(cx, |editor, cx| editor.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-resize").is_none());
    view.update(&mut visual.cx, |view, _, cx| {
        view.editor.update(cx, |editor, cx| {
            editor.set_disabled(false, cx);
            editor.set_mode(InputMode::Text, cx);
        })
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-resize").is_none());
}

#[gpui::test]
fn built_in_clear_is_undoable_and_search_submits_from_either_side(cx: &mut TestAppContext) {
    use super::InputSide;
    use std::{cell::RefCell, rc::Rc};
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let sink = submitted.clone();
    let view = cx.open_window(size(px(500.), px(240.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(|cx| InputState::new(cx).initial_value("Δraft🦊"));
        let field = cx.new(|cx| {
            Input::new(&editor, window, cx)
                .clearable(true)
                .search_icon(InputSide::Leading)
                .on_submit(move |value, _, _| sink.borrow_mut().push(value.to_string()))
        });
        Harness {
            field,
            editor,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |_, window, _| window.activate_window())
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let search = visual.debug_bounds("fusion-input-leading-icon").unwrap();
    let clear = visual.debug_bounds("fusion-input-clear").unwrap();
    assert!(search.origin.x < clear.origin.x);
    visual.simulate_click(search.center(), Default::default());
    assert_eq!(&*submitted.borrow(), &["Δraft🦊"]);
    view.update(&mut visual.cx, |view, window, cx| {
        window.focus(&view.field.read(cx).clear_focus.clone(), cx);
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    for down in [true, false] {
        let keystroke = gpui::Keystroke::parse("enter").unwrap();
        if down {
            visual.simulate_event(gpui::KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            });
        } else {
            visual.simulate_event(gpui::KeyUpEvent { keystroke });
        }
    }
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-clear").is_none());
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.editor.read(cx).value().is_empty());
        assert!(view.editor.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    visual.simulate_keystrokes("ctrl-z");
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(view.editor.read(cx).value().as_ref(), "Δraft🦊");
        view.field.update(cx, |field, cx| {
            field.suffix = field.prefix.take();
            cx.notify();
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let search = visual.debug_bounds("fusion-input-trailing-icon").unwrap();
    assert!(search.origin.x < visual.debug_bounds("fusion-input-clear").unwrap().origin.x);
    visual.simulate_click(search.center(), Default::default());
    assert_eq!(submitted.borrow().len(), 2);
    view.update(&mut visual.cx, |view, _, cx| {
        view.editor
            .update(cx, |editor, cx| editor.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(search.center(), Default::default());
    let clear = visual.debug_bounds("fusion-input-clear").unwrap();
    visual.simulate_click(clear.center(), Default::default());
    assert_eq!(submitted.borrow().len(), 2);
    view.update(&mut visual.cx, |view, _, cx| {
        assert_eq!(view.editor.read(cx).value().as_ref(), "Δraft🦊");
        view.editor.update(cx, |editor, cx| {
            editor.set_disabled(false, cx);
            editor.set_read_only(true, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-clear").is_none());
}

#[gpui::test]
fn icon_and_custom_adornments_replace_the_search_shortcut(cx: &mut TestAppContext) {
    use super::{InputIcon, InputSide};
    use std::{cell::Cell, rc::Rc};
    let clicks = Rc::new(Cell::new(0));
    let callback = clicks.clone();
    let view = cx.open_window(size(px(500.), px(240.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let editor = cx.new(InputState::new);
        let field = cx.new(|cx| {
            Input::new(&editor, window, cx)
                .search_icon(InputSide::Leading)
                .prefix(|_, _| {
                    div()
                        .debug_selector(|| "custom-prefix".into())
                        .child("@")
                        .into_any_element()
                })
                .search_icon(InputSide::Trailing)
                .icon(
                    InputSide::Trailing,
                    InputIcon::new(gpui_icons::GpuiIcons::LucideX)
                        .on_click("Dismiss", move |_, _| callback.set(callback.get() + 1)),
                )
                .on_submit(|_, _, _| panic!("the replaced search action must not run"))
        });
        Harness {
            field,
            editor,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-input-leading-icon").is_none());
    assert!(visual.debug_bounds("custom-prefix").is_some());
    let icon = visual.debug_bounds("fusion-input-trailing-icon").unwrap();
    visual.simulate_click(icon.center(), Default::default());
    assert_eq!(clicks.get(), 1);
    view.update(&mut visual.cx, |view, _, cx| {
        view.editor
            .update(cx, |editor, cx| editor.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(icon.center(), Default::default());
    assert_eq!(clicks.get(), 1);
}
