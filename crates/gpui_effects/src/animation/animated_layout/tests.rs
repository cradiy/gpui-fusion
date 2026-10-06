use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, ScrollHandle,
    TestAppContext, WindowHandle, canvas, point, size,
};
use std::cell::RefCell;

struct Preview {
    order: Vec<usize>,
    width: Pixels,
    grid: bool,
    parent_x: Pixels,
    scroll: ScrollHandle,
    enabled: bool,
    clicked: Option<usize>,
    bounds: Rc<RefCell<[Option<Bounds<Pixels>>; 4]>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut group = animated_layout("items")
            .w(self.width)
            .gap(px(10.))
            .duration(Duration::from_secs(1))
            .enabled(self.enabled)
            .when(self.grid, |group| group.grid().grid_cols(3))
            .when(!self.grid, |group| group.flex().flex_wrap());
        for &index in &self.order {
            let bounds = self.bounds.clone();
            group = group.item(
                index,
                div()
                    .id("button")
                    .w(px(80.))
                    .h(px(40.))
                    .flex_shrink_0()
                    .on_click(cx.listener(move |this, _, _, _| this.clicked = Some(index)))
                    .child(
                        canvas(
                            move |region, _, _| bounds.borrow_mut()[index] = Some(region),
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    ),
            );
        }
        div().child(
            div()
                .id("scroller")
                .relative()
                .left(self.parent_x)
                .w(px(300.))
                .h(px(80.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(group),
        )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| *view.bounds.borrow_mut() = [None; 4])
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn animated_layout_measures_retargets_and_keeps_input_at_displayed_positions(
    cx: &mut TestAppContext,
) {
    let bounds = Rc::new(RefCell::new([None; 4]));
    let scroll = ScrollHandle::default();
    let handle = cx.open_window(size(px(500.), px(300.)), |_, _| Preview {
        order: vec![0, 1, 2],
        width: px(180.),
        grid: false,
        parent_x: px(0.),
        scroll: scroll.clone(),
        enabled: true,
        clicked: None,
        bounds: bounds.clone(),
    });
    let origin = |index: usize| bounds.borrow()[index].unwrap().origin;
    draw(handle, cx);
    assert_eq!(origin(0), point(px(0.), px(0.)));
    assert_eq!(origin(1), point(px(90.), px(0.)));
    assert_eq!(
        origin(2),
        point(px(0.), px(50.)),
        "normal flex wrapping is preserved"
    );
    handle
        .update(cx, |view, _, _| view.order.swap(0, 1))
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        origin(1),
        point(px(90.), px(0.)),
        "reordering starts at the previous displayed position"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    let halfway = origin(1);
    assert!((halfway.x - px(11.25)).abs() <= px(0.5));
    cx.update_window(handle.into(), |_, window, cx| {
        let position = origin(1) + point(px(30.), px(20.));
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
        }
    })
    .unwrap();
    handle
        .update(cx, |view, _, _| {
            assert_eq!(view.clicked, Some(1));
            view.order.swap(0, 1);
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(origin(1), halfway, "retargeting is continuous");
    handle
        .update(cx, |view, _, _| view.parent_x = px(30.))
        .unwrap();
    scroll.set_offset(point(px(0.), px(-10.)));
    draw(handle, cx);
    assert_eq!(
        origin(1),
        halfway + point(px(30.), px(-10.)),
        "parent movement and scrolling apply immediately"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(origin(1), point(px(120.), px(-10.)));

    handle
        .update(cx, |view, _, _| {
            view.grid = true;
            view.width = px(260.);
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        origin(2),
        point(px(30.), px(50.)),
        "grid reflow animates from the old local position"
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(handle, cx);
    assert_eq!(origin(2), point(px(210.), px(0.)));
    handle
        .update(cx, |view, _, _| view.order = vec![0, 2, 3])
        .unwrap();
    draw(handle, cx);
    assert!(
        bounds.borrow()[1].is_none(),
        "removed items are not retained"
    );
    assert_eq!(
        origin(3),
        point(px(210.), px(0.)),
        "new items start at their measured position"
    );
    assert_eq!(
        origin(2),
        point(px(210.), px(0.)),
        "remaining items move into the gap"
    );
    handle
        .update(cx, |view, _, _| view.enabled = false)
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        origin(2),
        point(px(120.), px(0.)),
        "disabled animation settles immediately"
    );
}
