use super::{
    ReorderCallback, ReorderState,
    state::{Pointer, finite},
};
use gpui::{
    KeyDownEvent, MouseButton, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, TouchEvent,
    TouchPhase, WeakEntity, Window, px,
};

pub(super) fn register(
    state: WeakEntity<ReorderState>,
    callback: Option<ReorderCallback>,
    window: &mut Window,
) {
    window.on_mouse_event({
        let state = state.clone();
        let callback = callback.clone();
        move |event: &TouchEvent, phase, window, cx| {
            let mut proposal = None;
            let consumed = state
                .update(cx, |state, cx| {
                    if phase.capture() != state.pointer().is_some() || !finite(event.position) {
                        return false;
                    }
                    let pointer = Pointer::Touch(event.id);
                    let active = state.pointer() == Some(pointer);
                    match event.phase {
                        TouchPhase::Started => {
                            if !state.contacts.contains(&event.id) {
                                state.contacts.push(event.id);
                            }
                            if state.contacts.len() > 1 {
                                let owned = state.pointer().is_some();
                                state.cancel(cx);
                                return owned;
                            }
                            if state.enabled
                                && !window.default_prevented()
                                && state.viewport.contains(&event.position)
                            {
                                state.candidate = Some((event.id, event.position));
                            }
                        }
                        TouchPhase::Moved => {
                            if active {
                                return state.move_to(pointer, event.position, cx);
                            }
                            if let Some((id, start)) = state.candidate
                                && id == event.id
                                && (window.default_prevented()
                                    || (event.position.x - start.x)
                                        .abs()
                                        .max((event.position.y - start.y).abs())
                                        > px(8.))
                            {
                                state.candidate = None;
                            }
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.contacts.retain(|id| *id != event.id);
                            if state.candidate.is_some_and(|(id, _)| id == event.id) {
                                state.candidate = None;
                            }
                            if active {
                                if event.phase == TouchPhase::Cancelled {
                                    state.cancel(cx);
                                } else {
                                    state.move_to(pointer, event.position, cx);
                                    proposal = state.end(pointer, cx);
                                }
                                return true;
                            }
                        }
                    }
                    false
                })
                .unwrap_or(false);
            if consumed {
                window.prevent_default();
            }
            if let (Some(event), Some(callback)) = (proposal, &callback) {
                callback(&event, window, cx);
            }
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if !phase.capture() {
                return;
            }
            let consumed = state
                .update(cx, |state, cx| {
                    if state.pointer() != Some(Pointer::Mouse) {
                        return false;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.cancel(cx);
                        return false;
                    }
                    state.move_to(Pointer::Mouse, event.position, cx)
                })
                .unwrap_or(false);
            if consumed {
                window.prevent_default();
                cx.stop_propagation();
            }
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.capture() || event.button != MouseButton::Left {
                return;
            }
            let mut proposal = None;
            let consumed = state
                .update(cx, |state, cx| {
                    if state.pointer() != Some(Pointer::Mouse) {
                        return false;
                    }
                    state.move_to(Pointer::Mouse, event.position, cx);
                    proposal = state.end(Pointer::Mouse, cx);
                    true
                })
                .unwrap_or(false);
            if consumed {
                window.prevent_default();
                cx.stop_propagation();
            }
            if let (Some(event), Some(callback)) = (proposal, &callback) {
                callback(&event, window, cx);
            }
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |_: &ScrollWheelEvent, phase, window, cx| {
            if phase.capture()
                && state
                    .read_with(cx, |state, _| state.pointer().is_some())
                    .unwrap_or(false)
            {
                window.prevent_default();
                cx.stop_propagation();
            }
        }
    });
    window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
        if phase.capture() && event.keystroke.key == "escape" {
            let cancelled = state
                .update(cx, |state, cx| {
                    let active = state.pointer().is_some();
                    if active {
                        state.cancel(cx);
                    }
                    active
                })
                .unwrap_or(false);
            if cancelled {
                window.prevent_default();
                cx.stop_propagation();
            }
        }
    });
}
