use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    components::switch::{Switch, SwitchContent},
    default_theme,
    theme::ThemeStore,
    theme::runtime::ThemeRuntime,
};
use std::{cell::RefCell, rc::Rc};

struct Harness {
    switch: Entity<Switch>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.switch.clone())
    }
}
#[gpui::test]
fn switch_activation_drag_cancellation_and_theme_preserve_state(cx: &mut TestAppContext) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let events = changes.clone();
    let view = cx.open_window(size(px(360.), px(200.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let switch = cx.new(|cx| {
            Switch::new("Sync", false, window, cx)
                .material_mode(gpui_fusion::style::MaterialMode::Glass)
                .content(
                    SwitchContent::Text("0".into()),
                    SwitchContent::Text("1".into()),
                )
                .animated(false)
                .on_change(move |value, _, _| events.borrow_mut().push(value))
        });
        Harness { switch, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let bounds = visual.debug_bounds("fusion-switch").unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    assert_eq!(&*changes.borrow(), &[true]);
    visual.simulate_keystrokes("space");
    visual.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
    });
    assert_eq!(&*changes.borrow(), &[true, false]);
    view.update(&mut visual.cx, |h, window, cx| {
        h.switch.update(cx, |s, cx| s.set_checked(true, cx));
        h.theme.update(cx, |t, cx| t.set_mode("dark", cx).unwrap());
        assert!(h.switch.read(cx).checked());
        assert!(h.switch.read(cx).focus_handle(cx).is_focused(window));
    })
    .unwrap();
    assert_eq!(changes.borrow().len(), 2);
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: bounds.center(),
        ..Default::default()
    });
    visual.update(|window, cx| window.draw(cx).clear());
    let outside = gpui::point(bounds.left() - px(15.), bounds.center().y);
    visual.simulate_event(gpui::MouseMoveEvent {
        position: outside,
        pressed_button: Some(gpui::MouseButton::Left),
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), 2);
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.switch.read(cx).checked())
    })
    .unwrap();
    visual.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: outside,
        ..Default::default()
    });
    assert_eq!(&*changes.borrow(), &[true, false, false]);
    visual.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: bounds.center(),
        ..Default::default()
    });
    view.update(&mut visual.cx, |h, _, cx| {
        h.switch.update(cx, |s, cx| s.set_disabled(true, cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: bounds.center(),
        ..Default::default()
    });
    visual.simulate_click(bounds.center(), Default::default());
    visual.simulate_keystrokes("space");
    assert_eq!(changes.borrow().len(), 3);
}

#[gpui::test]
fn style_cache_refreshes_for_theme_instance_and_invalid_variants(cx: &mut TestAppContext) {
    use gpui_fusion::style::{StylePatch, Styled};
    let view = cx.open_window(size(px(360.), px(200.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let switch = cx.new(|cx| Switch::new("Sync", false, window, cx).animated(false));
        Harness { switch, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    let original = visual.debug_bounds("fusion-switch").unwrap().size.width;
    view.update(&mut visual.cx, |h, _, cx| {
        h.switch.update(cx, |s, cx| {
            s.set_style(StylePatch::default().w(px(94.)), cx)
        })
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-switch").unwrap().size.width,
        px(94.)
    );
    view.update(&mut visual.cx, |h, _, cx| {
        h.switch
            .update(cx, |s, cx| s.set_style(StylePatch::default(), cx));
        let mut styles = gpui_fusion::default_styles();
        let recipe = styles.recipes.get_mut("switch").unwrap();
        recipe.base = std::mem::take(&mut recipe.base).w(px(86.));
        h.theme.update(cx, |t, cx| {
            t.replace_styles_json(&serde_json::to_string(&styles).unwrap(), cx)
                .unwrap()
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-switch").unwrap().size.width,
        px(86.)
    );
    assert_ne!(original, px(86.));
    view.update(&mut visual.cx, |h, _, cx| {
        h.switch
            .update(cx, |s, cx| s.set_variant(Some("missing".into()), cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.switch.read(cx).style_error().is_some());
        h.switch.update(cx, |s, cx| {
            s.set_variant(None, cx);
            s.set_material_mode(Some(gpui_fusion::style::MaterialMode::Glass), cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.switch.read(cx).style_error().is_none())
    })
    .unwrap();
    assert_ne!(
        visual.debug_bounds("fusion-switch").unwrap().size.width,
        px(86.)
    );
}
