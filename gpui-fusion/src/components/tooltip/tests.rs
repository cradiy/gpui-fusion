use super::*;
use crate::{components::button::Button, default_theme, theme::ThemeStore};
use gpui::{AppContext, TestAppContext, VisualTestContext, point, size};
use std::{cell::Cell, rc::Rc};

struct Harness {
    tip: Entity<Tooltip>,
    button: Entity<Button>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_2()
            .flex()
            .items_start()
            .child(self.tip.clone())
    }
}
fn tick(visual: &mut VisualTestContext, ms: u64) {
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(ms));
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());
}
fn setup(cx: &mut TestAppContext, activations: Rc<Cell<usize>>) -> gpui::WindowHandle<Harness> {
    cx.open_window(size(px(260.), px(180.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let button = cx.new(|cx| {
            Button::new("Restore", window, cx)
                .on_click(move |_, _| activations.set(activations.get() + 1))
        });
        let tip = cx.new(|cx| {
            Tooltip::new("Restore the most recent snapshot", &button, window, cx)
                .show_delay(Duration::from_millis(200))
                .hide_delay(Duration::from_millis(80))
                .transition_duration(Duration::ZERO)
                .max_width(px(170.))
        });
        Harness { tip, button, theme }
    })
}
#[gpui::test]
fn hover_delay_cancellation_popup_hover_and_activation(cx: &mut TestAppContext) {
    let activations = Rc::new(Cell::new(0));
    let handle = setup(cx, activations.clone());
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let anchor = visual.debug_bounds("fusion-tooltip-anchor").unwrap();
    let away = point(px(245.), px(165.));
    visual.simulate_mouse_move(anchor.center(), None, Default::default());
    tick(&mut visual, 100);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
    visual.simulate_mouse_move(away, None, Default::default());
    tick(&mut visual, 300);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
    visual.simulate_mouse_move(anchor.center(), None, Default::default());
    tick(&mut visual, 201);
    let bubble = visual.debug_bounds("fusion-tooltip-bubble").unwrap();
    assert!(
        bubble.top() >= anchor.bottom(),
        "top placement must flip at the viewport edge"
    );
    assert!(bubble.right() <= px(260.) && bubble.bottom() <= px(180.));
    visual.simulate_mouse_move(bubble.center(), None, Default::default());
    tick(&mut visual, 100);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_some());
    visual.simulate_mouse_move(away, None, Default::default());
    tick(&mut visual, 81);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
    visual.simulate_mouse_move(anchor.center(), None, Default::default());
    tick(&mut visual, 201);
    visual.simulate_click(anchor.center(), Default::default());
    tick(&mut visual, 300);
    assert_eq!(activations.get(), 1);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
}
#[gpui::test]
fn keyboard_escape_theme_changes_and_disabled_hint(cx: &mut TestAppContext) {
    let handle = setup(cx, Rc::new(Cell::new(0)));
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    visual.simulate_keystrokes("tab");
    handle
        .update(&mut visual.cx, |h, window, cx| {
            h.button.focus_handle(cx).focus(window, cx)
        })
        .unwrap();
    tick(&mut visual, 1);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_some());
    handle
        .update(&mut visual.cx, |h, window, cx| {
            h.theme.update(cx, |theme, cx| {
                theme.set_mode("dark", cx).unwrap();
                theme.set_material_mode(MaterialMode::Glass, cx);
            });
            assert!(h.button.focus_handle(cx).is_focused(window));
        })
        .unwrap();
    tick(&mut visual, 1);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_some());
    visual.simulate_keystrokes("escape");
    tick(&mut visual, 300);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
    handle
        .update(&mut visual.cx, |h, window, cx| {
            assert!(h.button.focus_handle(cx).is_focused(window));
            window.blur();
            h.tip
                .update(cx, |tip, cx| tip.set_disabled(true, window, cx));
        })
        .unwrap();
    let anchor = visual.debug_bounds("fusion-tooltip-anchor").unwrap();
    visual.simulate_mouse_move(anchor.center(), None, Default::default());
    tick(&mut visual, 500);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_none());
    handle
        .update(&mut visual.cx, |h, window, cx| {
            h.tip
                .update(cx, |tip, cx| tip.set_disabled(false, window, cx));
        })
        .unwrap();
    visual.run_until_parked();
    tick(&mut visual, 201);
    assert!(visual.debug_bounds("fusion-tooltip-bubble").is_some());
}
