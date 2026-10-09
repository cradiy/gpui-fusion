use super::*;
use crate::components::pager::{Pager, PagerState};
use gpui::{
    Context, MouseButton, PlatformInput, Render, TestAppContext, TouchEvent, TouchId, TouchPhase,
    VisualTestContext, px,
};

struct Example {
    pager: Entity<PagerState>,
    zoom: Entity<ZoomState>,
    width: f32,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom.clone();
        Pager::new("pages", &self.pager, move |index, _, _| {
            ZoomView::new(
                ("zoom", index),
                &zoom,
                div().size_full().bg(gpui::rgb(0x456789)),
            )
        })
        .w(px(self.width))
        .h(px(300.))
    }
}
fn setup(cx: &mut TestAppContext) -> gpui::WindowHandle<Example> {
    let view = cx.open_window(size(px(600.), px(400.)), |window, cx| Example {
        pager: cx.new(|cx| PagerState::new(3, window, cx)),
        zoom: cx.new(|cx| ZoomState::new(size(px(800.), px(600.)), window, cx)),
        width: 400.,
    });
    cx.set_subtree_effects_supported(view.into(), true);
    view
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn touch(cx: &mut VisualTestContext, id: u64, phase: TouchPhase, x: f32, y: f32) {
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Touch(TouchEvent {
                id: TouchId(id),
                phase,
                position: point(px(x), px(y)),
                force: None,
            }),
            cx,
        );
    });
}

#[gpui::test]
fn anchored_zoom_clamps_pan_and_refits_after_resize(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    draw(&mut cx);
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Pinch(gpui::PinchEvent {
                position: point(px(300.), px(150.)),
                delta: 1.,
                phase: TouchPhase::Moved,
                ..Default::default()
            }),
            cx,
        );
    });
    view.update(&mut cx.cx, |this, _, cx| {
        let state = this.zoom.read(cx);
        assert_eq!(state.zoom(), 2.);
        assert_eq!(state.offset(), point(px(-100.), px(0.)));
    })
    .unwrap();
    draw(&mut cx);
    cx.simulate_mouse_down(
        point(px(300.), px(150.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(px(900.), px(800.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(px(900.), px(800.)),
        MouseButton::Left,
        Default::default(),
    );
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.zoom.read(cx).offset(), point(px(200.), px(150.)));
        assert_eq!(this.pager.read(cx).current_page(), Some(0));
        this.width = 200.;
        cx.notify();
    })
    .unwrap();
    draw(&mut cx);
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.zoom.read(cx).offset(), point(px(100.), px(0.)));
        this.zoom.update(cx, |state, cx| state.reset(cx));
        assert_eq!(this.zoom.read(cx).zoom(), 1.);
        assert_eq!(this.zoom.read(cx).offset(), point(px(0.), px(0.)));
    })
    .unwrap();
}

#[gpui::test]
fn touch_pinch_and_pan_take_priority_over_pager_until_reset(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    draw(&mut cx);
    touch(&mut cx, 1, TouchPhase::Started, 150., 150.);
    touch(&mut cx, 2, TouchPhase::Started, 250., 150.);
    touch(&mut cx, 1, TouchPhase::Moved, 100., 150.);
    touch(&mut cx, 2, TouchPhase::Moved, 300., 150.);
    touch(&mut cx, 1, TouchPhase::Ended, 100., 150.);
    touch(&mut cx, 2, TouchPhase::Ended, 300., 150.);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.zoom.read(cx).zoom(), 2.);
        assert_eq!(this.zoom.read(cx).offset(), point(px(0.), px(0.)));
        assert_eq!(this.pager.read(cx).current_page(), Some(0));
    })
    .unwrap();
    draw(&mut cx);
    touch(&mut cx, 3, TouchPhase::Started, 300., 150.);
    touch(&mut cx, 3, TouchPhase::Moved, 120., 150.);
    touch(&mut cx, 4, TouchPhase::Started, 220., 150.);
    touch(&mut cx, 4, TouchPhase::Moved, 320., 150.);
    touch(&mut cx, 4, TouchPhase::Cancelled, 320., 150.);
    touch(&mut cx, 3, TouchPhase::Ended, 120., 150.);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.zoom.read(cx).zoom(), 4.);
        assert_eq!(this.pager.read(cx).current_page(), Some(0));
        this.zoom.update(cx, |state, cx| state.reset(cx));
    })
    .unwrap();
    draw(&mut cx);
    touch(&mut cx, 5, TouchPhase::Started, 300., 150.);
    touch(&mut cx, 5, TouchPhase::Moved, 100., 150.);
    touch(&mut cx, 5, TouchPhase::Ended, 100., 150.);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(this.pager.read(cx).current_page(), Some(1))
    })
    .unwrap();
}
