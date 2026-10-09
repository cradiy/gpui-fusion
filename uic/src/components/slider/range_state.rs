use std::ops::RangeInclusive;

use gpui::{App, Context, FocusHandle, Focusable};

use super::interaction::{CaptureToken, InteractionPhase};
use crate::components::range::NumericRange;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeSliderThumb {
    Lower,
    Upper,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RangeSliderEvent {
    Changing(RangeInclusive<f64>),
    Changed(RangeInclusive<f64>),
}

pub struct RangeSliderState {
    values: RangeInclusive<f64>,
    range: NumericRange,
    step: f64,
    min_gap: f64,
    disabled: bool,
    dragging: Option<RangeSliderThumb>,
    lower_focus: FocusHandle,
    upper_focus: FocusHandle,
    pub(super) capture: CaptureToken,
}

impl gpui::EventEmitter<RangeSliderEvent> for RangeSliderState {}

impl Focusable for RangeSliderState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.lower_focus.clone()
    }
}

impl RangeSliderState {
    pub fn new(
        values: RangeInclusive<f64>,
        range: RangeInclusive<f64>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut state = Self {
            values,
            range: NumericRange::new(range),
            step: 0.,
            min_gap: 0.,
            disabled: false,
            dragging: None,
            lower_focus: cx.focus_handle(),
            upper_focus: cx.focus_handle(),
            capture: CaptureToken::default(),
        };
        state.normalize();
        state
    }

    /// A zero step allows continuous pointer input.
    pub fn step(mut self, step: f64) -> Self {
        assert!(
            step.is_finite() && step >= 0.,
            "step must be finite and nonnegative"
        );
        self.step = step;
        self.normalize();
        self
    }

    /// Minimum distance between endpoints, capped at the domain's span.
    pub fn min_gap(mut self, gap: f64) -> Self {
        assert!(
            gap.is_finite() && gap >= 0.,
            "gap must be finite and nonnegative"
        );
        self.min_gap = gap.min(self.range.span());
        self.normalize();
        self
    }

    pub fn values(&self) -> RangeInclusive<f64> {
        self.values.clone()
    }

    pub fn range(&self) -> RangeInclusive<f64> {
        self.range.as_inclusive()
    }

    pub fn step_size(&self) -> f64 {
        self.step
    }

    pub fn minimum_gap(&self) -> f64 {
        self.min_gap
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    pub fn thumb_focus_handle(&self, thumb: RangeSliderThumb) -> FocusHandle {
        match thumb {
            RangeSliderThumb::Lower => self.lower_focus.clone(),
            RangeSliderThumb::Upper => self.upper_focus.clone(),
        }
    }

    /// Normalizes a replacement without emitting a user interaction event.
    pub fn set_values(&mut self, values: RangeInclusive<f64>, cx: &mut Context<Self>) {
        let previous = self.values.clone();
        self.values = values;
        self.normalize();
        if self.values != previous {
            cx.notify();
        }
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        self.dragging = None;
        self.capture.clear_touch();
        cx.notify();
    }

    pub(super) fn value(&self, thumb: RangeSliderThumb) -> f64 {
        match thumb {
            RangeSliderThumb::Lower => *self.values.start(),
            RangeSliderThumb::Upper => *self.values.end(),
        }
    }

    pub(super) fn ratio(&self, thumb: RangeSliderThumb) -> f32 {
        self.range.ratio(self.value(thumb))
    }

    pub(super) fn effective_step(&self) -> f64 {
        if self.step > 0. {
            self.step
        } else {
            self.range.span() / 100.
        }
    }

    pub(super) fn limits(&self, thumb: RangeSliderThumb) -> RangeInclusive<f64> {
        match thumb {
            RangeSliderThumb::Lower => {
                self.range.min()..=self.floor(*self.values.end() - self.min_gap)
            }
            RangeSliderThumb::Upper => {
                self.ceil(*self.values.start() + self.min_gap)..=self.range.max()
            }
        }
    }

    pub(super) fn commit(&mut self, thumb: RangeSliderThumb, value: f64, cx: &mut Context<Self>) {
        if !self.disabled && self.adjust(thumb, value) {
            cx.emit(RangeSliderEvent::Changed(self.values()));
            cx.notify();
        }
    }

    pub(super) fn pointer(
        &mut self,
        ratio: f32,
        phase: InteractionPhase,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        if self.disabled || phase == InteractionPhase::Hover {
            return None;
        }
        if phase == InteractionPhase::Cancel {
            self.dragging = None;
            cx.notify();
            return None;
        }
        let value = self.range.value_at(ratio);
        if phase == InteractionPhase::Start {
            let lower = self.value(RangeSliderThumb::Lower);
            let upper = self.value(RangeSliderThumb::Upper);
            self.dragging = Some(if value - lower <= upper - value {
                RangeSliderThumb::Lower
            } else {
                RangeSliderThumb::Upper
            });
        }
        let thumb = self.dragging?;
        let changed = self.adjust(thumb, value);
        if phase == InteractionPhase::Commit {
            self.dragging = None;
            cx.emit(RangeSliderEvent::Changed(self.values()));
        } else if changed {
            cx.emit(RangeSliderEvent::Changing(self.values()));
        }
        if changed || phase != InteractionPhase::Preview {
            cx.notify();
        }
        (phase == InteractionPhase::Start).then(|| self.thumb_focus_handle(thumb))
    }

    fn adjust(&mut self, thumb: RangeSliderThumb, value: f64) -> bool {
        let limits = self.limits(thumb);
        let value = self.snap(value).clamp(*limits.start(), *limits.end());
        if self.value(thumb) == value {
            return false;
        }
        self.values = match thumb {
            RangeSliderThumb::Lower => value..=*self.values.end(),
            RangeSliderThumb::Upper => *self.values.start()..=value,
        };
        true
    }

    fn normalize(&mut self) {
        let a = self.range.clamp(*self.values.start());
        let b = self.range.clamp(*self.values.end());
        let lower = self
            .snap(a.min(b))
            .min(self.floor(self.range.max() - self.min_gap));
        let upper = self.snap(a.max(b)).max(self.ceil(lower + self.min_gap));
        self.values = lower..=upper;
    }

    fn snap(&self, value: f64) -> f64 {
        let value = self.range.clamp(value);
        if value == self.range.max() {
            value
        } else {
            self.range.snap(value, self.step)
        }
    }

    fn floor(&self, value: f64) -> f64 {
        if self.step == 0. || value >= self.range.max() {
            self.range.clamp(value)
        } else {
            self.range
                .clamp(self.range.min() + self.grid_position(value).floor() * self.step)
        }
    }

    fn ceil(&self, value: f64) -> f64 {
        if self.step == 0. {
            self.range.clamp(value)
        } else {
            self.range
                .clamp(self.range.min() + self.grid_position(value).ceil() * self.step)
        }
    }

    fn grid_position(&self, value: f64) -> f64 {
        let position = (value - self.range.min()) / self.step;
        let nearest = position.round();
        if (position - nearest).abs() <= f64::EPSILON * position.abs().max(1.) * 4. {
            nearest
        } else {
            position
        }
    }
}
