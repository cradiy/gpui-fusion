use super::state::{Pointer, SwipeActionsState};
use gpui::{
    Hitbox, LongPressEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TouchEvent,
    WeakEntity, Window,
};

pub(super) fn register(state: WeakEntity<SwipeActionsState>, hitbox: Hitbox, window: &mut Window) {
    window.on_mouse_event({
        let state = state.clone();
        let hitbox = hitbox.clone();
        move |event: &TouchEvent, phase, window, cx| {
            let _ = state.update(cx, |state, cx| {
                if phase.capture() != state.claimed() {
                    return;
                }
                let inside = hitbox.is_hovered(window) && hitbox.bounds.contains(&event.position);
                if state.touch(event, inside, window.default_prevented(), cx) {
                    window.prevent_default();
                    // Other rows must observe contact endings even when this row owns the drag.
                }
            });
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if event.button != MouseButton::Left {
                return;
            }
            if !phase.bubble()
                || window.default_prevented()
                || !hitbox.is_hovered(window)
                || !hitbox.bounds.contains(&event.position)
            {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                state.begin(Pointer::Mouse, event.position, cx)
            });
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if !phase.capture() {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    state.end(Pointer::Mouse, true, cx);
                } else if state.move_to(Pointer::Mouse, event.position, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
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
                if state.claimed() {
                    state.move_to(Pointer::Mouse, event.position, cx);
                }
                if state.end(Pointer::Mouse, false, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            });
        }
    });
    window.on_mouse_event(move |_: &LongPressEvent, phase, _, cx| {
        if phase.capture() {
            let _ = state.update(cx, |state, cx| {
                state.cancel(cx);
            });
        }
    });
}
