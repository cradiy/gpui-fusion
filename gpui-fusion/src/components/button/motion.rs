use crate::components::motion::Spring;
use gpui::{Pixels, Size, size};
use std::time::Instant;

pub(super) struct CursorLight {
    amount: f32,
    target: f32,
    last: Instant,
}

impl CursorLight {
    pub fn new(now: Instant) -> Self {
        Self {
            amount: 0.,
            target: 0.,
            last: now,
        }
    }

    pub fn sample(&mut self, active: bool, now: Instant, animated: bool) -> (f32, bool) {
        let target = f32::from(active);
        if animated {
            let elapsed = now.duration_since(self.last).as_secs_f32();
            self.amount += (self.target - self.amount) * (1. - (-elapsed * 6.).exp());
            if (self.target - self.amount).abs() < 0.002 {
                self.amount = self.target;
            }
        } else {
            self.amount = target;
        }
        self.target = target;
        self.last = now;
        (self.amount, self.amount != target)
    }
}

pub(super) struct ButtonMotion {
    springs: [Spring; 4],
    targets: [f32; 4],
    last: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ButtonFrame {
    press: f32,
    hover: [f32; 2],
    pub label_scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;
    use std::time::Duration;

    #[test]
    fn cursor_light_fades_continuously_and_stops_at_rest() {
        let start = Instant::now();
        let mut light = CursorLight::new(start);
        assert_eq!(light.sample(true, start, true), (0., true));
        let midway = start + Duration::from_millis(120);
        let (amount, active) = light.sample(true, midway, true);
        assert!(active && amount > 0. && amount < 1.);
        assert_eq!(light.sample(false, midway, true), (amount, true));
        assert_eq!(
            light.sample(false, midway + Duration::from_secs(2), true),
            (0., false)
        );
        assert_eq!(
            light.sample(true, midway + Duration::from_secs(2), false),
            (1., false)
        );
    }

    #[test]
    fn hover_axes_overshoot_out_of_phase_and_release_settles() {
        let start = Instant::now();
        let mut motion = ButtonMotion::new(start);
        motion.sample(false, true, start, true);
        let target = ButtonMotion::new(start).sample(false, true, start, false).0;
        let mut peaks = [(0., 0); 2];
        for step in 1..100 {
            let frame = motion
                .sample(false, true, start + Duration::from_millis(step * 8), true)
                .0;
            for (axis, peak) in peaks.iter_mut().enumerate() {
                if frame.hover[axis] > peak.0 {
                    *peak = (frame.hover[axis], step);
                }
            }
        }
        assert!(peaks[0].0 > target.hover[0] && peaks[1].0 > target.hover[1]);
        assert_ne!(peaks[0].1, peaks[1].1);
        let release = start + Duration::from_secs(1);
        let before = motion.sample(false, true, release, true).0;
        assert_eq!(motion.sample(false, false, release, true).0, before);
        let (rest, active) = motion.sample(false, false, release + Duration::from_secs(5), true);
        assert!(!active);
        let bounds = size(px(132.), px(36.));
        assert_eq!(rest.lens_size(bounds), bounds);
        assert_eq!(rest.label_scale, 1.);
    }

    #[test]
    fn pressing_preserves_hover_growth_and_can_be_interrupted_without_a_jump() {
        let start = Instant::now();
        let mut motion = ButtonMotion::new(start);
        let hover = motion.sample(false, true, start, false).0;
        let (initial, active) = motion.sample(true, true, start, true);
        assert!(active);
        assert_eq!(initial, hover);
        let middle = start + Duration::from_millis(84);
        let pressed = motion.sample(true, true, middle, true).0;
        let bounds = size(px(132.), px(36.));
        assert!(pressed.lens_size(bounds).height > hover.lens_size(bounds).height);
        assert!(pressed.label_scale > hover.label_scale);
        assert_eq!(motion.sample(false, true, middle, true).0, pressed);
        let (rest, active) = motion.sample(false, false, middle, false);
        assert!(!active);
        assert_eq!(rest.lens_size(bounds), bounds);
    }
}

impl ButtonFrame {
    pub fn lens_size(self, bounds: Size<Pixels>) -> Size<Pixels> {
        let scale = 1. + self.press.clamp(-0.12, 1.15) * 0.08;
        let growth = bounds.height * 0.16;
        size(
            bounds.width * scale + growth * self.hover[0],
            bounds.height * scale + growth * self.hover[1],
        )
    }
}

impl ButtonMotion {
    pub fn new(now: Instant) -> Self {
        Self {
            springs: [
                Spring::default(),
                Spring::default(),
                Spring::default(),
                Spring {
                    value: 1.,
                    velocity: 0.,
                },
            ],
            targets: [0., 0., 0., 1.],
            last: now,
        }
    }

    pub fn sample(
        &mut self,
        pressed: bool,
        hovered: bool,
        now: Instant,
        animated: bool,
    ) -> (ButtonFrame, bool) {
        let elapsed = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let targets = [
            f32::from(pressed),
            f32::from(hovered),
            f32::from(hovered),
            if pressed {
                1.08
            } else if hovered {
                1.04
            } else {
                1.
            },
        ];
        for (index, (frequency, damping)) in [(16., 0.6), (17., 0.42), (12., 0.34), (16., 0.6)]
            .into_iter()
            .enumerate()
        {
            let spring = &mut self.springs[index];
            if animated {
                spring.advance(self.targets[index], elapsed, frequency, damping);
            } else {
                spring.value = targets[index];
                spring.velocity = 0.;
            }
        }
        self.targets = targets;
        let active = self
            .springs
            .iter()
            .zip(targets)
            .any(|(spring, target)| spring.value != target || spring.velocity != 0.);
        (
            ButtonFrame {
                press: self.springs[0].value,
                hover: [self.springs[1].value, self.springs[2].value],
                label_scale: self.springs[3].value,
            },
            active,
        )
    }
}
