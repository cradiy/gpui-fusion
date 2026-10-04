use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, TestAppContext,
    WindowHandle, canvas, point, px,
};
use std::{cell::Cell, rc::Rc};

struct Preview {
    targets: [Bounds<Pixels>; 2],
    bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    clicks: [usize; 2],
    order: [usize; 2],
    enabled: bool,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().children(self.order.map(|i| {
            let bounds = self.bounds[i].clone();
            layout_transition(("item", i), self.targets[i])
                .duration(Duration::from_secs(1))
                .enabled(self.enabled)
                .on_click(cx.listener(move |this, _, _, _| {
                    this.clicks[i] += 1;
                }))
                .child(canvas(move |region, _, _| bounds.set(region), |_, _, _, _| {}).size_full())
        }))
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

#[gpui::test]
fn retargeting_keeps_visible_geometry_and_clicks_in_sync(cx: &mut TestAppContext) {
    let bounds = std::array::from_fn(|_| Rc::new(Cell::new(Bounds::default())));
    let handle = cx.open_window(size(px(800.), px(600.)), |_, _| Preview {
        targets: [rect(20., 30., 100., 80.), rect(600., 30., 100., 80.)],
        bounds: bounds.clone(),
        clicks: [0; 2],
        order: [0, 1],
        enabled: true,
    });
    draw(handle, cx);
    assert_eq!(bounds[0].get(), rect(20., 30., 100., 80.));
    handle
        .update(cx, |view, _, cx| {
            view.targets[0] = rect(300., 180., 200., 140.);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bounds[0].get(),
        rect(20., 30., 100., 80.),
        "new target must start at current geometry"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    let displayed = bounds[0].get();
    assert!(displayed.origin.x > px(200.) && displayed.origin.x < px(300.));
    assert!(displayed.size.width > px(150.) && displayed.size.width < px(200.));
    cx.update_window(handle.into(), |_, window, cx| {
        let position = displayed.origin + point(px(4.), px(4.));
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
        .update(cx, |view, _, cx| {
            assert_eq!(view.clicks, [1, 0]);
            view.targets[0] = rect(60., 300., 80., 160.);
            view.order = [1, 0];
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bounds[0].get(),
        displayed,
        "retargeting and reordering must preserve the displayed start"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(bounds[0].get(), rect(60., 300., 80., 160.));
    handle
        .update(cx, |view, _, cx| {
            view.enabled = false;
            view.targets[0] = rect(100., 100., 160., 120.);
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(bounds[0].get(), rect(100., 100., 160., 120.));
    handle
        .update(cx, |view, _, cx| {
            view.enabled = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        bounds[0].get(),
        rect(100., 100., 160., 120.),
        "enabling must not replay disabled motion"
    );
}

#[test]
fn motion_finishes_and_zero_duration_snaps() {
    let now = Instant::now();
    let mut state = LayoutState {
        from: rect(0., 0., 20., 20.),
        target: rect(100., 30., 80., 60.),
        started: now,
        duration: Duration::from_secs(1),
    };
    assert!(!state.sample(now + Duration::from_secs(1)).1);
    assert!(
        !state
            .update(rect(0., 0., 0., 0.), Duration::ZERO, true, now)
            .1
    );
    assert_eq!(state.sample(now).0, rect(0., 0., 0., 0.));
}

struct TextPreview {
    width: Pixels,
    marker: Rc<Cell<Bounds<Pixels>>>,
}

impl Render for TextPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let marker = self.marker.clone();
        div().relative().size_full().child(
            layout_transition("text", Bounds::new(point(px(20.), px(20.)), size(self.width, px(400.))))
                .duration(Duration::from_secs(1)).flex().flex_col()
                .text_size(px(16.)).line_height(px(24.))
                .child(div().w_full().flex_shrink_0().child("A paragraph should wrap as its panel becomes narrower, while keeping its text at the same font size."))
                .child(canvas(move |bounds, _, _| marker.set(bounds), |_, _, _, _| {}).w_full().h(px(1.)).flex_shrink_0()),
        )
    }
}

#[gpui::test]
fn resizing_reflows_text_instead_of_scaling_a_snapshot(cx: &mut TestAppContext) {
    let marker = Rc::new(Cell::new(Bounds::default()));
    let handle = cx.open_window(size(px(800.), px(600.)), |_, _| TextPreview {
        width: px(500.),
        marker: marker.clone(),
    });
    let draw = |cx: &mut TestAppContext| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap()
    };
    draw(cx);
    let wide = marker.get();
    handle
        .update(cx, |view, _, cx| {
            view.width = px(140.);
            cx.notify();
        })
        .unwrap();
    draw(cx);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx);
    let intermediate = marker.get();
    assert!(intermediate.size.width > px(140.) && intermediate.size.width < wide.size.width);
    assert!(
        intermediate.origin.y > wide.origin.y,
        "text must wrap while width changes"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx);
    assert_eq!(marker.get().size.width, px(140.));
    assert!(marker.get().origin.y >= intermediate.origin.y);
}
