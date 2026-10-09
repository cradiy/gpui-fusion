use super::*;
use gpui::{
    Hitbox, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, TouchEvent, TouchId, TouchPhase,
    WeakEntity,
};

#[derive(Default)]
pub(super) struct TouchState {
    contacts: Vec<TouchId>,
    pub(super) pending: Option<TouchDrag>,
}

#[derive(Clone, Copy)]
pub(super) struct TouchDrag {
    id: TouchId,
    start: Point<Pixels>,
    claimed: bool,
}

pub(super) fn register(entity: WeakEntity<Flip>, hitbox: Hitbox, window: &mut Window) {
    window.on_touch_event(move |event, phase, window, cx| {
        let _ = entity.update(cx, |this, cx| {
            let claimed = this.touch.pending.is_some_and(|drag| drag.claimed);
            if phase.capture() != claimed {
                return;
            }
            let inside = hitbox.is_hovered(window) && hitbox.bounds.contains(&event.position);
            if this.touch_event(event, inside, window, cx) {
                window.prevent_default();
            }
        });
    });
}

impl Flip {
    fn begin_drag(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(edge) = self.edge_at(f32::from(position.x)) else {
            return false;
        };
        let direction = self.direction_for_edge(edge);
        self.anticipated_position = self.destination_position(direction);
        self.emit_preload_request(FlipPreloadReason::Triggered(direction), cx);
        self.load_slot_range(self.flip_range(direction), window, cx);
        if !self.flip_ready(direction) {
            cx.notify();
            return false;
        }
        self.active_edge = edge;
        self.configure_sequence_edge(direction, edge);
        let (progress, pointer_y) = self.normalized_pointer(position.x.into(), position.y.into());
        self.progress = progress.max(0.015);
        self.pointer_y = pointer_y;
        self.velocity = 0.;
        self.target = None;
        self.dragging = true;
        cx.notify();
        true
    }

    fn move_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (progress, pointer_y) = self.normalized_pointer(position.x.into(), position.y.into());
        self.velocity = progress - self.progress;
        self.progress = progress;
        self.pointer_y = pointer_y;
        cx.notify();
    }

    fn end_drag(&mut self, cancelled: bool, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        let complete =
            !cancelled && self.progress + self.velocity * 5. >= self.completion_threshold;
        if cancelled {
            self.velocity = 0.;
        }
        self.target = Some(if complete { 1. } else { 0. });
        self.last_frame = Instant::now();
        cx.notify();
    }

    fn touch_event(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.phase {
            TouchPhase::Started => {
                if self.touch.contacts.contains(&event.id) {
                    return false;
                }
                self.touch.contacts.push(event.id);
                if self.touch.contacts.len() > 1 {
                    let claimed = self.touch.pending.take().is_some_and(|drag| drag.claimed);
                    if claimed {
                        self.end_drag(true, cx);
                    }
                    return claimed;
                }
                if inside
                    && !window.default_prevented()
                    && !self.is_animating()
                    && self.edge_at(event.position.x.into()).is_some()
                {
                    self.touch.pending = Some(TouchDrag {
                        id: event.id,
                        start: event.position,
                        claimed: false,
                    });
                }
                false
            }
            TouchPhase::Moved => {
                let Some(mut drag) = self.touch.pending.filter(|drag| drag.id == event.id) else {
                    return false;
                };
                if !drag.claimed {
                    if window.default_prevented() {
                        self.touch.pending = None;
                        return false;
                    }
                    let delta = event.position - drag.start;
                    let dx = f32::from(delta.x);
                    let dy = f32::from(delta.y).abs();
                    if dx.abs().max(dy) <= 8. {
                        return false;
                    }
                    let inward = match self.edge_at(drag.start.x.into()) {
                        Some(FlipEdge::Left) => dx > 0.,
                        Some(FlipEdge::Right) => dx < 0.,
                        None => false,
                    };
                    if dx.abs() <= dy * 1.2 || !inward || !self.begin_drag(drag.start, window, cx) {
                        self.touch.pending = None;
                        return false;
                    }
                    drag.claimed = true;
                    self.touch.pending = Some(drag);
                }
                self.move_drag(event.position, cx);
                true
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.touch.contacts.retain(|id| *id != event.id);
                let Some(drag) = self.touch.pending.filter(|drag| drag.id == event.id) else {
                    return false;
                };
                self.touch.pending = None;
                if drag.claimed {
                    if event.phase == TouchPhase::Ended
                        && self
                            .normalized_pointer(event.position.x.into(), event.position.y.into())
                            .0
                            != self.progress
                    {
                        self.move_drag(event.position, cx);
                    }
                    self.end_drag(event.phase == TouchPhase::Cancelled, cx);
                }
                drag.claimed
            }
        }
    }

    pub(super) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.touch.contacts.is_empty() {
            self.begin_drag(event.position, window, cx);
        }
    }

    pub(super) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dragging && self.touch.pending.is_none() {
            self.move_drag(event.position, cx);
        }
    }

    pub(super) fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.touch.pending.is_none() {
            self.end_drag(false, cx);
        }
    }
}

#[cfg(test)]
mod tests;
