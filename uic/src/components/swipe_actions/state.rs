use gpui::{
    Context, EventEmitter, Pixels, Point, Subscription, TouchEvent, TouchId, TouchPhase, Window, px,
};
use scheduler::Instant;
use std::time::Duration;

const SLOP: f32 = 8.;
const SETTLE: Duration = Duration::from_millis(180);
const EXIT: Duration = Duration::from_millis(210);

/// The direction in which the user moves the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
}

/// Emitted once on release when an enabled swipe reaches its distance threshold.
pub struct SwipeTriggered {
    pub direction: SwipeDirection,
}

/// Visual feedback for the current displacement, including the return animation.
#[derive(Clone, Copy, Debug)]
pub struct SwipeProgress {
    pub direction: SwipeDirection,
    /// Signed horizontal displacement in logical pixels, without a distance cap.
    pub displacement: Pixels,
    /// Distance as a fraction of the trigger threshold, clamped to 0..=1.
    pub progress: f32,
    /// Whether releasing the current gesture would trigger its action.
    pub ready: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pointer {
    Mouse,
    Touch(TouchId),
}

struct Drag {
    pointer: Pointer,
    start: Point<Pixels>,
    offset: f32,
    claimed: bool,
    rejected: bool,
}

struct Dismissal {
    direction: SwipeDirection,
    started: Instant,
    from: f32,
}

/// Retain one state per row, using the row's stable identity across list updates.
pub struct SwipeActionsState {
    enabled: [bool; 2],
    threshold: f32,
    drag: Option<Drag>,
    motion: Option<(Instant, f32)>,
    dismissal: Option<Dismissal>,
    width: Pixels,
    contacts: Vec<TouchId>,
    _observation: Subscription,
}

impl EventEmitter<SwipeTriggered> for SwipeActionsState {}

impl SwipeActionsState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            enabled: [true; 2],
            threshold: 96.,
            drag: None,
            motion: None,
            dismissal: None,
            width: px(0.),
            contacts: Vec::new(),
            _observation: window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        }
    }

    pub(super) fn visual(&self, now: Instant) -> (Pixels, bool) {
        if let Some(exit) = &self.dismissal {
            let distance = f32::from(self.width).max(exit.from.abs());
            let target = if exit.direction == SwipeDirection::Left {
                -distance
            } else {
                distance
            };
            let t = (now.duration_since(exit.started).as_secs_f32() / EXIT.as_secs_f32()).min(1.);
            return (
                px(exit.from + (target - exit.from) * (1. - (1. - t).powi(3))),
                t < 1.,
            );
        }
        if let Some(drag) = &self.drag
            && drag.claimed
        {
            return (px(drag.offset), false);
        }
        if let Some((start, from)) = self.motion {
            let t = (now.duration_since(start).as_secs_f32() / SETTLE.as_secs_f32()).min(1.);
            return (px(from * (1. - t).powi(3)), t < 1.);
        }
        (px(0.), false)
    }

    pub(super) fn measure(&mut self, width: Pixels) {
        self.width = width;
    }

    pub(super) fn set_dismissal(&mut self, direction: Option<SwipeDirection>, now: Instant) {
        if self.dismissal.as_ref().map(|exit| exit.direction) == direction {
            return;
        }
        let from = self.visual(now).0.into();
        self.dismissal = direction.map(|direction| Dismissal {
            direction,
            started: now,
            from,
        });
        self.drag = None;
        self.motion = None;
        self.contacts.clear();
    }

    pub(super) fn collapsed(&self, now: Instant) -> bool {
        self.dismissal
            .as_ref()
            .is_some_and(|exit| self.width == px(0.) || now.duration_since(exit.started) >= EXIT)
    }

    pub(super) fn feedback(&self, offset: Pixels) -> SwipeProgress {
        SwipeProgress {
            displacement: offset,
            direction: if offset < px(0.) {
                SwipeDirection::Left
            } else {
                SwipeDirection::Right
            },
            progress: (f32::from(offset).abs() / self.threshold).min(1.),
            ready: self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.claimed && drag.offset.abs() >= self.threshold),
        }
    }

    pub(super) fn configure(
        &mut self,
        left: bool,
        right: bool,
        threshold: Pixels,
        cx: &mut Context<Self>,
    ) {
        let threshold = f32::from(threshold);
        let threshold = if threshold.is_finite() {
            threshold.max(SLOP * 2.)
        } else {
            96.
        };
        if self.enabled != [left, right] || self.threshold != threshold {
            self.cancel(cx);
            self.enabled = [left, right];
            self.threshold = threshold;
        }
    }

    pub(super) fn claimed(&self) -> bool {
        self.drag.as_ref().is_some_and(|drag| drag.claimed)
    }

    pub(super) fn begin(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.dismissal.is_some() {
            return;
        }
        // A new press has its own distance budget, including during return animation.
        self.motion = None;
        self.drag = Some(Drag {
            pointer,
            start: position,
            offset: 0.,
            claimed: false,
            rejected: false,
        });
        cx.notify();
    }

    pub(super) fn move_to(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self
            .drag
            .as_mut()
            .filter(|drag| drag.pointer == pointer && !drag.rejected)
        else {
            return false;
        };
        let dx = f32::from(position.x - drag.start.x);
        let dy = f32::from(position.y - drag.start.y).abs();
        if !drag.claimed && dx.abs().max(dy) > SLOP {
            if dx.abs() <= dy * 1.2
                || (dx < 0. && !self.enabled[0])
                || (dx > 0. && !self.enabled[1])
            {
                drag.rejected = true;
                return false;
            }
            drag.claimed = true;
        }
        if !drag.claimed {
            return false;
        }
        drag.offset = if (dx < 0. && self.enabled[0]) || (dx > 0. && self.enabled[1]) {
            dx
        } else {
            0.
        };
        cx.notify();
        true
    }

    pub(super) fn end(
        &mut self,
        pointer: Pointer,
        cancelled: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = self.drag.as_ref().filter(|drag| drag.pointer == pointer) else {
            return false;
        };
        let consumed = drag.claimed;
        let trigger = consumed && !cancelled && drag.offset.abs() >= self.threshold;
        let offset = drag.offset;
        self.drag = None;
        if consumed {
            self.motion = Some((cx.background_executor().now(), offset));
            cx.notify();
        }
        if trigger {
            cx.emit(SwipeTriggered {
                direction: if offset < 0. {
                    SwipeDirection::Left
                } else {
                    SwipeDirection::Right
                },
            });
        }
        consumed
    }

    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(pointer) = self.drag.as_ref().map(|drag| drag.pointer) else {
            return false;
        };
        self.end(pointer, true, cx)
    }

    pub(super) fn touch(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        prevented: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let pointer = Pointer::Touch(event.id);
        if event.phase == TouchPhase::Started {
            if self.contacts.contains(&event.id) {
                return false;
            }
            if self.contacts.is_empty() {
                self.contacts.push(event.id);
                if inside && !prevented {
                    self.begin(pointer, event.position, cx);
                }
                return false;
            }
            self.contacts.push(event.id);
            return self.cancel(cx);
        }
        if !self.contacts.contains(&event.id) {
            return false;
        }
        let consumed = if prevented || event.phase == TouchPhase::Cancelled {
            self.cancel(cx)
        } else if event.phase == TouchPhase::Moved {
            self.move_to(pointer, event.position, cx)
        } else {
            if self.claimed() {
                self.move_to(pointer, event.position, cx);
            }
            self.end(pointer, false, cx)
        };
        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.contacts.retain(|id| *id != event.id);
        }
        consumed
    }
}
