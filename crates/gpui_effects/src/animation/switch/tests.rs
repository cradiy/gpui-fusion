use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, TestAppContext,
    WindowHandle, point, prelude::*, px, size,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct Preview {
    value: Arc<String>,
    enabled: bool,
    builds: Rc<RefCell<Vec<String>>>,
    clicks: usize,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let builds = self.builds.clone();
        let entity = cx.entity().downgrade();
        animated_switch("switch", self.value.clone(), move |value| {
            builds.borrow_mut().push(value.as_str().to_owned());
            let entity = entity.clone();
            div()
                .id("button")
                .size_full()
                .child(value.as_str().to_owned())
                .on_click(move |_, _, cx| {
                    entity.update(cx, |view, _| view.clicks += 1).unwrap();
                })
        })
        .w(px(200.))
        .h(px(60.))
        .duration(Duration::from_secs(1))
        .enabled(self.enabled)
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    handle
        .update(cx, |view, _, _| view.builds.borrow_mut().clear())
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn animated_switch_coalesces_reverses_and_releases_outgoing_values(cx: &mut TestAppContext) {
    let now = cx.background_executor.now();
    let duration = Duration::from_secs(1);
    let mut state = SwitchState::new(0, duration, now);
    state.update(&1, duration, true, now);
    let middle = now + Duration::from_millis(400);
    let progress = state.update(&1, duration, true, middle).0;
    assert_eq!(
        state.update(&0, duration, true, middle).0,
        progress,
        "reversal must preserve visible progress"
    );
    state.update(&0, duration, true, now + Duration::from_secs(1));
    assert!(state.to.is_none());
    assert_eq!(state.from_slot, 0);
    state.update(&1, duration, true, now + Duration::from_secs(1));
    let incoming_slot = state.to_slot;
    state.update(&2, duration, true, now + Duration::from_secs(2));
    assert_eq!(
        state.from_slot, incoming_slot,
        "incoming identity must survive completion"
    );
    assert_ne!(
        state.to_slot, 0,
        "new content must not inherit an old child's identity"
    );

    let builds = Rc::new(RefCell::new(Vec::new()));
    let first = Arc::new("A".to_owned());
    let weak_first = Arc::downgrade(&first);
    let handle = cx.open_window(size(px(300.), px(150.)), |_, _| Preview {
        value: first,
        enabled: true,
        builds: builds.clone(),
        clicks: 0,
    });
    draw(handle, cx);
    assert_eq!(&*builds.borrow(), &["A"]);
    handle
        .update(cx, |view, _, cx| {
            view.value = Arc::new("B".to_owned());
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(&*builds.borrow(), &["A", "B"]);
    cx.update_window(handle.into(), |_, window, cx| {
        for event in [
            PlatformInput::MouseDown(MouseDownEvent {
                position: point(px(20.), px(20.)),
                ..Default::default()
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                position: point(px(20.), px(20.)),
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
            assert_eq!(
                view.clicks, 0,
                "content must not be clickable during a switch"
            );
            view.value = Arc::new("C".to_owned());
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        &*builds.borrow(),
        &["A", "B"],
        "third value must not add another visible subtree"
    );
    handle
        .update(cx, |view, _, cx| {
            view.value = Arc::new("D".to_owned());
            cx.notify();
        })
        .unwrap();
    cx.executor().advance_clock(duration);
    draw(handle, cx);
    assert_eq!(
        &*builds.borrow(),
        &["B", "D"],
        "only the latest pending value should enter"
    );
    assert!(
        weak_first.upgrade().is_none(),
        "outgoing value must be released at completion"
    );
    cx.executor().advance_clock(duration);
    draw(handle, cx);
    assert_eq!(&*builds.borrow(), &["D"]);
    handle
        .update(cx, |view, _, cx| {
            view.value = Arc::new("E".to_owned());
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        &*builds.borrow(),
        &["E"],
        "disabled motion must snap to latest content"
    );
}
