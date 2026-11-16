use super::*;
use crate::{components::button::Button, default_theme, theme::ThemeStore};
use gpui::{AppContext, TestAppContext, VisualTestContext, size};
use std::{cell::Cell, rc::Rc};

struct Harness {
    badge: Entity<Badge>,
    theme: Entity<ThemeRuntime>,
    button: Entity<Button>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_8().flex().items_start().child(Badged::new(
            div()
                .debug_selector(|| "host".into())
                .child(self.button.clone()),
            self.badge.clone(),
        ))
    }
}
#[gpui::test]
fn attached_count_preserves_host_layout_and_pointer_activation(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let received = clicks.clone();
    let view = cx.open_window(size(px(340.), px(200.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let badge = cx.new(|cx| {
            Badge::new("Pending reviews", BadgeContent::Count(0), cx).count_limit(Some(50))
        });
        let button = cx.new(|cx| {
            Button::new("Review", window, cx)
                .material_mode(MaterialMode::Plain)
                .on_click(move |_, _| received.set(received.get() + 1))
        });
        Harness {
            badge,
            button,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|w, c| {
        w.activate_window();
        w.draw(c).clear();
    });
    let host = visual.debug_bounds("host").unwrap();
    assert!(visual.debug_bounds("fusion-badge").is_none());
    view.update(&mut visual.cx, |h, _, c| {
        h.badge.update(c, |b, c| {
            b.set_content(BadgeContent::Count(51), c);
            assert_eq!(b.displayed_text().as_deref(), Some("50+"));
        })
    })
    .unwrap();
    visual.update(|w, c| w.draw(c).clear());
    assert_eq!(visual.debug_bounds("host").unwrap(), host);
    let marker = visual.debug_bounds("fusion-badge").unwrap();
    let overlap = host.intersect(&marker);
    assert!(overlap.size.width > px(0.) && overlap.size.height > px(0.));
    visual.simulate_click(overlap.center(), Default::default());
    assert_eq!(
        clicks.get(),
        1,
        "the badge must not intercept its host's clicks"
    );
    view.update(&mut visual.cx, |h, _, c| {
        h.theme.update(c, |t, c| {
            t.set_mode("dark", c).unwrap();
            t.set_material_mode(MaterialMode::Glass, c);
        });
    })
    .unwrap();
    visual.run_until_parked();
    visual.update(|w, c| w.draw(c).clear());
    view.update(&mut visual.cx, |h, _, c| {
        h.badge.update(c, |b, c| {
            assert_eq!(b.content(), &BadgeContent::Count(51));
            assert!(b.resolve(c).unwrap().root.material.is_some());
            b.set_material_mode(Some(MaterialMode::Plain), c);
            assert!(b.resolve(c).unwrap().root.material.is_none());
            b.set_material_mode(None, c);
            assert!(b.resolve(c).unwrap().root.material.is_some());
            b.set_content(BadgeContent::Count(0), c);
        })
    })
    .unwrap();
    visual.update(|w, c| w.draw(c).clear());
    assert!(visual.debug_bounds("fusion-badge").is_none());
    assert_eq!(visual.debug_bounds("host").unwrap(), host);
    view.update(&mut visual.cx, |h, _, c| {
        h.badge.update(c, |b, c| {
            b.set_show_zero(true, c);
            assert_eq!(b.displayed_text().as_deref(), Some("0"));
        })
    })
    .unwrap();
    visual.update(|w, c| w.draw(c).clear());
    assert!(visual.debug_bounds("fusion-badge").is_some());
}

fn frame(visual: &mut VisualTestContext, millis: u64) {
    visual
        .cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(millis));
    visual.update(|w, c| w.draw(c).clear());
}

#[gpui::test]
fn drag_capture_rebound_interruption_dismissal_and_reset(cx: &mut TestAppContext) {
    let dismissals = Rc::new(Cell::new(0));
    let host_clicks = Rc::new(Cell::new(0));
    let received = dismissals.clone();
    let clicks = host_clicks.clone();
    let view = cx.open_window(size(px(420.), px(300.)), |window, cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        let badge = cx.new(|cx| {
            Badge::new("Active jobs", BadgeContent::Count(12), cx)
                .dismissible(true)
                .on_dismiss(move |_, _| received.set(received.get() + 1))
        });
        let button = cx.new(|cx| {
            Button::new("Queue", window, cx)
                .material_mode(MaterialMode::Plain)
                .on_click(move |_, _| clicks.set(clicks.get() + 1))
        });
        Harness {
            badge,
            button,
            theme,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|w, c| {
        w.activate_window();
        w.draw(c).clear();
    });
    let original = visual.debug_bounds("fusion-badge").unwrap();
    let start = original.center();
    let nearby = start + point(px(24.), px(14.));
    visual.simulate_mouse_down(start, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 200);
    visual.simulate_mouse_move(nearby, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 200);
    assert_eq!(
        visual.debug_bounds("fusion-badge").unwrap().center(),
        nearby
    );
    visual.simulate_mouse_up(nearby, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 100);
    let returning = visual.debug_bounds("fusion-badge").unwrap().center();
    assert!(returning.x > start.x && returning.x < nearby.x);
    visual.simulate_mouse_down(returning, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 0);
    assert_eq!(
        visual.debug_bounds("fusion-badge").unwrap().center(),
        returning
    );
    // Losing capture cancels the gesture even if the pointer is far from the anchor.
    visual.update(|w, _| w.release_pointer());
    visual.simulate_mouse_up(
        start + point(px(150.), px(0.)),
        gpui::MouseButton::Left,
        Default::default(),
    );
    frame(&mut visual, 500);
    frame(&mut visual, 16);
    assert_eq!(visual.debug_bounds("fusion-badge").unwrap(), original);
    assert_eq!(dismissals.get(), 0);
    assert_eq!(host_clicks.get(), 0);
    // Change material during a captured drag and release outside the badge's original bounds.
    visual.simulate_mouse_down(start, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 200);
    let far = start + point(px(110.), px(55.));
    visual.simulate_mouse_move(far, gpui::MouseButton::Left, Default::default());
    view.update(&mut visual.cx, |h, _, c| {
        h.theme
            .update(c, |t, c| t.set_material_mode(MaterialMode::Glass, c))
    })
    .unwrap();
    visual.run_until_parked();
    frame(&mut visual, 200);
    assert_eq!(visual.debug_bounds("fusion-badge").unwrap().center(), far);
    visual.simulate_mouse_up(far, gpui::MouseButton::Left, Default::default());
    frame(&mut visual, 300);
    frame(&mut visual, 16);
    assert_eq!(dismissals.get(), 1);
    assert!(visual.debug_bounds("fusion-badge").is_none());
    frame(&mut visual, 500);
    assert_eq!(dismissals.get(), 1);
    view.update(&mut visual.cx, |h, w, c| {
        h.badge.update(c, |b, c| {
            assert!(b.is_dismissed());
            b.reset(w, c);
            assert_eq!(b.content(), &BadgeContent::Count(12));
        })
    })
    .unwrap();
    frame(&mut visual, 0);
    assert_eq!(visual.debug_bounds("fusion-badge").unwrap(), original);
    assert_eq!(host_clicks.get(), 0);
}
