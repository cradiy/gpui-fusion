use gpui::{
    App, AppContext, Context, Entity, Focusable, IntoElement, KeyUpEvent, Keystroke, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px, size,
};
use gpui_fusion::{
    components::checkbox::{Checkbox, CheckboxPart, CheckboxState},
    default_theme,
    style::{MaterialMode, StylePatch, Styled as FusionStyled},
    theme::{ThemeStore, runtime::ThemeRuntime},
};
use std::{cell::RefCell, rc::Rc};

struct Harness {
    control: Entity<Checkbox>,
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
fn indicator_activation_label_exclusion_keyboard_cancellation_and_disabled_input(
    cx: &mut TestAppContext,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let received = changes.clone();
    let handle = cx.open_window(size(px(380.), px(140.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            Checkbox::new("Attach metadata", CheckboxState::Mixed, window, cx)
                .animated(false)
                .on_change(move |state, _, _| received.borrow_mut().push(state))
        });
        Harness { control, theme }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let label = visual
        .debug_bounds("fusion-checkbox-label")
        .unwrap()
        .center();
    let indicator = visual
        .debug_bounds("fusion-checkbox-indicator")
        .unwrap()
        .center();
    visual.simulate_click(label, Default::default());
    assert!(changes.borrow().is_empty());
    visual.simulate_click(indicator, Default::default());
    assert_eq!(&*changes.borrow(), &[CheckboxState::Checked]);
    visual.simulate_keystrokes("space");
    visual.simulate_keystrokes("space");
    assert_eq!(changes.borrow().len(), 1);
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    assert_eq!(
        &*changes.borrow(),
        &[CheckboxState::Checked, CheckboxState::Unchecked]
    );
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    assert_eq!(changes.borrow().len(), 2);
    visual.simulate_keystrokes("space escape");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    assert_eq!(changes.borrow().len(), 2);
    visual.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: indicator,
        ..Default::default()
    });
    visual.update(|window, cx| window.draw(cx).clear());
    let outside = point(px(370.), px(130.));
    visual.simulate_event(MouseMoveEvent {
        position: outside,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    visual.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: outside,
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), 2);
    visual.simulate_keystrokes("space");
    visual.update(|window, _| window.blur());
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    assert_eq!(changes.borrow().len(), 2);
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |c, cx| {
                c.set_state(CheckboxState::Mixed, cx);
                c.set_disabled(true, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(indicator, Default::default());
    visual.simulate_keystrokes("space");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    handle
        .update(&mut visual.cx, |h, _, cx| {
            assert_eq!(h.control.read(cx).state(), CheckboxState::Mixed)
        })
        .unwrap();
    assert_eq!(changes.borrow().len(), 2);
}

#[gpui::test]
fn theme_material_and_part_updates_preserve_state_focus_and_geometry(cx: &mut TestAppContext) {
    let handle = cx.open_window(size(px(380.), px(140.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            Checkbox::new("Embed preview", true, window, cx)
                .animated(false)
                .part(CheckboxPart::Indicator, |s| s.w(px(26.)).h(px(26.)))
        });
        Harness { control, theme }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let original = visual.debug_bounds("fusion-checkbox-indicator").unwrap();
    assert_eq!(original.size.width, px(26.));
    handle
        .update(&mut visual.cx, |h, window, cx| {
            h.control.read(cx).focus_handle(cx).focus(window, cx);
            h.theme.update(cx, |t, cx| {
                t.set_mode("dark", cx).unwrap();
                t.set_material_mode(MaterialMode::Glass, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-checkbox-indicator").unwrap(),
        original
    );
    handle
        .update(&mut visual.cx, |h, window, cx| {
            assert_eq!(h.control.read(cx).state(), CheckboxState::Checked);
            assert!(h.control.read(cx).focus_handle(cx).is_focused(window));
            assert!(h.control.read(cx).style_error().is_none());
            h.control.update(cx, |c, cx| {
                c.set_style(StylePatch::default(), cx);
                c.set_variant(Some("missing".into()), cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            assert!(h.control.read(cx).style_error().is_some());
            h.control.update(cx, |c, cx| {
                c.set_variant(None, cx);
                c.set_material_mode(Some(MaterialMode::Plain), cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual
            .debug_bounds("fusion-checkbox-indicator")
            .unwrap()
            .size
            .width,
        px(20.)
    );
    handle
        .update(&mut visual.cx, |h, _, cx| {
            assert!(h.control.read(cx).style_error().is_none())
        })
        .unwrap();
}
