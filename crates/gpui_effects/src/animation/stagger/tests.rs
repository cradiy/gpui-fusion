use super::*;
use gpui::{AppContext, Context, Render, TestAppContext, WindowHandle, canvas, px, size};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Preview {
    visible: bool,
    enabled: bool,
    order: [usize; 3],
    frames: Rc<RefCell<HashMap<usize, PresenceFrame>>>,
    bottom: Rc<Cell<Pixels>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut group = staggered_presence("group", self.visible)
            .duration(Duration::from_secs(1))
            .interval(Duration::from_millis(100))
            .enabled(self.enabled)
            .flex()
            .flex_col()
            .gap(px(8.));
        for id in self.order {
            let frames = self.frames.clone();
            group = group.item(("item", id), move |frame| {
                frames.borrow_mut().insert(id, frame);
                div().w(px(100.)).h(px(40.)).opacity(frame.progress)
            });
        }
        let bottom = self.bottom.clone();
        div().flex().flex_col().child(group).child(
            canvas(
                move |bounds, _, _| bottom.set(bounds.top()),
                |_, _, _, _| {},
            )
            .size(px(10.)),
        )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| view.frames.borrow_mut().clear())
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn stagger_preserves_order_reversals_and_layout_slots(cx: &mut TestAppContext) {
    let frames = Rc::new(RefCell::new(HashMap::new()));
    let bottom = Rc::new(Cell::new(px(0.)));
    let handle = cx.open_window(size(px(400.), px(300.)), |_, _| Preview {
        visible: false,
        enabled: true,
        order: [0, 1, 2],
        frames: frames.clone(),
        bottom: bottom.clone(),
    });
    draw(handle, cx);
    assert!(frames.borrow().is_empty());
    handle
        .update(cx, |view, _, cx| {
            view.visible = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_millis(150));
    draw(handle, cx);
    let entering: Vec<_> = (0..3).map(|i| frames.borrow()[&i].progress).collect();
    assert!(entering[0] > entering[1] && entering[1] > entering[2]);
    assert_eq!(entering[2], 0., "last item must still be waiting");
    assert_eq!(bottom.get(), px(136.));
    handle
        .update(cx, |view, _, cx| {
            view.visible = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        (0..3)
            .map(|i| frames.borrow()[&i].progress)
            .collect::<Vec<_>>(),
        entering
    );
    cx.executor().advance_clock(Duration::from_millis(150));
    draw(handle, cx);
    assert_eq!(
        frames.borrow()[&0].progress,
        entering[0],
        "reverse exit must delay first item"
    );
    assert_eq!(frames.borrow()[&1].progress, 0.);
    assert_eq!(
        bottom.get(),
        px(136.),
        "completed items must retain layout slots until group exit"
    );
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(handle, cx);
    assert!(frames.borrow().is_empty());
    assert_eq!(bottom.get(), px(0.));
    handle
        .update(cx, |view, _, cx| {
            view.visible = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    cx.executor().advance_clock(Duration::from_millis(150));
    draw(handle, cx);
    let before: Vec<_> = (0..3).map(|i| frames.borrow()[&i].progress).collect();
    handle
        .update(cx, |view, _, cx| {
            view.order = [2, 1, 0];
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        (0..3)
            .map(|i| frames.borrow()[&i].progress)
            .collect::<Vec<_>>(),
        before,
        "reordering stable keys must not restart motion"
    );
    handle
        .update(cx, |view, _, cx| {
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert!(
        frames
            .borrow()
            .values()
            .all(|frame| frame.progress == 1. && frame.phase == PresencePhase::Visible)
    );
    handle
        .update(cx, |view, _, cx| {
            view.visible = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert!(frames.borrow().is_empty());
}
