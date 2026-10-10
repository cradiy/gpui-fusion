use gpui::{
    Along, App, Axis, Context, EventEmitter, FocusHandle, Focusable, HitboxId, Pixels, Point,
    Subscription, TouchId, Window,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitPaneEvent {
    Changing(f32),
    Changed(f32),
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Limits {
    pub min: f32,
    pub max: f32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            min: 0.,
            max: f32::INFINITY,
        }
    }
}
impl Limits {
    pub fn validate(self) {
        assert!(
            self.min.is_finite() && self.min >= 0. && self.max >= self.min,
            "pane sizes must be nonnegative with max >= min"
        );
    }
}

/// Returns the feasible interval for the first pane, excluding the divider.
pub(super) fn interval(space: f32, first: Limits, second: Limits) -> (f32, f32) {
    if space < first.min + second.min {
        let position = space * (first.min / (first.min + second.min));
        return (position, position);
    }
    if space > first.max + second.max {
        // Keep both maxima as lower bounds and distribute unavoidable excess by ratio.
        return (first.max, space - second.max);
    }
    (
        first.min.max(space - second.max),
        first.max.min(space - second.min),
    )
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pointer {
    Mouse,
    Touch(TouchId),
}
#[derive(Clone, Copy)]
struct Drag {
    pointer: Pointer,
    origin: f32,
    first: f32,
    ratio: f32,
}

/// Retained sizing and divider focus for one split in one window.
pub struct SplitPaneState {
    ratio: f32,
    default_ratio: f32,
    pub(super) axis: Axis,
    pub(super) space: f32,
    pub(super) first: f32,
    pub(super) interval: (f32, f32),
    drag: Option<Drag>,
    pub(super) capture: Option<HitboxId>,
    focus: FocusHandle,
    _subscriptions: [Subscription; 2],
}
impl EventEmitter<SplitPaneEvent> for SplitPaneState {}
impl Focusable for SplitPaneState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl SplitPaneState {
    pub fn new(ratio: f32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        validate_ratio(ratio);
        Self {
            ratio,
            default_ratio: ratio,
            axis: Axis::Horizontal,
            space: 0.,
            first: 0.,
            interval: (0., 0.),
            drag: None,
            capture: None,
            focus: cx.focus_handle(),
            _subscriptions: [
                window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
                cx.observe_window_activation(window, |state, window, cx| {
                    if !window.is_window_active() {
                        state.cancel(cx);
                    }
                }),
            ],
        }
    }
    /// Preferred first-pane fraction of the space remaining after the divider.
    pub fn ratio(&self) -> f32 {
        self.ratio
    }
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
    /// Restore a saved ratio without emitting a user interaction event.
    pub fn set_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        validate_ratio(ratio);
        self.drag = None;
        self.ratio = ratio;
        cx.notify();
    }
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.drag = None;
        self.commit(self.default_ratio, cx);
    }
    pub(super) fn measure(
        &mut self,
        axis: Axis,
        space: f32,
        limits: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        if self.axis != axis || self.space != space || self.interval != limits {
            self.cancel(cx);
        }
        self.axis = axis;
        self.space = space;
        self.interval = limits;
        self.first = (space * self.ratio).clamp(limits.0, limits.1);
    }
    pub(super) fn begin(&mut self, pointer: Pointer, position: Point<Pixels>) -> bool {
        if self.drag.is_some() || self.space <= 0. || self.interval.0 == self.interval.1 {
            return false;
        }
        self.drag = Some(Drag {
            pointer,
            origin: f32::from(position.along(self.axis)),
            first: (self.space * self.ratio).clamp(self.interval.0, self.interval.1),
            ratio: self.ratio,
        });
        true
    }
    pub(super) fn owns(&self, pointer: Pointer) -> bool {
        self.drag.is_some_and(|drag| drag.pointer == pointer)
    }
    pub(super) fn move_to(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.drag.filter(|drag| drag.pointer == pointer) else {
            return;
        };
        let first = (drag.first + f32::from(position.along(self.axis)) - drag.origin)
            .clamp(self.interval.0, self.interval.1);
        let ratio = first / self.space;
        if first != (self.space * self.ratio).clamp(self.interval.0, self.interval.1) {
            self.ratio = ratio;
            cx.emit(SplitPaneEvent::Changing(ratio));
            cx.notify();
        }
    }
    pub(super) fn end(&mut self, pointer: Pointer, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.filter(|drag| drag.pointer == pointer) {
            self.drag = None;
            if self.ratio != drag.ratio {
                cx.emit(SplitPaneEvent::Changed(self.ratio));
            }
            cx.notify();
        }
    }
    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            self.ratio = drag.ratio;
            cx.notify();
        }
    }
    pub(super) fn adjust(&mut self, delta: f32, cx: &mut Context<Self>) {
        if self.space > 0. {
            let first = (self.space * self.ratio).clamp(self.interval.0, self.interval.1);
            self.commit(
                (first + delta).clamp(self.interval.0, self.interval.1) / self.space,
                cx,
            );
        }
    }
    pub(super) fn edge(&mut self, end: bool, cx: &mut Context<Self>) {
        if self.space > 0. {
            self.commit(
                if end {
                    self.interval.1
                } else {
                    self.interval.0
                } / self.space,
                cx,
            );
        }
    }
    fn commit(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.ratio != ratio {
            self.ratio = ratio;
            cx.emit(SplitPaneEvent::Changed(ratio));
            cx.notify();
        }
    }
}
fn validate_ratio(ratio: f32) {
    assert!(
        ratio.is_finite() && (0.0..=1.0).contains(&ratio),
        "split ratio must be between 0 and 1"
    );
}
