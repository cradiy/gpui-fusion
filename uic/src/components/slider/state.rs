use std::ops::RangeInclusive;

use gpui::{Context, FocusHandle, Focusable};

use super::{SliderEvent, interaction::CaptureToken};
use crate::components::range::NumericRange;

pub struct SliderState {
    value: f64,
    range: NumericRange,
    step: f64,
    disabled: bool,
    dragging: bool,
    hovered: bool,
    hover_ratio: Option<f32>,
    focus_handle: FocusHandle,
    capture: CaptureToken,
}

impl gpui::EventEmitter<SliderEvent> for SliderState {}

impl SliderState {
    pub fn new(value: f64, range: RangeInclusive<f64>, cx: &mut Context<Self>) -> Self {
        let range = NumericRange::new(range);
        Self {
            value: range.clamp(value),
            range,
            step: 0.0,
            disabled: false,
            dragging: false,
            hovered: false,
            hover_ratio: None,
            focus_handle: cx.focus_handle(),
            capture: CaptureToken::default(),
        }
    }

    /// Sets the snapping interval. `0.0` keeps pointer input continuous.
    pub fn step(mut self, step: f64) -> Self {
        self.step = valid_step(step);
        self.value = self.range.snap(self.value, self.step);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn range(&self) -> RangeInclusive<f64> {
        self.range.as_inclusive()
    }

    pub fn step_size(&self) -> f64 {
        self.step
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the pointer is currently dragging this slider.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Returns whether the pointer is currently over this slider.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        let value = self.range.snap(value, self.step);
        if self.value != value {
            self.value = value;
            cx.notify();
        }
    }

    pub fn set_range(&mut self, range: RangeInclusive<f64>, cx: &mut Context<Self>) {
        self.range = NumericRange::new(range);
        self.value = self.range.snap(self.value, self.step);
        cx.notify();
    }

    pub fn set_step(&mut self, step: f64, cx: &mut Context<Self>) {
        self.step = valid_step(step);
        self.value = self.range.snap(self.value, self.step);
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            if disabled {
                self.capture.clear_touch();
                self.dragging = false;
                self.hovered = false;
                self.hover_ratio = None;
            }
            cx.notify();
        }
    }

    pub(super) fn ratio(&self) -> f32 {
        self.range.ratio(self.value)
    }

    pub(super) fn ratio_for(&self, value: f64) -> f32 {
        self.range.ratio(value)
    }

    pub(super) fn hover_ratio(&self) -> Option<f32> {
        self.hover_ratio
    }

    pub(super) fn value_for_ratio(&self, ratio: f32) -> f64 {
        self.range.snap(self.range.value_at(ratio), self.step)
    }

    pub(super) fn focus(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub(super) fn capture(&self) -> CaptureToken {
        self.capture.clone()
    }

    pub(super) fn preview_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.hover_ratio = Some(ratio);
        let value = self.range.snap(self.range.value_at(ratio), self.step);
        if self.value == value {
            cx.notify();
            return;
        }
        self.value = value;
        cx.emit(SliderEvent::Changing(value));
        cx.notify();
    }

    pub(super) fn start_drag(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.dragging = true;
        self.hover_ratio = Some(ratio);
        self.preview_ratio(ratio, cx);
        cx.notify();
    }

    pub(super) fn preview_hover_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.disabled || self.hover_ratio == Some(ratio) {
            return;
        }
        self.hover_ratio = Some(ratio);
        cx.notify();
    }

    pub(super) fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let hovered = hovered && !self.disabled;
        if self.hovered != hovered {
            self.hovered = hovered;
            if !hovered && !self.dragging {
                self.hover_ratio = None;
            }
            cx.notify();
        }
    }

    pub(super) fn commit_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.dragging = false;
        self.hover_ratio = self.hovered.then_some(ratio);
        self.value = self.range.snap(self.range.value_at(ratio), self.step);
        cx.emit(SliderEvent::Changed(self.value));
        cx.notify();
    }

    pub(super) fn commit_delta(&mut self, delta: f64, cx: &mut Context<Self>) {
        self.commit_value(self.value + delta, cx);
    }

    pub(super) fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        self.hover_ratio = None;
        cx.notify();
    }

    pub(super) fn commit_value(&mut self, value: f64, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let value = self.range.snap(value, self.step);
        if self.value == value {
            return;
        }
        self.value = value;
        cx.emit(SliderEvent::Changed(value));
        cx.notify();
    }

    pub(super) fn effective_step(&self) -> f64 {
        if self.step > 0.0 {
            self.step
        } else {
            self.range.span() / 100.0
        }
    }

    pub(super) fn min(&self) -> f64 {
        self.range.min()
    }

    pub(super) fn max(&self) -> f64 {
        self.range.max()
    }
}

impl Focusable for SliderState {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn valid_step(step: f64) -> f64 {
    if step.is_finite() && step > 0.0 {
        step
    } else {
        0.0
    }
}
