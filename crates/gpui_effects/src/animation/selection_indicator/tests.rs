use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, TestAppContext,
    WindowHandle, canvas,
};

struct Preview {
    selected: Option<usize>,
    order: [usize; 2],
    wide: bool,
    enabled: bool,
    offset: Pixels,
    underline: bool,
    clicks: usize,
    painted: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let painted = self.painted.clone();
        let mut group = selection_indicator(
            "group",
            div().size_full().child(
                canvas(
                    move |bounds, _, _| painted.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .size_full(),
            ),
        )
        .flex()
        .gap(px(10.))
        .p(px(5.))
        .duration(Duration::from_secs(1))
        .enabled(self.enabled);
        if let Some(selected) = self.selected {
            group = group.selected(("item", selected));
        }
        if self.underline {
            group = group.underline(px(3.)).inset(px(2.));
        }
        for index in self.order {
            group = group.item(
                ("item", index),
                div()
                    .id("button")
                    .w(px(if index == 0 {
                        80.
                    } else if self.wide {
                        180.
                    } else {
                        120.
                    }))
                    .h(px(40.))
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, _, _| {
                        this.clicks += 1;
                    })),
            );
        }
        div().child(div().relative().left(self.offset).p(px(20.)).child(group))
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| view.painted.set(None))
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn selection_indicator_measures_retargets_and_preserves_item_input(cx: &mut TestAppContext) {
    let painted = Rc::new(Cell::new(None));
    let handle = cx.open_window(size(px(600.), px(200.)), |_, _| Preview {
        selected: Some(0),
        order: [0, 1],
        wide: false,
        enabled: true,
        offset: px(0.),
        underline: false,
        clicks: 0,
        painted: painted.clone(),
    });
    draw(handle, cx);
    let first = painted.get().unwrap();
    assert_eq!(
        first,
        Bounds::new(point(px(25.), px(25.)), size(px(80.), px(40.)))
    );
    handle
        .update(cx, |view, _, cx| {
            view.selected = Some(1);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap(),
        first,
        "switching starts at the displayed bounds"
    );
    // The moving decoration must not intercept the destination's click.
    cx.update_window(handle.into(), |_, window, cx| {
        let position = point(px(125.), px(35.));
        for event in [
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                ..Default::default()
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                ..Default::default()
            }),
        ] {
            window.dispatch_event(event, cx);
            window.draw(cx).clear();
        }
    })
    .unwrap();
    handle
        .update(cx, |view, _, _| assert_eq!(view.clicks, 1))
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    let moving = painted.get().unwrap();
    assert!(moving.left() > first.left() && moving.left() < px(115.));
    assert!(moving.size.width > px(80.) && moving.size.width < px(120.));
    handle
        .update(cx, |view, _, cx| {
            view.selected = Some(0);
            view.offset = px(30.);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap().origin,
        moving.origin + point(px(30.), px(0.)),
        "parent movement must apply immediately during reversal"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap().origin,
        first.origin + point(px(30.), px(0.))
    );
    handle
        .update(cx, |view, _, cx| {
            view.selected = Some(1);
            view.order = [1, 0];
            view.wide = true;
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap(),
        Bounds::new(point(px(55.), px(25.)), size(px(180.), px(40.)))
    );
    handle
        .update(cx, |view, _, cx| {
            view.underline = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap(),
        Bounds::new(point(px(57.), px(60.)), size(px(176.), px(3.)))
    );
    handle
        .update(cx, |view, _, cx| {
            view.selected = Some(99);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert!(
        painted.get().is_none(),
        "missing selection must remove the decoration"
    );
    handle
        .update(cx, |view, _, cx| {
            view.selected = Some(0);
            view.enabled = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        painted.get().unwrap(),
        Bounds::new(point(px(247.), px(60.)), size(px(76.), px(3.))),
        "returning selection must snap without a stale animation"
    );
}
