use super::state::{Pointer, SplitPaneState};
use gpui::{
    Focusable, Hitbox, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TouchPhase,
    WeakEntity, Window,
};

pub(super) fn register(state: WeakEntity<SplitPaneState>, hitbox: Hitbox, window: &mut Window) {
    window.on_mouse_event({
        let state = state.clone();
        let hitbox = hitbox.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || !hitbox.is_hovered(window)
                || window.default_prevented()
            {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                state.focus_handle(cx).focus(window, cx);
                if event.click_count == 2 {
                    state.reset(cx);
                } else if state.begin(Pointer::Mouse, event.position) {
                    window.capture_pointer(hitbox.id);
                    cx.notify();
                }
            });
            window.prevent_default();
            cx.stop_propagation();
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if !phase.capture() {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                if !state.owns(Pointer::Mouse) {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left)
                    || window.captured_hitbox() != state.capture
                {
                    state.cancel(cx);
                    return;
                }
                state.move_to(Pointer::Mouse, event.position, cx);
                window.prevent_default();
                cx.stop_propagation();
            });
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.capture() || event.button != MouseButton::Left {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                if !state.owns(Pointer::Mouse) {
                    return;
                }
                state.move_to(Pointer::Mouse, event.position, cx);
                state.end(Pointer::Mouse, cx);
                window.release_pointer();
                window.prevent_default();
                cx.stop_propagation();
            });
        }
    });
    window.on_touch_event(move |event, phase, window, cx| {
        let pointer = Pointer::Touch(event.id);
        let _ = state.update(cx, |state, cx| {
            if event.phase == TouchPhase::Started {
                if !phase.bubble()
                    || window.default_prevented()
                    || !hitbox.is_hovered(window)
                    || !hitbox.bounds.contains(&event.position)
                    || !state.begin(pointer, event.position)
                {
                    return;
                }
                state.focus_handle(cx).focus(window, cx);
                cx.notify();
            } else {
                if !phase.capture() || !state.owns(pointer) {
                    return;
                }
                match event.phase {
                    TouchPhase::Moved => state.move_to(pointer, event.position, cx),
                    TouchPhase::Ended => {
                        state.move_to(pointer, event.position, cx);
                        state.end(pointer, cx);
                    }
                    TouchPhase::Cancelled => state.cancel(cx),
                    TouchPhase::Started => unreachable!(),
                }
            }
            window.prevent_default();
            cx.stop_propagation();
        });
    });
}
