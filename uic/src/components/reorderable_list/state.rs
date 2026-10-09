use gpui::{
    Bounds, Context, ElementId, Pixels, Point, ScrollHandle, Subscription, TouchId, Window, point,
    px,
};
use scheduler::Instant;
use std::{collections::HashMap, time::Duration};

const SETTLE: Duration = Duration::from_millis(180);

/// A proposed move in the current list. Remove `from`, then insert at `to`.
#[derive(Clone, Debug)]
pub struct ReorderEvent {
    pub id: ElementId,
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pointer {
    Mouse,
    Touch(TouchId),
}

struct Drag {
    pointer: Pointer,
    id: ElementId,
    position: Point<Pixels>,
    grab: Pixels,
    to: usize,
}

pub(super) struct Motion {
    from: Pixels,
    target: Pixels,
    started: Instant,
}
impl Motion {
    fn sample(&self, now: Instant) -> (Pixels, bool) {
        let t = (now.saturating_duration_since(self.started).as_secs_f32() / SETTLE.as_secs_f32())
            .min(1.);
        (
            self.target + (self.from - self.target) * (1. - t).powi(3),
            self.from != self.target && t < 1.,
        )
    }
}

/// Retained interaction and scrolling for a vertical reorderable list.
/// The application owns item data and applies accepted reorder events.
pub struct ReorderState {
    pub(super) keys: Vec<ElementId>,
    pub(super) geometry: Vec<Bounds<Pixels>>,
    pub(super) viewport: Bounds<Pixels>,
    pub(super) scroll: ScrollHandle,
    pub(super) enabled: bool,
    drag: Option<Drag>,
    pub(super) contacts: Vec<TouchId>,
    pub(super) candidate: Option<(TouchId, Point<Pixels>)>,
    motions: HashMap<ElementId, Motion>,
    lifted: Option<ElementId>,
    last_frame: Option<Instant>,
    _observations: [Subscription; 2],
}
impl ReorderState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            keys: vec![],
            geometry: vec![],
            viewport: Bounds::default(),
            scroll: ScrollHandle::new(),
            enabled: true,
            drag: None,
            contacts: vec![],
            candidate: None,
            motions: HashMap::new(),
            lifted: None,
            last_frame: None,
            _observations: [
                window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
                cx.observe_window_activation(window, |state, window, cx| {
                    if !window.is_window_active() {
                        state.cancel(cx);
                        state.contacts.clear();
                    }
                }),
            ],
        }
    }
    pub fn scroll_handle(&self) -> ScrollHandle {
        self.scroll.clone()
    }
    pub fn dragged_item(&self) -> Option<&ElementId> {
        self.drag.as_ref().map(|drag| &drag.id)
    }
    /// Cancels the current move without changing application data.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.drag = None;
        self.candidate = None;
        self.last_frame = None;
        cx.notify();
    }
    pub(super) fn configure(
        &mut self,
        keys: Vec<ElementId>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.keys != keys || self.enabled != enabled {
            self.cancel(cx);
            self.motions.retain(|key, _| keys.contains(key));
            self.keys = keys;
            self.geometry.clear();
        }
        self.enabled = enabled;
    }
    pub(super) fn pointer(&self) -> Option<Pointer> {
        self.drag.as_ref().map(|drag| drag.pointer)
    }
    fn content_y(&self, position: Point<Pixels>) -> Pixels {
        position.y - self.viewport.top() - self.scroll.offset().y
    }
    pub(super) fn begin(
        &mut self,
        id: &ElementId,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.enabled || self.drag.is_some() || !finite(position) {
            return false;
        }
        let Some(index) = self.keys.iter().position(|key| key == id) else {
            return false;
        };
        let Some(bounds) = self.geometry.get(index) else {
            return false;
        };
        let y = self
            .motions
            .get(id)
            .map(|m| m.sample(cx.background_executor().now()).0)
            .unwrap_or(bounds.top());
        self.drag = Some(Drag {
            pointer,
            id: id.clone(),
            position,
            grab: self.content_y(position) - y,
            to: index,
        });
        self.lifted = Some(id.clone());
        self.last_frame = None;
        cx.notify();
        true
    }
    pub(super) fn move_to(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !finite(position) {
            return false;
        }
        let Some(drag) = self.drag.as_mut().filter(|drag| drag.pointer == pointer) else {
            return false;
        };
        drag.position = position;
        cx.notify();
        true
    }
    pub(super) fn end(&mut self, pointer: Pointer, cx: &mut Context<Self>) -> Option<ReorderEvent> {
        if self.pointer() != Some(pointer) {
            return None;
        }
        self.update_destination();
        let drag = self.drag.take().unwrap();
        self.candidate = None;
        self.last_frame = None;
        cx.notify();
        let from = self.keys.iter().position(|key| *key == drag.id)?;
        (from != drag.to).then_some(ReorderEvent {
            id: drag.id,
            from,
            to: drag.to,
        })
    }
    fn update_destination(&mut self) {
        let Some(drag) = &self.drag else {
            return;
        };
        let Some(from) = self.keys.iter().position(|key| *key == drag.id) else {
            return;
        };
        if self.geometry.len() != self.keys.len() {
            return;
        }
        let center =
            self.content_y(drag.position) - drag.grab + self.geometry[from].size.height / 2.;
        let mut to = from;
        while to + 1 < self.geometry.len() && center > self.geometry[to + 1].center().y {
            to += 1;
        }
        while to > 0 && center < self.geometry[to - 1].center().y {
            to -= 1;
        }
        self.drag.as_mut().unwrap().to = to;
    }
    pub(super) fn layout(
        &mut self,
        viewport: Bounds<Pixels>,
        geometry: Vec<Bounds<Pixels>>,
        now: Instant,
        animate: bool,
    ) -> (Vec<Pixels>, bool) {
        self.viewport = viewport;
        self.geometry = geometry;
        let mut moving = false;
        if let Some(drag) = &self.drag {
            let edge = (viewport.size.height / 4.).min(px(56.));
            let y = drag.position.y;
            let speed = if edge <= px(0.) {
                0.
            } else if y < viewport.top() + edge {
                -((viewport.top() + edge - y) / edge).clamp(0., 1.)
            } else if y > viewport.bottom() - edge {
                ((y - viewport.bottom() + edge) / edge).clamp(0., 1.)
            } else {
                0.
            };
            let dt = self
                .last_frame
                .map(|last| now.saturating_duration_since(last).as_secs_f32().min(0.05))
                .unwrap_or(0.);
            let old = self.scroll.offset();
            let next = (old.y - px(speed * 600. * dt)).clamp(-self.scroll.max_offset().y, px(0.));
            self.scroll.set_offset(point(old.x, next));
            moving =
                speed < 0. && next < px(0.) || speed > 0. && next > -self.scroll.max_offset().y;
            self.last_frame = Some(now);
        }
        self.update_destination();
        let mut targets: Vec<_> = self.geometry.iter().map(|bounds| bounds.top()).collect();
        if let Some(drag) = &self.drag {
            let from = self.keys.iter().position(|key| *key == drag.id).unwrap();
            let mut order: Vec<_> = (0..self.keys.len()).collect();
            order.remove(from);
            order.insert(drag.to, from);
            let gap = self
                .geometry
                .get(1)
                .map(|b| b.top() - self.geometry[0].bottom())
                .unwrap_or(px(0.));
            let mut top = self.geometry[0].top();
            for i in order {
                targets[i] = top;
                top += self.geometry[i].size.height + gap;
            }
            targets[from] = self.content_y(drag.position) - drag.grab;
        }
        let mut offsets = Vec::with_capacity(targets.len());
        for (index, target) in targets.into_iter().enumerate() {
            let key = &self.keys[index];
            let direct = !animate || self.drag.as_ref().is_some_and(|d| &d.id == key);
            let motion = self.motions.entry(key.clone()).or_insert(Motion {
                from: target,
                target,
                started: now,
            });
            if direct {
                motion.from = target;
                motion.target = target;
            } else if motion.target != target {
                motion.from = motion.sample(now).0;
                motion.target = target;
                motion.started = now;
            }
            let (y, active) = motion.sample(now);
            moving |= active;
            offsets.push(y - self.geometry[index].top());
            if !active && self.drag.is_none() && self.lifted.as_ref() == Some(key) {
                self.lifted = None;
            }
        }
        (offsets, moving)
    }
    pub(super) fn is_lifted(&self, id: &ElementId) -> bool {
        self.lifted.as_ref() == Some(id)
    }
}
pub(super) fn finite(p: Point<Pixels>) -> bool {
    f32::from(p.x).is_finite() && f32::from(p.y).is_finite()
}
