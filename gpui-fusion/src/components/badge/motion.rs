use gpui::{Pixels, Point, point, px};
use scheduler::Instant;
use std::time::Duration;

#[derive(Clone, Copy)]
struct Drag {
    pointer: Point<Pixels>,
    origin: Point<Pixels>,
    last: Point<Pixels>,
    sampled: Instant,
}
#[derive(Clone, Copy)]
struct Motion {
    id: u64,
    start: Instant,
    duration: Duration,
    from: Point<Pixels>,
    velocity: Point<f32>,
    target: Point<Pixels>,
    dismiss: bool,
}
#[derive(Clone, Copy)]
pub(super) struct Visual {
    pub offset: Point<Pixels>,
    pub opacity: f32,
    pub active: bool,
    pub dismissing: bool,
    pub completion: Option<u64>,
}
#[derive(Default)]
pub(super) struct DragState {
    offset: Point<Pixels>,
    velocity: Point<f32>,
    drag: Option<Drag>,
    motion: Option<Motion>,
    generation: u64,
    dismissed: bool,
}
impl DragState {
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }
    pub fn reset(&mut self) {
        self.offset = Point::default();
        self.velocity = Point::default();
        self.drag = None;
        self.motion = None;
        self.dismissed = false;
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn start(&mut self, pointer: Point<Pixels>, now: Instant) {
        self.offset = self.visual(now).offset;
        self.velocity = Point::default();
        self.motion = None;
        self.drag = Some(Drag {
            pointer,
            origin: self.offset,
            last: pointer,
            sampled: now,
        });
    }
    pub fn move_to(&mut self, pointer: Point<Pixels>, now: Instant) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        let dt = now.duration_since(drag.sampled).as_secs_f32();
        // A release at the last sampled position must not erase the flick velocity.
        if pointer != drag.last || dt > 0.08 {
            let sample = point(
                f32::from(pointer.x - drag.last.x) / dt.max(0.001),
                f32::from(pointer.y - drag.last.y) / dt.max(0.001),
            );
            let retain = (-dt / 0.045).exp();
            self.velocity = point(
                self.velocity.x * retain + sample.x * (1. - retain),
                self.velocity.y * retain + sample.y * (1. - retain),
            );
            drag.last = pointer;
            drag.sampled = now;
        }
        self.offset = drag.origin + pointer - drag.pointer;
    }
    pub fn release(
        &mut self,
        pointer: Point<Pixels>,
        threshold: Pixels,
        now: Instant,
        animated: bool,
    ) {
        if self.drag.is_none() {
            return;
        }
        self.move_to(pointer, now);
        self.drag = None;
        let projected =
            self.offset + point(px(self.velocity.x * 0.085), px(self.velocity.y * 0.085));
        let dismiss = length(projected) >= f32::from(threshold);
        let target = if dismiss {
            let distance = length(projected).max(0.001);
            let travel = 150. + (self.velocity.x.hypot(self.velocity.y) * 0.055).min(170.);
            self.offset + projected * (travel / distance)
        } else {
            Point::default()
        };
        self.animate(target, dismiss, now, animated);
    }
    pub fn cancel(&mut self, now: Instant, animated: bool) {
        self.offset = self.visual(now).offset;
        self.drag = None;
        self.velocity = Point::default();
        self.animate(Point::default(), false, now, animated);
    }
    fn animate(&mut self, target: Point<Pixels>, dismiss: bool, now: Instant, animated: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.motion = Some(Motion {
            id: self.generation,
            start: now,
            duration: if !animated {
                Duration::ZERO
            } else {
                Duration::from_millis(if dismiss { 280 } else { 460 })
            },
            from: self.offset,
            velocity: self.velocity,
            target,
            dismiss,
        });
    }
    pub fn visual(&self, now: Instant) -> Visual {
        let Some(m) = self.motion else {
            return Visual {
                offset: self.offset,
                opacity: if self.dismissed { 0. } else { 1. },
                active: false,
                dismissing: false,
                completion: None,
            };
        };
        let t = if m.duration.is_zero() {
            1.
        } else {
            (now.duration_since(m.start).as_secs_f32() / m.duration.as_secs_f32()).clamp(0., 1.)
        };
        let offset = if t >= 1. {
            m.target
        } else if m.dismiss {
            m.from + (m.target - m.from) * (1. - (1. - t).powi(4))
        } else {
            let elapsed = t * m.duration.as_secs_f32();
            let decay = (-12. * elapsed).exp();
            let angular = 14.;
            // The endpoint correction settles exactly without a final position jump.
            let settle = 1. - t * t * t;
            let axis = |from: Pixels, to: Pixels, v: f32| {
                let x = f32::from(from - to);
                to + px((x * (angular * elapsed).cos()
                    + ((v + 12. * x) / angular) * (angular * elapsed).sin())
                    * decay
                    * settle)
            };
            point(
                axis(m.from.x, m.target.x, m.velocity.x),
                axis(m.from.y, m.target.y, m.velocity.y),
            )
        };
        Visual {
            offset,
            opacity: if m.dismiss {
                1. - t * t * (3. - 2. * t)
            } else {
                1.
            },
            active: t < 1.,
            dismissing: m.dismiss,
            completion: (t >= 1.).then_some(m.id),
        }
    }
    pub fn finish(&mut self, id: u64) -> bool {
        let Some(m) = self.motion else {
            return false;
        };
        if m.id != id {
            return false;
        }
        self.offset = m.target;
        self.motion = None;
        self.velocity = Point::default();
        self.dismissed = m.dismiss;
        m.dismiss
    }
}
pub(super) fn length(offset: Point<Pixels>) -> f32 {
    f32::from(offset.x).hypot(f32::from(offset.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flick_projection_hold_and_stale_completions() {
        let now = Instant::now();
        let origin = point(px(0.), px(0.));
        let release = point(px(32.), px(0.));
        let mut state = DragState::default();
        state.start(origin, now);
        state.move_to(release, now + Duration::from_millis(20));
        state.release(release, px(72.), now + Duration::from_millis(21), false);
        let exit = state.visual(now + Duration::from_millis(21));
        assert!(exit.dismissing && !exit.active);
        let stale = exit.completion.unwrap();
        state.reset();
        assert!(!state.finish(stale));
        assert!(!state.is_dismissed());
        state.start(origin, now);
        state.move_to(release, now + Duration::from_millis(20));
        state.release(release, px(72.), now + Duration::from_millis(500), true);
        let returning = state.visual(now + Duration::from_millis(500));
        assert!(!returning.dismissing && returning.active);
        assert_eq!(returning.offset, release);
        let settled = state.visual(now + Duration::from_secs(1));
        assert_eq!(settled.offset, origin);
        assert!(!settled.active);
        assert!(!state.finish(settled.completion.unwrap()));
        assert!(
            state
                .visual(now + Duration::from_secs(2))
                .completion
                .is_none()
        );
    }
}
