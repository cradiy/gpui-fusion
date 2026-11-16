use super::*;
use crate::{default_theme, theme::ThemeStore};
use gpui::{AppContext, KeyDownEvent, Keystroke, TestAppContext, VisualTestContext, size};

struct Harness {
    host: Entity<ToastHost>,
    theme: Entity<ThemeRuntime>,
    focus: FocusHandle,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("origin")
                    .track_focus(&self.focus)
                    .tab_stop(true)
                    .child("Content"),
            )
            .child(self.host.clone())
    }
}
fn setup(cx: &mut TestAppContext, animated: bool) -> gpui::WindowHandle<Harness> {
    cx.open_window(size(px(600.), px(480.)), |w, c| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            c,
        )
        .unwrap();
        let focus = c.focus_handle();
        focus.focus(w, c);
        let host = c.new(|c| ToastHost::new(w, c).max_visible(1).animated(animated));
        Harness { host, theme, focus }
    })
}
fn tick(v: &mut VisualTestContext, millis: u64) {
    v.cx.executor().advance_clock(Duration::from_millis(millis));
    v.run_until_parked();
    v.update(|w, c| w.draw(c).clear());
}
#[gpui::test]
fn queued_lifetimes_hover_and_status_update(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    v.update(|w, c| {
        w.activate_window();
        w.draw(c).clear();
    });
    let (first, second) = handle
        .update(&mut v.cx, |h, w, c| {
            h.host.update(c, |host, c| {
                (
                    host.show(
                        Toast::new("Index complete").timeout(Some(Duration::from_secs(1))),
                        w,
                        c,
                    ),
                    host.show(
                        Toast::new("Fetching metadata").kind(ToastKind::Loading),
                        w,
                        c,
                    ),
                )
            })
        })
        .unwrap();
    tick(&mut v, 100);
    let bounds = v.debug_bounds("fusion-toast-1").unwrap();
    assert!(v.debug_bounds("fusion-toast-2").is_none());
    v.simulate_mouse_move(bounds.center(), None, Default::default());
    tick(&mut v, 2000);
    assert!(v.debug_bounds("fusion-toast-1").is_some());
    v.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, Default::default());
    tick(&mut v, 850);
    assert!(v.debug_bounds("fusion-toast-1").is_some());
    tick(&mut v, 60);
    assert!(v.debug_bounds("fusion-toast-1").is_none());
    assert!(v.debug_bounds("fusion-toast-2").is_some());
    tick(&mut v, 6000);
    handle
        .update(&mut v.cx, |h, w, c| {
            h.theme.update(c, |t, c| {
                t.set_mode("dark", c).unwrap();
                t.set_material_mode(MaterialMode::Glass, c);
            });
            h.host.update(c, |host, c| {
                assert!(host.contains(second));
                assert!(
                    host.update_toast(
                        second,
                        Toast::new("Metadata ready")
                            .kind(ToastKind::Success)
                            .timeout(Some(Duration::from_millis(500))),
                        w,
                        c
                    )
                );
                assert!(!host.update_toast(first, Toast::new("Already removed"), w, c));
            });
        })
        .unwrap();
    tick(&mut v, 200);
    assert!(v.debug_bounds("fusion-toast-2").is_some());
    v.deactivate_window();
    tick(&mut v, 1200);
    assert!(v.debug_bounds("fusion-toast-2").is_some());
    v.update(|w, _| w.activate_window());
    tick(&mut v, 350);
    handle
        .update(&mut v.cx, |h, _, c| {
            let host = h.host.read(c);
            assert!(!host.contains(second));
            assert!(host.timer.is_none());
        })
        .unwrap();
}
#[gpui::test]
fn keyboard_pause_restore_focus_and_reverse_exit(cx: &mut TestAppContext) {
    let handle = setup(cx, true);
    let mut v = VisualTestContext::from_window(handle.into(), cx);
    v.update(|w, c| {
        w.activate_window();
        w.draw(c).clear();
    });
    let id = handle
        .update(&mut v.cx, |h, w, c| {
            h.host.update(c, |host, c| {
                host.show(
                    Toast::new("Transfer finished").timeout(Some(Duration::from_secs(1))),
                    w,
                    c,
                )
            })
        })
        .unwrap();
    tick(&mut v, 400);
    let settled = v.debug_bounds("fusion-toast-1").unwrap();
    handle
        .update(&mut v.cx, |h, w, c| {
            let focus = h.host.read(c).entries[0].focus.clone();
            focus.focus(w, c);
        })
        .unwrap();
    tick(&mut v, 2000);
    assert!(v.debug_bounds("fusion-toast-1").is_some());
    v.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("escape").unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    tick(&mut v, 140);
    let exiting = v.debug_bounds("fusion-toast-1").unwrap();
    assert!(
        exiting.top() > settled.top(),
        "settled: {settled:?}, exiting: {exiting:?}"
    );
    handle
        .update(&mut v.cx, |h, w, c| {
            assert!(h.focus.is_focused(w));
            h.host.update(c, |host, c| {
                assert!(host.update_toast(id, Toast::new("Additional result").timeout(None), w, c))
            });
        })
        .unwrap();
    tick(&mut v, 0);
    assert_eq!(
        v.debug_bounds("fusion-toast-1").unwrap().origin,
        exiting.origin
    );
    tick(&mut v, 400);
    assert_eq!(
        v.debug_bounds("fusion-toast-1").unwrap().origin,
        settled.origin
    );
    handle
        .update(&mut v.cx, |h, w, c| {
            h.host.update(c, |host, c| host.clear(w, c))
        })
        .unwrap();
    tick(&mut v, 400);
    assert!(v.debug_bounds("fusion-toast-1").is_none());
}
