use super::*;
use crate::{
    components::input::{Input, InputState},
    default_theme,
    theme::ThemeStore,
};
use gpui::{
    EntityInputHandler, Keystroke, Point, TestAppContext, VisualTestContext, WindowHandle, point,
    size,
};
use std::cell::{Cell, RefCell};

struct Harness {
    popover: Entity<Popover>,
    input: Entity<InputState>,
    outside: Entity<Button>,
    clicks: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<PopoverEvent>>>,
    position: Point<Pixels>,
    _subscription: Subscription,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(px(12.))
                    .child(self.outside.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(self.position.x)
                    .top(self.position.y)
                    .child(self.popover.clone()),
            )
    }
}
fn setup(cx: &mut TestAppContext, animated: bool) -> WindowHandle<Harness> {
    cx.open_window(size(px(680.), px(520.)), |w, c| {
        ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            c,
        )
        .unwrap();
        let input = c.new(|c| InputState::new(c).multiline());
        let field = c.new(|c| Input::new(&input, w, c));
        let focus = input.focus_handle(c);
        let popover = c.new(|c| {
            Popover::new("Options", move |_, _| field.clone(), w, c)
                .title("Properties")
                .initial_focus(focus)
                .animated(animated)
        });
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let outside = c.new(|c| {
            Button::new("Background action", w, c).on_click(move |_, _| count.set(count.get() + 1))
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let records = events.clone();
        let subscription = c.subscribe(&popover, move |_, _, e: &PopoverEvent, _| {
            records.borrow_mut().push(*e)
        });
        Harness {
            popover,
            input,
            outside,
            clicks,
            events,
            position: point(px(420.), px(420.)),
            _subscription: subscription,
        }
    })
}
fn tick(v: &mut VisualTestContext, ms: u64) {
    v.cx.executor().advance_clock(Duration::from_millis(ms));
    v.run_until_parked();
    v.update(|w, c| w.draw(c).clear());
    v.run_until_parked();
}
fn key(v: &mut VisualTestContext, key: &str) {
    v.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    tick(v, 0);
}
#[gpui::test]
fn trigger_composition_and_outside_click_keep_input_routing(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    let trigger = v.debug_bounds("fusion-popover-anchor").unwrap();
    v.simulate_click(trigger.center(), Default::default());
    tick(&mut v, 0);
    v.simulate_keystrokes("a enter");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.popover.read(c).is_open());
            assert!(h.input.focus_handle(c).is_focused(w));
            assert_eq!(h.input.read(c).value().as_ref(), "a\n");
            h.input.update(c, |i, c| {
                i.replace_and_mark_text_in_range(None, "mi", Some(2..2), w, c)
            });
        })
        .unwrap();
    tick(&mut v, 0);
    key(&mut v, "escape");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.popover.read(c).is_open());
            h.input.update(c, |i, c| i.unmark_text(w, c));
        })
        .unwrap();
    key(&mut v, "escape");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(!h.popover.read(c).is_visible());
            assert!(h.popover.focus_handle(c).is_focused(w));
            h.popover.update(c, |p, c| p.open(w, c));
        })
        .unwrap();
    tick(&mut v, 0);
    v.simulate_click(point(px(50.), px(30.)), Default::default());
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, _, c| {
            assert_eq!(h.clicks.get(), 1);
            assert!(!h.popover.read(c).is_visible());
            assert!(
                h.events
                    .borrow()
                    .contains(&PopoverEvent::Closed(PopoverCloseReason::OutsideClick))
            );
        })
        .unwrap();
}
#[gpui::test]
fn anchor_tracking_collision_and_clipped_anchor_dismissal(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            h.popover.update(c, |p, c| p.open(w, c))
        })
        .unwrap();
    tick(&mut v, 0);
    let anchor = v.debug_bounds("fusion-popover-anchor").unwrap();
    let panel = v.debug_bounds("fusion-popover-panel").unwrap();
    assert!(panel.bottom() < anchor.top());
    assert!(panel.right() <= px(668.) && panel.left() >= px(12.));
    handle
        .update(&mut v.cx, |h, _, c| {
            h.position = point(px(80.), px(90.));
            c.notify();
        })
        .unwrap();
    tick(&mut v, 0);
    let anchor = v.debug_bounds("fusion-popover-anchor").unwrap();
    let moved = v.debug_bounds("fusion-popover-panel").unwrap();
    assert!(moved.top() > anchor.bottom());
    assert_ne!(panel.origin, moved.origin);
    handle
        .update(&mut v.cx, |h, _, c| {
            h.popover.update(c, |p, c| {
                p.content = Rc::new(|_, _| {
                    div()
                        .h(px(1800.))
                        .child("Scrollable content")
                        .into_any_element()
                });
                c.notify();
            });
        })
        .unwrap();
    tick(&mut v, 0);
    let tall = v.debug_bounds("fusion-popover-panel").unwrap();
    assert!(
        tall.top() >= px(12.) && tall.bottom() <= px(508.),
        "{tall:?}"
    );
    handle
        .update(&mut v.cx, |h, _, c| {
            h.position = point(px(900.), px(900.));
            c.notify();
        })
        .unwrap();
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(!h.popover.read(c).is_visible());
            assert!(!h.input.focus_handle(c).is_focused(w));
            assert_eq!(
                h.events.borrow().last(),
                Some(&PopoverEvent::Closed(PopoverCloseReason::AnchorLost))
            );
        })
        .unwrap();
}
#[gpui::test]
fn reversal_and_theme_changes_preserve_panel_state(cx: &mut TestAppContext) {
    let handle = setup(cx, true);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            h.popover.update(c, |p, c| p.open(w, c))
        })
        .unwrap();
    tick(&mut v, 350);
    let settled = v.debug_bounds("fusion-popover-panel").unwrap();
    handle
        .update(&mut v.cx, |h, w, c| {
            h.input.update(c, |i, c| i.set_value("Pinned", c));
            h.popover.update(c, |p, c| p.close(w, c));
        })
        .unwrap();
    tick(&mut v, 100);
    let exiting = v.debug_bounds("fusion-popover-panel").unwrap();
    assert!(exiting.top() > settled.top());
    handle
        .update(&mut v.cx, |h, w, c| {
            h.popover.update(c, |p, c| p.open(w, c));
            ThemeRuntime::global(c).update(c, |t, c| {
                t.set_mode("dark", c).unwrap();
                t.set_material_mode(MaterialMode::Glass, c);
            });
        })
        .unwrap();
    tick(&mut v, 0);
    assert_eq!(
        v.debug_bounds("fusion-popover-panel").unwrap().origin,
        exiting.origin
    );
    tick(&mut v, 350);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert_eq!(h.input.read(c).value().as_ref(), "Pinned");
            assert!(h.events.borrow().iter().all(|e| *e == PopoverEvent::Opened));
            h.popover.update(c, |p, c| p.set_disabled(true, w, c));
            assert!(!h.popover.read(c).is_visible());
            h.popover.update(c, |p, c| p.open(w, c));
            assert!(!h.popover.read(c).is_visible());
            assert!(!h.input.focus_handle(c).is_focused(w));
        })
        .unwrap();
}

