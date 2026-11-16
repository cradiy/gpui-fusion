use gpui::{
    App, AppContext, Context, Entity, Focusable, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Render, SharedString, TestAppContext, VisualTestContext, Window,
    div, point, prelude::*, px, size,
};
use gpui_fusion::{
    components::segmented::{Segment, Segmented},
    default_theme,
    style::{MaterialMode, StylePatch, Styled as FusionStyled},
    theme::{ThemeStore, runtime::ThemeRuntime},
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn glass_item_colors_stay_stable_across_selection_and_pointer_states() {
    use gpui_fusion::{
        default_styles,
        style::{PartName, State, StateSelector, Token},
        theme::ResolveContext,
    };
    let theme = default_theme();
    let styles = default_styles();
    let recipe = styles.recipe("segmented").unwrap();
    let instance = StylePatch::default().part(PartName("indicator".into()), |s| {
        s.text_color(Token::<gpui::Hsla>::new("primary"))
    });
    for mode in ["light", "dark"] {
        let context = ResolveContext::new(&theme, mode, "neutral").unwrap();
        let resolve = |material, states: &StateSelector, part| {
            recipe
                .resolve(None, material, &instance, &context, states, Some(part))
                .unwrap()
        };
        let idle = StateSelector::default();
        let outside = resolve(MaterialMode::Glass, &idle, "item").foreground;
        let inside = resolve(MaterialMode::Glass, &idle, "indicator").foreground;
        assert_ne!(inside, outside);
        for states in [
            State::Hovered.into(),
            State::Selected.into(),
            StateSelector([State::Selected, State::Hovered, State::Pressed].into()),
        ] {
            assert_eq!(
                resolve(MaterialMode::Glass, &states, "item").foreground,
                outside
            );
            assert_eq!(
                resolve(MaterialMode::Glass, &states, "indicator").foreground,
                inside
            );
        }
        let disabled = StateSelector([State::Disabled, State::Selected, State::Hovered].into());
        assert_eq!(
            resolve(MaterialMode::Glass, &disabled, "item").foreground,
            resolve(MaterialMode::Plain, &disabled, "item").foreground,
        );
        assert_ne!(
            resolve(MaterialMode::Plain, &State::Selected.into(), "item").foreground,
            resolve(MaterialMode::Plain, &idle, "item").foreground,
        );
    }
}

struct Harness {
    control: Entity<Segmented>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.control.clone())
    }
}
fn install(cx: &mut App) -> Entity<ThemeRuntime> {
    ThemeRuntime::install(
        ThemeStore::new(default_theme(), "light").unwrap(),
        "neutral",
        cx,
    )
    .unwrap()
}
#[gpui::test]
fn selection_click_keyboard_drag_and_cancel(cx: &mut TestAppContext) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let callback = changes.clone();
    let view = cx.open_window(size(px(460.), px(200.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            Segmented::new(
                "Encoding",
                [
                    Segment::new("raw", "Raw"),
                    Segment::new("reserved", "Reserved").disabled(true),
                    Segment::new("lossless", "Lossless"),
                    Segment::new("lossy", "Lossy"),
                ],
                window,
                cx,
            )
            .unwrap()
            .animated(false)
            .on_change(move |value, _, _| callback.borrow_mut().push(value))
        });
        Harness { control, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let first = visual.debug_bounds("fusion-segment-0").unwrap().center();
    let disabled = visual.debug_bounds("fusion-segment-1").unwrap().center();
    let third = visual.debug_bounds("fusion-segment-2").unwrap().center();
    let last = visual.debug_bounds("fusion-segment-3").unwrap().center();
    visual.simulate_click(disabled, Default::default());
    assert!(changes.borrow().is_empty());
    visual.simulate_click(third, Default::default());
    assert_eq!(&*changes.borrow(), &[SharedString::from("lossless")]);
    visual.simulate_keystrokes("left");
    visual.simulate_keystrokes("left");
    visual.simulate_keystrokes("home");
    visual.simulate_keystrokes("end");
    assert_eq!(
        changes
            .borrow()
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>(),
        ["lossless", "raw", "lossy", "raw", "lossy"]
    );
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: last,
        ..Default::default()
    });
    visual.update(|window, cx| window.draw(cx).clear());
    let outside = point(first.x - px(50.), first.y);
    visual.simulate_event(MouseMoveEvent {
        position: outside,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), 5);
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: outside,
        ..Default::default()
    });
    assert_eq!(changes.borrow().last().unwrap().as_ref(), "raw");
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: first,
        ..Default::default()
    });
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(MouseMoveEvent {
        position: last,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    visual.simulate_keystrokes("escape");
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: last,
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), 6);
    view.update(&mut visual.cx, |h, window, cx| {
        h.control.update(cx, |s, cx| {
            assert!(s.set_selected("reserved", cx).is_err());
            s.set_selected("lossless", cx).unwrap();
        });
        h.theme.update(cx, |theme, cx| {
            theme.set_mode("dark", cx).unwrap();
            theme.set_material_mode(MaterialMode::Glass, cx);
        });
        assert_eq!(h.control.read(cx).selected().unwrap().as_ref(), "lossless");
        assert!(h.control.read(cx).focus_handle(cx).is_focused(window));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: third,
        ..Default::default()
    });
    view.update(&mut visual.cx, |h, _, cx| {
        h.control.update(cx, |s, cx| s.set_disabled(true, cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: first,
        ..Default::default()
    });
    visual.simulate_click(first, Default::default());
    visual.simulate_keystrokes("right");
    assert_eq!(changes.borrow().len(), 6);
}

#[gpui::test]
fn option_validation_empty_controls_and_style_reload(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(460.), px(200.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            assert!(
                Segmented::new(
                    "Formats",
                    [Segment::new("pcm", "PCM"), Segment::new("pcm", "Other")],
                    window,
                    cx
                )
                .is_err()
            );
            Segmented::new("Formats", [], window, cx)
                .unwrap()
                .animated(false)
        });
        Harness { control, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, window, cx| {
        assert!(h.control.read(cx).selected().is_none());
        h.control = cx.new(|cx| {
            Segmented::new(
                "Formats",
                [Segment::new("pcm", "PCM"), Segment::new("flac", "FLAC")],
                window,
                cx,
            )
            .unwrap()
            .animated(false)
        });
        cx.notify();
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    let before = visual.debug_bounds("fusion-segmented").unwrap();
    view.update(&mut visual.cx, |h, _, cx| {
        h.control.update(cx, |s, cx| {
            s.set_style(
                StylePatch::default()
                    .w(px(360.))
                    .part(gpui_fusion::style::PartName("item".into()), |s| {
                        s.text_size(px(16.))
                    }),
                cx,
            )
        });
        h.theme
            .update(cx, |t, cx| t.set_material_mode(MaterialMode::Glass, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-segmented").unwrap().size.width > before.size.width);
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.control.read(cx).style_error().is_none());
        h.control.update(cx, |s, cx| {
            s.set_style(
                StylePatch::default().part(gpui_fusion::style::PartName("invalid".into()), |s| {
                    s.border(px(1.))
                }),
                cx,
            );
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.control.read(cx).style_error().is_some());
        h.control
            .update(cx, |s, cx| s.set_style(StylePatch::default(), cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.control.read(cx).style_error().is_none())
    })
    .unwrap();
}
