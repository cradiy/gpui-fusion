use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    components::button::Button, default_theme, theme::ThemeStore, theme::runtime::ThemeRuntime,
};
use std::{cell::Cell, rc::Rc};

struct Harness {
    button: Entity<Button>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.button.clone())
    }
}

#[gpui::test]
fn activation_theme_reload_and_disabling_preserve_control_identity(cx: &mut TestAppContext) {
    let count = Rc::new(Cell::new(0));
    let counter = count.clone();
    let view = cx.open_window(size(px(400.), px(180.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let button = cx.new(|cx| {
            Button::new("Confirm", window, cx).on_click(move |_, _| counter.set(counter.get() + 1))
        });
        Harness { button, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let bounds = visual.debug_bounds("fusion-button").unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    assert_eq!(count.get(), 1);
    press(&mut visual, "space");
    press(&mut visual, "enter");
    assert_eq!(count.get(), 3);
    view.update(&mut visual.cx, |h, window, cx| {
        let focus = h.button.read(cx).focus_handle(cx);
        h.theme
            .update(cx, |runtime, cx| runtime.set_mode("dark", cx).unwrap());
        assert!(focus.is_focused(window));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    press(&mut visual, "space");
    assert_eq!(count.get(), 4);
    view.update(&mut visual.cx, |h, _, cx| {
        h.button
            .update(cx, |button, cx| button.set_disabled(true, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(bounds.center(), Default::default());
    press(&mut visual, "space");
    press(&mut visual, "enter");
    assert_eq!(count.get(), 4);
}

#[test]
fn glass_recipe_roundtrips_and_instance_can_remove_material() {
    use gpui_fusion::{
        style::{Material, State, StateSelector, StylePatch, Styled},
        theme::{ResolveContext, Theme},
    };
    let theme = Theme::from_json(&serde_json::to_string(&default_theme()).unwrap()).unwrap();
    let context = ResolveContext::new(&theme, "dark", "neutral").unwrap();
    let styles = gpui_fusion::default_styles();
    let recipe = styles.recipe("button").unwrap();
    let plain = StylePatch::default();
    let rest = recipe
        .resolve(
            None,
            gpui_fusion::style::MaterialMode::Glass,
            &plain,
            &context,
            &StateSelector::default(),
            None,
        )
        .unwrap();
    let pressed = recipe
        .resolve(
            None,
            gpui_fusion::style::MaterialMode::Glass,
            &plain,
            &context,
            &State::Pressed.into(),
            None,
        )
        .unwrap();
    let rest_material = rest.material.unwrap();
    let pressed_material = pressed.material.unwrap();
    assert_eq!(rest_material.frost, pressed_material.frost);
    assert_eq!(rest_material.blur_radius, pressed_material.blur_radius);
    assert!(pressed_material.refraction > rest_material.refraction);
    let instance = plain.material(Material::Plain);
    let result = recipe
        .resolve(
            None,
            gpui_fusion::style::MaterialMode::Glass,
            &instance,
            &context,
            &State::Pressed.into(),
            None,
        )
        .unwrap();
    assert!(result.material.is_none());
}

fn press(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_keystrokes(key);
    visual.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap(),
    });
}

#[test]
fn dashed_border_survives_json_and_explicit_solid_override() {
    use gpui_fusion::{
        style::{StateSelector, StylePatch, Styled},
        theme::{ResolveContext, Theme},
    };
    let theme = Theme::from_json(&serde_json::to_string(&default_theme()).unwrap()).unwrap();
    let context = ResolveContext::new(&theme, "light", "neutral").unwrap();
    let styles = gpui_fusion::default_styles();
    let recipe = styles.recipe("button").unwrap();
    let resolve = |patch: &StylePatch| {
        recipe
            .resolve(
                Some("dashed"),
                gpui_fusion::style::MaterialMode::Plain,
                patch,
                &context,
                &StateSelector::default(),
                None,
            )
            .unwrap()
    };
    let dashed = resolve(&StylePatch::default());
    assert_eq!(
        dashed.to_gpui().border_style,
        Some(gpui::BorderStyle::Dashed)
    );
    let solid = resolve(&StylePatch::default().border_solid());
    assert_eq!(solid.to_gpui().border_style, Some(gpui::BorderStyle::Solid));
    let changes = solid.changes_from(&dashed);
    assert!(changes.paint);
    assert!(!changes.layout);
}