#[gpui::test]
fn child_select_outside_panel_remains_interactive(cx: &mut TestAppContext) {
    use crate::components::select::{Select, SelectOption};
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    let select = handle
        .update(&mut v.cx, |h, w, c| {
            let select = c.new(|c| {
                Select::new(
                    "Scale",
                    (1..=6).map(|n| SelectOption::new(n.to_string(), format!("Scale {n}"))),
                    w,
                    c,
                )
                .unwrap()
                .transition_duration(Duration::ZERO)
            });
            h.position = point(px(80.), px(70.));
            let content = select.clone();
            h.popover.update(c, |p, c| {
                p.content = Rc::new(move |_, _| content.clone().into_any_element());
                p.initial_focus = None;
                c.notify();
            });
            c.notify();
            select
        })
        .unwrap();
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            h.popover.update(c, |p, c| p.open(w, c))
        })
        .unwrap();
    tick(&mut v, 0);
    let trigger = v.debug_bounds("fusion-select-trigger").unwrap();
    v.simulate_click(trigger.center(), Default::default());
    tick(&mut v, 0);
    let option = v.debug_bounds("fusion-select-option-5").unwrap();
    let panel = v.debug_bounds("fusion-popover-panel").unwrap();
    assert!(!panel.contains(&option.center()));
    v.simulate_click(option.center(), Default::default());
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, _, c| {
            assert!(h.popover.read(c).is_open());
            assert_eq!(select.read(c).selected().map(|s| s.as_ref()), Some("6"));
            assert!(!select.read(c).is_open());
        })
        .unwrap();
}
