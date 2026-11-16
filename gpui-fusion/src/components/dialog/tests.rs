use super::*;
use crate::{
    components::input::{Input, InputState},
    default_theme,
    theme::ThemeStore,
};
use gpui::{
    EntityInputHandler, Focusable, Keystroke, TestAppContext, VisualTestContext, WindowHandle,
    point, size,
};
use std::cell::Cell;

struct Harness {
    dialog: Entity<Dialog>,
    input: Entity<InputState>,
    origin: FocusHandle,
    allow_close: Rc<Cell<bool>>,
    closed: Rc<Cell<usize>>,
    _subscription: Subscription,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("origin")
                    .track_focus(&self.origin)
                    .tab_stop(true)
                    .child("Background"),
            )
            .child(self.dialog.clone())
    }
}
fn setup(cx: &mut TestAppContext, animated: bool) -> WindowHandle<Harness> {
    cx.open_window(size(px(640.), px(480.)), |w, c| {
        ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            c,
        )
        .unwrap();
        crate::components::input::init(c);
        let origin = c.focus_handle().tab_stop(true);
        origin.focus(w, c);
        let input = c.new(|c| InputState::new(c).multiline());
        let field = c.new(|c| Input::new(&input, w, c));
        let initial_focus = input.focus_handle(c);
        let allow_close = Rc::new(Cell::new(true));
        let allowed = allow_close.clone();
        let dialog = c.new(|c| {
            Dialog::new("Edit metadata", move |_, _| field.clone(), w, c)
                .initial_focus(initial_focus)
                .animated(animated)
                .on_close_request(move |_, _, _| allowed.get())
        });
        let closed = Rc::new(Cell::new(0));
        let count = closed.clone();
        let subscription = c.subscribe(&dialog, move |_, _, _: &DialogClosed, _| {
            count.set(count.get() + 1)
        });
        Harness {
            dialog,
            input,
            origin,
            allow_close,
            closed,
            _subscription: subscription,
        }
    })
}
fn tick(v: &mut VisualTestContext, millis: u64) {
    v.cx.executor().advance_clock(Duration::from_millis(millis));
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
fn focus_cycle_composition_and_close_policy(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| h.dialog.update(c, |d, c| d.open(w, c)))
        .unwrap();
    tick(&mut v, 20);
    tick(&mut v, 20);
    v.simulate_keystrokes("p enter");
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.input.focus_handle(c).is_focused(w));
            assert_eq!(h.input.read(c).value().as_ref(), "p\n");
            assert!(h.dialog.read(c).is_open());
        })
        .unwrap();
    key(&mut v, "tab");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(
                h.dialog.read(c).close_focus.is_focused(w),
                "focused {:?}; root {:?}; first {:?}; last {:?}; close {:?}; input {:?}",
                w.focused(c),
                h.dialog.read(c).focus,
                h.dialog.read(c).first,
                h.dialog.read(c).last,
                h.dialog.read(c).close_focus,
                h.input.focus_handle(c)
            )
        })
        .unwrap();
    key(&mut v, "shift-tab");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.input.focus_handle(c).is_focused(w));
            h.input.update(c, |i, c| {
                i.replace_and_mark_text_in_range(None, "ko", Some(2..2), w, c)
            });
        })
        .unwrap();
    tick(&mut v, 0);
    key(&mut v, "escape");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.dialog.read(c).is_open());
            assert!(h.input.read(c).is_composing());
            h.input.update(c, |i, c| i.unmark_text(w, c));
            h.allow_close.set(false);
        })
        .unwrap();
    key(&mut v, "escape");
    v.simulate_click(point(px(4.), px(4.)), Default::default());
    handle
        .update(&mut v.cx, |h, _, c| {
            assert!(h.dialog.read(c).is_open());
            assert_eq!(h.closed.get(), 0);
            h.allow_close.set(true);
        })
        .unwrap();
    key(&mut v, "escape");
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(
                !h.dialog.read(c).is_visible(),
                "focused {:?}, root {:?}, composing {}",
                w.focused(c),
                h.dialog.read(c).focus,
                h.input.read(c).is_composing()
            );
            assert!(h.origin.is_focused(w));
            assert_eq!(h.closed.get(), 1);
        })
        .unwrap();
}

