use crate::components::motion::Spring;
use std::time::{Duration, Instant};

/// Interpolates a motion channel with continuous velocity and acceleration.
pub(super) fn phase(value: f32, start: f32, end: f32) -> f32 {
    let t = ((value - start) / (end - start)).clamp(0., 1.);
    t * t * t * (t * (t * 6. - 15.) + 10.)
}

pub(super) struct MenuMotion {
    spring: Spring,
    target: f32,
    last: Instant,
}
impl MenuMotion {
    pub fn new(now: Instant) -> Self {
        Self {
            spring: Spring::default(),
            target: 0.,
            last: now,
        }
    }
    pub fn sample(&mut self, expanded: bool, now: Instant, duration: Duration) -> f32 {
        let next = if expanded { 1. } else { 0. };
        if duration.is_zero() {
            self.spring.value = next;
            self.spring.velocity = 0.;
        } else {
            self.spring.advance(
                self.target,
                now.duration_since(self.last).as_secs_f32(),
                if self.target > 0. { 8. } else { 10. } / duration.as_secs_f32(),
                0.9,
            );
        }
        self.target = next;
        self.last = now;
        self.spring.value.clamp(0., 1.)
    }
    pub fn active(&self) -> bool {
        self.spring.value != self.target || self.spring.velocity != 0.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reversing_preserves_surface_position_and_velocity_then_settles() {
        let now = Instant::now();
        let duration = Duration::from_millis(300);
        let mut motion = MenuMotion::new(now);
        motion.sample(true, now, duration);
        let midway = now + Duration::from_millis(80);
        let position = motion.sample(true, midway, duration);
        let velocity = motion.spring.velocity;
        assert!(position > 0. && position < 1. && velocity > 0.);
        assert_eq!(motion.sample(false, midway, duration), position);
        assert_eq!(motion.spring.velocity, velocity);
        assert!(motion.active());
        assert_eq!(
            motion.sample(false, now + Duration::from_secs(2), duration),
            0.
        );
        assert!(!motion.active());
        assert_eq!(
            motion.sample(true, now + Duration::from_secs(2), Duration::ZERO),
            1.
        );
        assert!(!motion.active());
    }
}
