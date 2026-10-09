use super::state::{Pointer, ZoomState};
use gpui::{
    Hitbox, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent,
    ScrollWheelEvent, TouchEvent, TouchPhase, WeakEntity, Window, px,
};

pub(super) fn register(
    state: WeakEntity<ZoomState>,
    hitbox: Hitbox,
    wheel_zoom: bool,
    window: &mut Window,
) {
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
                }
            });
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        let hitbox = hitbox.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if phase.bubble()
                && event.button == MouseButton::Left
                && !window.default_prevented()
                && hitbox.is_hovered(window)
            {
                let _ = state.update(cx, |state, _| state.begin(Pointer::Mouse, event.position));
            }
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            let _ = state.update(cx, |state, cx| {
                if phase.capture() != state.claimed() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) || window.default_prevented() {
                    state.end(Pointer::Mouse);
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
                if state.end(Pointer::Mouse) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            });
        }
    });
    window.on_mouse_event({
        let state = state.clone();
        let hitbox = hitbox.clone();
        move |event: &PinchEvent, phase, window, cx| {
            if !phase.bubble() || window.default_prevented() || !hitbox.is_hovered(window) {
                return;
            }
            let _ = state.update(cx, |state, cx| {
                if !state.supported {
                    return;
                }
                if matches!(event.phase, TouchPhase::Started | TouchPhase::Moved) {
                    state.zoom_at(state.zoom() * (1. + event.delta), event.position, cx);
                }
                window.prevent_default();
                cx.stop_propagation();
            });
        }
    });
    window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
        if !wheel_zoom
            || !phase.bubble()
            || window.default_prevented()
            || !hitbox.should_handle_scroll(window)
        {
            return;
        }
        let _ = state.update(cx, |state, cx| {
            if !state.supported {
                return;
            }
            let delta = f32::from(event.delta.pixel_delta(px(24.)).y);
            if delta.is_finite() && delta != 0. {
                state.zoom_at(
                    state.zoom() * (delta * 0.002).clamp(-2., 2.).exp(),
                    event.position,
                    cx,
                );
                window.prevent_default();
                cx.stop_propagation();
            }
        });
    });
}
