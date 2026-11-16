use gpui_effects::LiquidGlassStyle;
use std::time::{Duration, Instant};

pub(crate) struct MaterialTransition {
    from: LiquidGlassStyle,
    target: LiquidGlassStyle,
    started: Instant,
}
impl MaterialTransition {
    pub(crate) fn new(material: LiquidGlassStyle, now: Instant) -> Self {
        Self {
            from: material,
            target: material,
            started: now,
        }
    }
    pub(crate) fn sample(
        &mut self,
        target: LiquidGlassStyle,
        now: Instant,
        duration: Duration,
    ) -> (LiquidGlassStyle, bool) {
        let current = self.value(now, duration);
        if target != self.target {
            self.from = current;
            self.target = target;
            self.started = now;
        }
        let active = !duration.is_zero()
            && self.from != self.target
            && now.duration_since(self.started) < duration;
        if !active {
            self.from = self.target;
        }
        (self.value(now, duration), active)
    }
    fn value(&self, now: Instant, duration: Duration) -> LiquidGlassStyle {
        let progress = if duration.is_zero() {
            1.
        } else {
            (now.duration_since(self.started).as_secs_f32() / duration.as_secs_f32()).min(1.)
        };
        let t = 1. - (1. - progress).powi(3);
        let mut value = self.target;
        value.refraction =
            self.from.refraction + (self.target.refraction - self.from.refraction) * t;
        value.thickness = self.from.thickness + (self.target.thickness - self.from.thickness) * t;
        value.blur_radius =
            self.from.blur_radius + (self.target.blur_radius - self.from.blur_radius) * t;
        value.frost = self.from.frost + (self.target.frost - self.from.frost) * t;
        value.optics.environment_reflection = self.from.optics.environment_reflection
            + (self.target.optics.environment_reflection - self.from.optics.environment_reflection)
                * t;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interruption_is_continuous_and_settled_material_stops_frames() {
        let now = Instant::now();
        let rest = LiquidGlassStyle::frosted();
        let target = LiquidGlassStyle::clear();
        let duration = Duration::from_millis(120);
        let mut motion = MaterialTransition::new(rest, now);
        assert!(motion.sample(target, now, duration).1);
        let halfway = now + duration / 2;
        let middle = motion.sample(target, halfway, duration).0;
        let reversed = motion.sample(rest, halfway, duration).0;
        assert_eq!(middle, reversed);
        let (settled, active) = motion.sample(rest, halfway + duration, duration);
        assert_eq!(settled, rest);
        assert!(!active);
        assert!(!motion.sample(rest, halfway + duration * 2, duration).1);
        assert_eq!(
            motion.sample(target, halfway + duration * 2, Duration::ZERO),
            (target, false)
        );
    }
}

#[derive(Default)]
pub(crate) struct Spring {
    pub(crate) value: f32,
    pub(crate) velocity: f32,
}
impl Spring {
    pub(crate) fn advance(&mut self, target: f32, seconds: f32, frequency: f32, damping: f32) {
        let displacement = self.value - target;
        let decay = (-frequency * damping * seconds).exp();
        let angular = frequency * (1. - damping * damping).sqrt();
        let phase = angular * seconds;
        let sine = phase.sin() / angular;
        let velocity = self.velocity;
        self.value = target
            + decay
                * (displacement * phase.cos()
                    + (velocity + frequency * damping * displacement) * sine);
        self.velocity = decay
            * (velocity * phase.cos()
                - (frequency * damping * velocity + frequency * frequency * displacement) * sine);
        if (self.value - target).abs() < 0.0001 && self.velocity.abs() < 0.001 {
            self.value = target;
            self.velocity = 0.;
        }
    }
}

pub(crate) struct GlassMotion {
    scale: Spring,
    target: f32,
    last: Instant,
}
impl GlassMotion {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            scale: Spring {
                value: 1.,
                velocity: 0.,
            },
            target: 1.,
            last: now,
        }
    }
    pub(crate) fn sample(&mut self, target: f32, now: Instant, animated: bool) -> (f32, bool) {
        let elapsed = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        if animated {
            self.scale.advance(self.target, elapsed, 22., 0.82);
        } else {
            self.scale.value = target;
            self.scale.velocity = 0.;
        }
        self.target = target;
        let active = self.scale.value != target || self.scale.velocity != 0.;
        (self.scale.value, active)
    }
}

#[cfg(test)]
mod glass_tests {
    use super::*;

    #[test]
    fn elastic_surface_retargets_without_jumping_and_stops_when_released() {
        let now = Instant::now();
        let mut motion = GlassMotion::new(now);
        let (initial, active) = motion.sample(1.03, now, true);
        assert_eq!(initial, 1.);
        assert!(active);
        let halfway = now + Duration::from_millis(90);
        let (hover, _) = motion.sample(1.03, halfway, true);
        assert!(hover > 1.);
        let (pressed, active) = motion.sample(1.05, halfway, true);
        assert_eq!(hover, pressed);
        assert!(active);
        motion.sample(1., halfway, true);
        let (settled, active) = motion.sample(1., now + Duration::from_secs(3), true);
        assert_eq!(settled, 1.);
        assert!(!active);
        assert_eq!(
            motion.sample(1.05, now + Duration::from_secs(3), false),
            (1.05, false)
        );
    }
}
