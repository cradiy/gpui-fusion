use super::*;
use gpui::{Image, ImageFormat, PlatformInput, TestAppContext, point, size};
use std::sync::Arc;

#[gpui::test]
fn touch_flip_claims_horizontal_drags_and_cancels_without_turning(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(400.), px(200.)), |_, _| {
        let source = Arc::new(Image::from_bytes(ImageFormat::Svg,
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#345"/></svg>"##.to_vec()));
        Flip::new(source.clone(), source.clone(), source.clone(), source)
    });
    let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear());
    let touch = |id, phase, x, y, cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            window.dispatch_event(
                PlatformInput::Touch(TouchEvent {
                    id: TouchId(id),
                    phase,
                    position: point(px(x), px(y)),
                    force: None,
                }),
                cx,
            )
        })
    };
    // Vertical movement gives ownership to scrolling, even if it later turns horizontal.
    touch(1, TouchPhase::Started, 380., 80., &mut cx);
    assert!(!touch(1, TouchPhase::Moved, 378., 110., &mut cx).default_prevented);
    assert!(!touch(1, TouchPhase::Moved, 60., 110., &mut cx).default_prevented);
    touch(1, TouchPhase::Ended, 60., 110., &mut cx);
    window
        .update(&mut cx.cx, |book, _, _| assert!(!book.is_animating()))
        .unwrap();

    // A drag starting in the middle does not take over scrolling.
    touch(2, TouchPhase::Started, 200., 80., &mut cx);
    assert!(!touch(2, TouchPhase::Moved, 20., 80., &mut cx).default_prevented);
    touch(2, TouchPhase::Ended, 20., 80., &mut cx);

    // Ownership survives leaving the view. Cancellation must never complete a turn.
    touch(3, TouchPhase::Started, 380., 80., &mut cx);
    assert!(touch(3, TouchPhase::Moved, -20., 80., &mut cx).default_prevented);
    assert!(touch(3, TouchPhase::Cancelled, -20., 80., &mut cx).default_prevented);
    window
        .update(&mut cx.cx, |book, _, _| {
            assert!(!book.dragging);
            assert_eq!(book.target, Some(0.));
            book.reset_interaction();
        })
        .unwrap();

    // Adding a second finger cancels the turn and cannot restart it mid-contact.
    touch(4, TouchPhase::Started, 380., 80., &mut cx);
    assert!(touch(4, TouchPhase::Moved, 200., 80., &mut cx).default_prevented);
    assert!(touch(5, TouchPhase::Started, 80., 80., &mut cx).default_prevented);
    touch(5, TouchPhase::Ended, 80., 80., &mut cx);
    assert!(!touch(4, TouchPhase::Moved, 20., 80., &mut cx).default_prevented);
    touch(4, TouchPhase::Ended, 20., 80., &mut cx);
    window
        .update(&mut cx.cx, |book, _, _| {
            assert_eq!(book.target, Some(0.));
            book.reset_interaction();
        })
        .unwrap();

    touch(6, TouchPhase::Started, 380., 80., &mut cx);
    assert!(touch(6, TouchPhase::Moved, 60., 80., &mut cx).default_prevented);
    window
        .update(&mut cx.cx, |book, _, _| {
            assert!(book.dragging);
            assert_eq!(book.target, None);
        })
        .unwrap();
    assert!(touch(6, TouchPhase::Ended, 60., 80., &mut cx).default_prevented);
    window
        .update(&mut cx.cx, |book, _, _| {
            assert!(!book.dragging);
            assert_eq!(book.target, Some(1.));
            book.reset_interaction();
        })
        .unwrap();

    cx.simulate_mouse_down(
        point(px(380.), px(80.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(px(60.), px(80.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(px(60.), px(80.)),
        MouseButton::Left,
        Default::default(),
    );
    window
        .update(&mut cx.cx, |book, _, _| assert_eq!(book.target, Some(1.)))
        .unwrap();
}
