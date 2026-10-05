use super::*;
use gpui::{
    AppContext, Context, ParentElement, Render, TestAppContext, WindowHandle, div, px, size,
};
use std::{cell::Cell, rc::Rc};

struct Preview {
    target: f64,
    enabled: bool,
    displayed: Rc<Cell<f64>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let displayed = self.displayed.clone();
        animated_number("value", self.target, move |value| {
            displayed.set(value);
            div().child(format!("{value:.1}"))
        })
        .duration(Duration::from_secs(1))
        .enabled(self.enabled)
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

fn set_target(handle: WindowHandle<Preview>, target: f64, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, cx| {
            view.target = target;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
}

#[gpui::test]
fn animated_number_retargets_without_jumps_and_recovers_from_nonfinite_values(
    cx: &mut TestAppContext,
) {
    let displayed = Rc::new(Cell::new(0.));
    let handle = cx.open_window(size(px(300.), px(150.)), |_, _| Preview {
        target: 10.,
        enabled: true,
        displayed: displayed.clone(),
    });
    draw(handle, cx);
    assert_eq!(
        displayed.get(),
        10.,
        "first render must not count up from zero"
    );
    set_target(handle, 110., cx);
    assert_eq!(displayed.get(), 10.);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    assert_eq!(displayed.get(), 60.);
    set_target(handle, -40., cx);
    assert_eq!(
        displayed.get(),
        60.,
        "retargeting must preserve the current value"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    assert_eq!(displayed.get(), 10.);
    // An unrelated redraw must not restart an unchanged target.
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    assert_eq!(displayed.get(), -40.);
    set_target(handle, 20., cx);
    handle
        .update(cx, |view, _, cx| {
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(displayed.get(), 20.);
    handle
        .update(cx, |view, _, cx| {
            view.enabled = true;
            cx.notify();
        })
        .unwrap();
    set_target(handle, f64::NAN, cx);
    assert!(displayed.get().is_nan());
    set_target(handle, -f64::MAX, cx);
    assert_eq!(displayed.get(), -f64::MAX);
    set_target(handle, f64::MAX, cx);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    assert_eq!(
        displayed.get(),
        0.,
        "opposite finite extremes must not overflow"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(displayed.get(), f64::MAX);
}
