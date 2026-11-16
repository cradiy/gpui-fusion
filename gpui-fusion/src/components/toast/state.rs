use super::{Toast, ToastId};
use gpui::{FocusHandle, Subscription};
use scheduler::Instant;
use std::time::Duration;

pub(super) struct Entry {
    pub id: ToastId,
    pub toast: Toast,
    pub focus: FocusHandle,
    pub restore_focus: Option<FocusHandle>,
    pub hovered: bool,
    pub focused: bool,
    pub remaining: Option<Duration>,
    pub deadline: Option<Instant>,
    pub motion: Motion,
    pub _subscriptions: Vec<Subscription>,
}
impl Entry {
    pub fn sync_deadline(&mut self, now: Instant, active: bool) {
        if self.motion.exiting() {
            self.deadline = None;
            return;
        }
        if self.hovered || self.focused || !active {
            if let Some(deadline) = self.deadline.take() {
                self.remaining = Some(deadline.saturating_duration_since(now));
            }
        } else if self.deadline.is_none() {
            self.deadline = self
                .remaining
                .and_then(|duration| now.checked_add(duration));
        }
    }
}

pub(super) struct Motion {
    from: f32,
    target: f32,
    start: Instant,
    duration: Duration,
}
impl Motion {
    pub fn new(now: Instant, duration: Duration) -> Self {
        Self {
            from: 0.,
            target: 1.,
            start: now,
            duration,
        }
    }
    pub fn started(&self) -> Instant {
        self.start
    }
    pub fn sample(&self, now: Instant) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.start);
        if self.duration.is_zero() || elapsed >= self.duration {
            return (self.target, false);
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        (
            self.from + (self.target - self.from) * t * t * (3. - 2. * t),
            true,
        )
    }
    pub fn retarget(&mut self, target: f32, now: Instant, duration: Duration) {
        self.from = self.sample(now).0;
        self.target = target;
        self.start = now;
        self.duration = duration.mul_f32((target - self.from).abs());
    }
    pub fn exiting(&self) -> bool {
        self.target == 0.
    }
    pub fn end(&self) -> Instant {
        self.start + self.duration
    }
}