#[gpui::test]
fn tall_content_is_bounded_and_empty_tab_cycle_stays_modal(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            h.dialog.update(c, |d, c| {
                d.content =
                    Rc::new(|_, _| div().h(px(1600.)).child("Long content").into_any_element());
                d.close_button = false;
                d.initial_focus = None;
                d.close_on_escape = false;
                d.close_on_backdrop = false;
                d.open(w, c);
            })
        })
        .unwrap();
    tick(&mut v, 0);
    let panel = v.debug_bounds("fusion-dialog-panel").unwrap();
    assert!(
        panel.top() >= px(24.) && panel.bottom() <= px(456.),
        "{panel:?}"
    );
    key(&mut v, "tab");
    key(&mut v, "shift-tab");
    key(&mut v, "escape");
    v.simulate_click(point(px(4.), px(4.)), Default::default());
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.dialog.read(c).is_open());
            assert!(h.dialog.read(c).focus.is_focused(w));
            h.dialog.update(c, |d, c| {
                assert!(d.close(w, c));
            });
            assert!(h.origin.is_focused(w));
        })
        .unwrap();
}

#[gpui::test]
fn interrupted_close_preserves_content_and_continuous_motion(cx: &mut TestAppContext) {
    let handle = setup(cx, true);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| h.dialog.update(c, |d, c| d.open(w, c)))
        .unwrap();
    tick(&mut v, 400);
    tick(&mut v, 20);
    let settled = v.debug_bounds("fusion-dialog-panel").unwrap();
    assert!(settled.left() >= px(24.) && settled.right() <= px(616.));
    handle
        .update(&mut v.cx, |h, w, c| {
            h.dialog.update(c, |d, c| assert!(d.close(w, c)))
        })
        .unwrap();
    tick(&mut v, 120);
    let middle = v.debug_bounds("fusion-dialog-panel").unwrap();
    assert!(middle.top() > settled.top());
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(!h.origin.is_focused(w));
            h.dialog.update(c, |d, c| d.open(w, c));
            ThemeRuntime::global(c).update(c, |t, c| {
                t.set_mode("dark", c).unwrap();
                t.set_material_mode(MaterialMode::Glass, c);
            });
        })
        .unwrap();
    tick(&mut v, 0);
    assert_eq!(
        v.debug_bounds("fusion-dialog-panel").unwrap().origin,
        middle.origin
    );
    tick(&mut v, 500);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.dialog.read(c).is_open());
            assert_eq!(h.closed.get(), 0);
            h.dialog.update(c, |d, c| {
                d.close(w, c);
            });
        })
        .unwrap();
    tick(&mut v, 400);
    assert!(v.debug_bounds("fusion-dialog-panel").is_none());
    handle
        .update(&mut v.cx, |h, w, _| {
            assert_eq!(h.closed.get(), 1);
            assert!(h.origin.is_focused(w));
        })
        .unwrap();
}

#[gpui::test]
fn invalid_styles_do_not_capture_or_strand_focus(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    tick(&mut v, 0);
    let invalid = StylePatch::default().bg(crate::style::Token::<gpui::Hsla>::new(
        "undefined_dialog_surface",
    ));
    handle
        .update(&mut v.cx, |h, w, c| {
            h.dialog.update(c, |d, c| {
                d.set_style(invalid.clone(), c);
                d.open(w, c);
                assert!(!d.is_visible());
                assert!(d.style_error().is_some());
            });
            assert!(h.origin.is_focused(w));
            h.dialog.update(c, |d, c| {
                d.set_style(StylePatch::default(), c);
                d.open(w, c);
            });
        })
        .unwrap();
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, _, c| {
            h.allow_close.set(false);
            h.dialog.update(c, |d, c| d.set_style(invalid, c));
        })
        .unwrap();
    tick(&mut v, 0);
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(!h.dialog.read(c).is_visible());
            assert!(h.dialog.read(c).style_error().is_some());
            assert!(h.origin.is_focused(w));
            assert_eq!(h.closed.get(), 1);
        })
        .unwrap();
}
