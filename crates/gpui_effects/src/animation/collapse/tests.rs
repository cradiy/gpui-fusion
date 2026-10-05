use super::*;
use gpui::{AppContext, Context, Render, TestAppContext, WindowHandle, canvas};
use std::{cell::Cell, rc::Rc};

struct Preview {
    expanded: bool,
    enabled: bool,
    width: Pixels,
    count: usize,
    builds: Rc<Cell<usize>>,
    bottom: Rc<Cell<Pixels>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let builds = self.builds.clone();
        let bottom = self.bottom.clone();
        let count = self.count;
        div()
            .w(self.width)
            .flex()
            .flex_col()
            .child(
                animated_collapse("collapse", self.expanded, move || {
                    builds.set(builds.get() + 1);
                    div()
                        .w_full()
                        .flex()
                        .flex_wrap()
                        .children((0..count).map(|_| div().w(px(80.)).h(px(40.)).flex_shrink_0()))
                })
                .duration(Duration::from_secs(1))
                .enabled(self.enabled),
            )
            .child(
                canvas(
                    move |bounds, _, _| bottom.set(bounds.top()),
                    |_, _, _, _| {},
                )
                .size(px(10.)),
            )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn collapse_tracks_intrinsic_height_reversal_and_width_changes(cx: &mut TestAppContext) {
    let builds = Rc::new(Cell::new(0));
    let bottom = Rc::new(Cell::new(px(0.)));
    let handle = cx.open_window(size(px(500.), px(500.)), |_, _| Preview {
        expanded: true,
        enabled: true,
        width: px(240.),
        count: 6,
        builds: builds.clone(),
        bottom: bottom.clone(),
    });
    draw(handle, cx);
    assert_eq!(
        bottom.get(),
        px(80.),
        "initial open layout must use natural height"
    );
    handle
        .update(cx, |view, _, cx| {
            view.expanded = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_millis(400));
    draw(handle, cx);
    let partial = bottom.get();
    assert!(partial > px(0.) && partial < px(80.));
    handle
        .update(cx, |view, _, cx| {
            view.expanded = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bottom.get(),
        partial,
        "reversal must preserve displayed height"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(bottom.get(), px(80.));
    handle
        .update(cx, |view, _, cx| {
            view.width = px(120.);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bottom.get(),
        px(80.),
        "new measured target must not snap layout"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    assert!(bottom.get() > px(80.) && bottom.get() < px(240.));
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(
        bottom.get(),
        px(240.),
        "content must reflow at actual parent width"
    );
    handle
        .update(cx, |view, _, cx| {
            view.count = 2;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(bottom.get(), px(240.));
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(bottom.get(), px(80.));
    handle
        .update(cx, |view, _, cx| {
            view.expanded = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    let previous_builds = builds.get();
    draw(handle, cx);
    assert_eq!(bottom.get(), px(0.));
    assert_eq!(
        builds.get(),
        previous_builds,
        "closed content must not be constructed"
    );
    handle
        .update(cx, |view, _, cx| {
            view.expanded = true;
            view.enabled = false;
            view.count = 3;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bottom.get(),
        px(120.),
        "disabled animation uses current natural layout"
    );
    handle
        .update(cx, |view, _, cx| {
            view.expanded = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(bottom.get(), px(0.));
}
