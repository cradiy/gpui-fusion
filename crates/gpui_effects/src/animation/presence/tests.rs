use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, TestAppContext,
    WindowHandle, canvas, point, prelude::*, px, size,
};
use std::{cell::Cell, rc::Rc};

struct Preview {
    visible: bool,
    enabled: bool,
    clicks: usize,
    frame: Rc<Cell<Option<PresenceFrame>>>,
    trailing_y: Rc<Cell<Pixels>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = self.frame.clone();
        let trailing_y = self.trailing_y.clone();
        let child = div()
            .id("button")
            .w(px(100.))
            .h(px(60.))
            .on_click(cx.listener(|this, _, _, _| this.clicks += 1));
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                animated_presence("presence", self.visible, move |value| {
                    frame.set(Some(value));
                    child.opacity(value.progress)
                })
                .duration(Duration::from_secs(1))
                .enabled(self.enabled),
            )
            .child(
                canvas(
                    move |bounds, _, _| trailing_y.set(bounds.top()),
                    |_, _, _, _| {},
                )
                .size(px(20.)),
            )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| {
            view.frame.set(None);
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

fn click(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| {
        let position = point(px(10.), px(10.));
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                ..Default::default()
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                ..Default::default()
            }),
            cx,
        );
    })
    .unwrap();
}

#[gpui::test]
fn presence_reverses_without_jumps_and_removes_hidden_children(cx: &mut TestAppContext) {
    let frame = Rc::new(Cell::new(None));
    let trailing_y = Rc::new(Cell::new(px(0.)));
    let handle = cx.open_window(size(px(400.), px(300.)), |_, _| Preview {
        visible: true,
        enabled: true,
        clicks: 0,
        frame: frame.clone(),
        trailing_y: trailing_y.clone(),
    });
    draw(handle, cx);
    assert_eq!(frame.get().unwrap().phase, PresencePhase::Visible);
    click(handle, cx);
    handle
        .update(cx, |view, _, cx| {
            assert_eq!(view.clicks, 1);
            view.visible = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_millis(400));
    draw(handle, cx);
    let exiting = frame.get().unwrap();
    assert_eq!(exiting.phase, PresencePhase::Exiting);
    assert!(exiting.progress > 0. && exiting.progress < 1.);
    click(handle, cx);
    handle
        .update(cx, |view, _, cx| {
            assert_eq!(view.clicks, 1);
            view.visible = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(frame.get().unwrap().progress, exiting.progress);
    assert_eq!(frame.get().unwrap().phase, PresencePhase::Entering);
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(frame.get().unwrap().progress, 1.);
    handle
        .update(cx, |view, _, cx| {
            view.visible = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert!(frame.get().is_none(), "hidden builder must not run");
    assert_eq!(
        trailing_y.get(),
        px(0.),
        "hidden child must leave neither height nor flex gap"
    );
    click(handle, cx);
    handle
        .update(cx, |view, _, cx| {
            assert_eq!(view.clicks, 1);
            view.visible = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        frame.get().unwrap().progress,
        0.,
        "stable hidden ID must preserve entry state"
    );
    handle
        .update(cx, |view, _, cx| {
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(frame.get().unwrap().progress, 1.);
    handle
        .update(cx, |view, _, cx| {
            view.visible = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert!(frame.get().is_none());
}
