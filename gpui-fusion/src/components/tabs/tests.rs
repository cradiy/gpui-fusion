use super::*;
use crate::{default_theme, theme::ThemeStore};
use gpui::{Keystroke, TestAppContext, VisualTestContext, size};
use std::cell::RefCell;

struct Page {
    count: usize,
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| format!("page-value-{}", self.count))
            .child(self.count.to_string())
    }
}
struct Harness {
    tabs: Entity<Tabs>,
    theme: Entity<ThemeRuntime>,
    page: Entity<Page>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().child(self.tabs.clone())
    }
}
fn press(cx: &mut VisualTestContext, key: &str) {
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.update(|window, cx| window.draw(cx).clear());
}
#[gpui::test]
fn keyboard_skips_disabled_pages_and_preserves_hidden_state_across_theme_changes(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let view = cx.open_window(size(px(460.), px(300.)), |window, cx| {
        let theme = ThemeRuntime::install(
            crate::theme::ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let page = cx.new(|_| Page { count: 7 });
        let other = cx.new(|_| Page { count: 2 });
        let tabs = cx.new(|cx| {
            Tabs::new(
                "Library",
                [
                    Tab::new("saved", "Saved").panel(page.clone()),
                    Tab::new("pending", "Pending").disabled(true),
                    Tab::new("recent", "Recent").panel(other),
                ],
                window,
                cx,
            )
            .unwrap()
            .animated(false)
            .on_change(move |value, _, _| output.borrow_mut().push(value))
        });
        tabs.read(cx).focus_handle(cx).focus(window, cx);
        Harness { tabs, theme, page }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    assert!(visual.debug_bounds("page-value-7").is_some());
    press(&mut visual, "right");
    assert!(visual.debug_bounds("page-value-7").is_none());
    assert!(visual.debug_bounds("page-value-2").is_some());
    view.update(&mut visual.cx, |h, window, cx| {
        assert_eq!(h.tabs.read(cx).selected().unwrap().as_ref(), "recent");
        assert!(h.tabs.read(cx).items[2].focus.is_focused(window));
        h.page.update(cx, |p, cx| {
            p.count = 9;
            cx.notify();
        });
        h.theme.update(cx, |t, cx| {
            t.set_mode("dark", cx).unwrap();
            t.set_material_mode(MaterialMode::Glass, cx);
        });
    })
    .unwrap();
    press(&mut visual, "home");
    assert!(visual.debug_bounds("page-value-9").is_some());
    assert!(visual.debug_bounds("page-value-2").is_none());
    assert_eq!(
        &*events.borrow(),
        &[SharedString::from("recent"), SharedString::from("saved")]
    );
    view.update(&mut visual.cx, |h, window, cx| {
        h.tabs.update(cx, |tabs, cx| {
            assert!(tabs.set_selected("pending", cx).is_err());
            assert_eq!(tabs.selected().unwrap().as_ref(), "saved");
            tabs.set_selected("recent", cx).unwrap();
            assert!(tabs.items[0].focus.is_focused(window));
            assert!(tabs.style_error().is_none());
        });
    })
    .unwrap();
    assert_eq!(events.borrow().len(), 2);
}
#[gpui::test]
fn manual_activation_keeps_the_panel_until_confirmation(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(420.), px(180.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let tabs = cx.new(|cx| {
            Tabs::new(
                "Sources",
                [Tab::new("local", "Local"), Tab::new("shared", "Shared")],
                window,
                cx,
            )
            .unwrap()
            .manual_activation(true)
            .animated(false)
        });
        tabs.read(cx).focus_handle(cx).focus(window, cx);
        Harness {
            tabs,
            theme,
            page: cx.new(|_| Page { count: 0 }),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    press(&mut visual, "end");
    view.update(&mut visual.cx, |tabs, window, cx| {
        assert_eq!(tabs.tabs.read(cx).selected().unwrap().as_ref(), "local");
        assert!(tabs.tabs.read(cx).items[1].focus.is_focused(window));
    })
    .unwrap();
    press(&mut visual, "space");
    view.update(&mut visual.cx, |tabs, _, cx| {
        assert_eq!(tabs.tabs.read(cx).selected().unwrap().as_ref(), "shared")
    })
    .unwrap();
}

#[gpui::test]
fn glass_selection_moves_one_surface_and_retargets_without_jumping(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(640.), px(220.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        theme.update(cx, |t, cx| t.set_material_mode(MaterialMode::Glass, cx));
        let tabs = cx.new(|cx| {
            Tabs::new(
                "Storage",
                [
                    Tab::new("disks", "Disks"),
                    Tab::new("snapshots", "Snapshots"),
                    Tab::new("replicas", "Remote replicas"),
                ],
                window,
                cx,
            )
            .unwrap()
            .transition_duration(Duration::from_millis(400))
        });
        Harness {
            tabs,
            theme,
            page: cx.new(|_| Page { count: 0 }),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    let first = visual.debug_bounds("fusion-tab-indicator").unwrap();
    let destination = visual.debug_bounds("fusion-tab-2").unwrap();
    view.update(&mut visual.cx, |h, _, cx| {
        h.tabs
            .update(cx, |tabs, cx| tabs.set_selected("replicas", cx).unwrap())
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(visual.debug_bounds("fusion-tab-indicator").unwrap(), first);
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(100));
    visual.update(|window, cx| window.draw(cx).clear());
    let middle = visual.debug_bounds("fusion-tab-indicator").unwrap();
    assert!(middle.left() > first.left() && middle.left() < destination.left());
    assert!(middle.size.width > first.size.width && middle.size.width < destination.size.width);
    view.update(&mut visual.cx, |h, _, cx| {
        h.tabs
            .update(cx, |tabs, cx| tabs.set_selected("disks", cx).unwrap())
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(visual.debug_bounds("fusion-tab-indicator").unwrap(), middle);
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(500));
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(visual.debug_bounds("fusion-tab-indicator").unwrap(), first);
}
