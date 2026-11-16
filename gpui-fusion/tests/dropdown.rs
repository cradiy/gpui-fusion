use gpui::{
    Context, Entity, FocusHandle, Focusable, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton,
    SharedString, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    components::dropdown::{
        Dropdown, DropdownAlign, DropdownPlacement, DropdownPresentation, DropdownSide, MenuItem,
    },
    default_theme,
    style::MaterialMode,
    theme::{ThemeStore, runtime::ThemeRuntime},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

struct Harness {
    menu: Entity<Dropdown>,
    theme: Entity<ThemeRuntime>,
    outside: FocusHandle,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let outside = self.outside.clone();
        div()
            .p_4()
            .flex()
            .flex_col()
            .items_start()
            .gap_4()
            .child(self.menu.clone())
            .child(
                div()
                    .absolute()
                    .left(px(210.))
                    .top(px(20.))
                    .id("outside")
                    .debug_selector(|| "outside".into())
                    .track_focus(&self.outside)
                    .tab_stop(true)
                    .w(px(100.))
                    .h(px(30.))
                    .child("Outside")
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        outside.focus(window, cx)
                    }),
            )
    }
}
fn press(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse(key).unwrap(),
    });
    visual.update(|window, cx| window.draw(cx).clear());
}
#[gpui::test]
fn menu_toggle_navigation_dismissal_and_theme_keep_trigger_identity(cx: &mut TestAppContext) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let callback = actions.clone();
    let view = cx.open_window(size(px(480.), px(300.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let menu = cx.new(|cx| {
            Dropdown::new(
                "Commands",
                [
                    MenuItem::new("archive", "Archive"),
                    MenuItem::new("sync", "Synchronize").disabled(true),
                    MenuItem::Separator,
                    MenuItem::new("inspect", "Inspect"),
                ],
                window,
                cx,
            )
            .open_label("Close commands")
            .transition_duration(Duration::ZERO)
            .on_action(move |value, _, _| callback.borrow_mut().push(value))
        });
        Harness {
            menu,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let trigger = visual.debug_bounds("fusion-button").unwrap();
    visual.simulate_click(trigger.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_some());
    let identity = view
        .update(&mut visual.cx, |view, _, cx| {
            let menu = view.menu.read(cx);
            assert!(menu.is_open());
            assert_eq!(menu.trigger().read(cx).label().as_ref(), "Close commands");
            menu.trigger().entity_id()
        })
        .unwrap();
    press(&mut visual, "down");
    press(&mut visual, "enter");
    assert_eq!(&*actions.borrow(), &[SharedString::from("inspect")]);
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
    view.update(&mut visual.cx, |view, window, cx| {
        let menu = view.menu.read(cx);
        assert_eq!(menu.trigger().entity_id(), identity);
        assert_eq!(menu.trigger().read(cx).label().as_ref(), "Commands");
        assert!(menu.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    press(&mut visual, "down");
    view.update(&mut visual.cx, |view, _, cx| {
        view.theme.update(cx, |theme, cx| {
            theme.set_mode("dark", cx).unwrap();
            theme.set_material_mode(MaterialMode::Glass, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_some());
    press(&mut visual, "escape");
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
    visual.simulate_click(trigger.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(trigger.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
    press(&mut visual, "down");
    let outside = visual.debug_bounds("outside").unwrap();
    visual.simulate_click(outside.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(!view.menu.read(cx).is_open());
        assert!(
            view.outside.is_focused(window),
            "focused {:?}, expected {:?}, trigger {:?}",
            window.focused(cx),
            view.outside,
            view.menu.focus_handle(cx)
        );
        view.menu
            .update(cx, |menu, cx| menu.set_disabled(true, window, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(trigger.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
}

#[gpui::test]
fn long_menu_fits_viewport_and_keyboard_scrolls_to_active_item(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(320.), px(240.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let menu = cx.new(|cx| {
            Dropdown::new(
                "Commands",
                (0..40).map(|i| MenuItem::new(format!("command-{i}"), format!("Command {i}"))),
                window,
                cx,
            )
            .transition_duration(Duration::ZERO)
        });
        Harness {
            menu,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |view, window, cx| {
        window.activate_window();
        view.menu.update(cx, |menu, cx| menu.open(window, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    press(&mut visual, "end");
    visual.update(|window, cx| window.draw(cx).clear());
    let menu = visual.debug_bounds("fusion-dropdown-menu").unwrap();
    let last = visual.debug_bounds("fusion-dropdown-item-39").unwrap();
    assert!(menu.top() >= px(0.) && menu.bottom() <= px(240.));
    assert!(menu.left() >= px(0.) && menu.right() <= px(320.));
    assert!(last.top() >= menu.top() && last.bottom() <= menu.bottom());
    press(&mut visual, "home");
    visual.update(|window, cx| window.draw(cx).clear());
    let first = visual.debug_bounds("fusion-dropdown-item-0").unwrap();
    assert!(first.top() >= menu.top() && first.bottom() <= menu.bottom());
    press(&mut visual, "tab");
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(!view.menu.read(cx).is_open());
        assert!(
            view.outside.is_focused(window),
            "focused {:?}, expected {:?}, trigger {:?}",
            window.focused(cx),
            view.outside,
            view.menu.focus_handle(cx)
        );
    })
    .unwrap();
}

#[gpui::test]
fn morph_menu_replaces_trigger_without_reflow_and_restores_it_on_selection(
    cx: &mut TestAppContext,
) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let callback = actions.clone();
    let view = cx.open_window(size(px(480.), px(300.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "dark").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        theme.update(cx, |theme, cx| {
            theme.set_material_mode(MaterialMode::Glass, cx)
        });
        let menu = cx.new(|cx| {
            Dropdown::new(
                "Commands",
                [MenuItem::new("inspect", "Inspect document")],
                window,
                cx,
            )
            .presentation(DropdownPresentation::Morph)
            .placement(
                DropdownPlacement::new(DropdownSide::Right)
                    .align(DropdownAlign::Start)
                    .gap(px(12.))
                    .offset(gpui::point(px(4.), px(10.))),
            )
            .transition_duration(Duration::ZERO)
            .on_action(move |value, _, _| callback.borrow_mut().push(value))
        });
        Harness {
            menu,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let trigger = visual.debug_bounds("fusion-button").unwrap();
    let outside = visual.debug_bounds("outside").unwrap();
    visual.simulate_click(trigger.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    let menu = visual.debug_bounds("fusion-dropdown-menu").unwrap();
    assert_eq!(menu.left(), trigger.right() + px(16.));
    assert_eq!(menu.top(), trigger.top() + px(10.));
    assert_eq!(visual.debug_bounds("outside").unwrap(), outside);
    let item = visual.debug_bounds("fusion-dropdown-item-0").unwrap();
    visual.simulate_click(item.center(), Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(&*actions.borrow(), &[SharedString::from("inspect")]);
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
    assert_eq!(visual.debug_bounds("fusion-button").unwrap(), trigger);
    view.update(&mut visual.cx, |view, window, cx| {
        assert!(view.menu.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    press(&mut visual, "space");
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_some());
    press(&mut visual, "escape");
    assert!(visual.debug_bounds("fusion-dropdown-menu").is_none());
}
