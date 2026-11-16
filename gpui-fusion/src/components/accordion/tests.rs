use super::*;
use crate::{default_theme, theme::ThemeStore};
use gpui::{Keystroke, TestAppContext, VisualTestContext, size};
use std::cell::RefCell;

struct Body {
    value: usize,
    focus: FocusHandle,
}
impl Render for Body {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("body-field")
            .debug_selector(|| format!("body-value-{}", self.value))
            .h(px(80.))
            .track_focus(&self.focus)
            .tab_stop(true)
            .child(self.value.to_string())
    }
}
struct Harness {
    accordion: Entity<Accordion>,
    theme: Entity<ThemeRuntime>,
    body: Entity<Body>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().child(self.accordion.clone())
    }
}
fn setup(
    cx: &mut gpui::App,
    window: &mut Window,
    mode: AccordionMode,
    duration: Duration,
    on_change: Change,
) -> Harness {
    let theme = ThemeRuntime::install(
        ThemeStore::new(default_theme(), "light").unwrap(),
        "neutral",
        cx,
    )
    .unwrap();
    let body = cx.new(|cx| Body {
        value: 11,
        focus: cx.focus_handle(),
    });
    let other = cx.new(|cx| Body {
        value: 23,
        focus: cx.focus_handle(),
    });
    let unavailable = cx.new(|cx| Body {
        value: 37,
        focus: cx.focus_handle(),
    });
    let accordion = cx.new(|cx| {
        Accordion::new(
            "Details",
            [
                AccordionItem::new("first", "First", body.clone()),
                AccordionItem::new("locked", "Locked", unavailable).disabled(true),
                AccordionItem::new("last", "Last", other),
            ],
            window,
            cx,
        )
        .unwrap()
        .mode(mode)
        .transition_duration(duration)
        .on_change(move |v, w, c| on_change(v, w, c))
    });
    Harness {
        accordion,
        theme,
        body,
    }
}
fn press(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.update(|window, cx| window.draw(cx).clear());
}
#[gpui::test]
fn exclusive_selection_validates_atomically_restores_focus_and_retains_content(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let received = events.clone();
    let view = cx.open_window(size(px(500.), px(600.)), |w, c| {
        setup(
            c,
            w,
            AccordionMode::Single,
            Duration::ZERO,
            Rc::new(move |v, _, _| received.borrow_mut().push(v)),
        )
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    assert!(visual.debug_bounds("body-value-11").is_none());
    view.update(&mut visual.cx, |h, w, c| {
        h.accordion.read(c).focus_handle(c).focus(w, c);
    })
    .unwrap();
    press(&mut visual, "space");
    assert!(visual.debug_bounds("body-value-11").is_some());
    view.update(&mut visual.cx, |h, w, c| {
        h.body.read(c).focus.clone().focus(w, c);
        h.accordion.update(c, |a, c| {
            assert!(a.set_expanded(["first", "last"], w, c).is_err());
            assert!(a.set_expanded(["locked"], w, c).is_err());
            assert_eq!(a.expanded(), [SharedString::from("first")]);
            a.set_expanded(std::iter::empty::<&str>(), w, c).unwrap();
            assert!(a.items[0].focus.is_focused(w));
        });
        h.body.update(c, |body, c| {
            body.value = 19;
            c.notify();
        });
        h.theme.update(c, |theme, c| {
            theme.set_mode("dark", c).unwrap();
            theme.set_material_mode(MaterialMode::Glass, c);
        });
    })
    .unwrap();
    press(&mut visual, "down");
    view.update(&mut visual.cx, |h, w, c| {
        assert!(h.accordion.read(c).items[2].focus.is_focused(w))
    })
    .unwrap();
    press(&mut visual, "enter");
    assert!(visual.debug_bounds("body-value-23").is_some());
    press(&mut visual, "home");
    press(&mut visual, "enter");
    assert!(visual.debug_bounds("body-value-19").is_some());
    assert!(visual.debug_bounds("body-value-23").is_none());
    assert_eq!(
        &*events.borrow(),
        &[
            vec![SharedString::from("first")],
            vec![SharedString::from("last")],
            vec![SharedString::from("first")]
        ]
    );
}
#[gpui::test]
fn multiple_sections_animate_height_and_reverse_from_current_bounds(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(500.), px(600.)), |w, c| {
        setup(
            c,
            w,
            AccordionMode::Multiple,
            Duration::from_millis(400),
            Rc::new(|_, _, _| {}),
        )
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|w, c| w.draw(c).clear());
    let closed = visual
        .debug_bounds("fusion-accordion-header-1")
        .unwrap()
        .top();
    view.update(&mut visual.cx, |h, w, c| {
        h.accordion
            .update(c, |a, c| a.set_expanded(["first", "last"], w, c).unwrap())
    })
    .unwrap();
    visual.update(|w, c| w.draw(c).clear());
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(150));
    visual.update(|w, c| w.draw(c).clear());
    let middle = visual
        .debug_bounds("fusion-accordion-header-1")
        .unwrap()
        .top();
    assert!(middle > closed && middle < closed + px(112.));
    view.update(&mut visual.cx, |h, w, c| {
        h.accordion
            .update(c, |a, c| a.set_expanded(["last"], w, c).unwrap())
    })
    .unwrap();
    visual.update(|w, c| w.draw(c).clear());
    assert_eq!(
        visual
            .debug_bounds("fusion-accordion-header-1")
            .unwrap()
            .top(),
        middle
    );
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(500));
    visual.update(|w, c| w.draw(c).clear());
    assert_eq!(
        visual
            .debug_bounds("fusion-accordion-header-1")
            .unwrap()
            .top(),
        closed
    );
    assert!(visual.debug_bounds("body-value-11").is_none());
    assert!(visual.debug_bounds("body-value-23").is_some());
}
