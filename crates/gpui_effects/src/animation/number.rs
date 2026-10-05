use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use scheduler::Instant;

/// Interpolates a numeric target and builds caller-styled content from its current value.
/// Keep the ID stable across renders. The first value appears immediately.
pub fn animated_number<E: IntoElement>(
    id: impl Into<ElementId>,
    value: f64,
    build: impl FnOnce(f64) -> E + 'static,
) -> AnimatedNumber {
    AnimatedNumber {
        id: id.into(),
        target: value,
        duration: Duration::from_millis(240),
        enabled: true,
        build: Some(Box::new(move |value| build(value).into_any_element())),
    }
}

/// A numeric transition with application-defined formatting and layout.
/// Values use `f64` precision; non-finite targets snap without interpolation.
pub struct AnimatedNumber {
    id: ElementId,
    target: f64,
    duration: Duration,
    enabled: bool,
    build: Option<Box<dyn FnOnce(f64) -> AnyElement>>,
}

impl AnimatedNumber {
    /// Time to reach a new target. Retargeting starts from the current value.
    /// Zero skips interpolation. Defaults to 240 ms with cubic smoothstep easing.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Snaps to the target when disabled, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

struct NumberState {
    from: f64,
    target: f64,
    started: Instant,
    duration: Duration,
}

impl NumberState {
    fn sample(&self, now: Instant) -> (f64, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.from == self.target
            || !self.from.is_finite()
            || !self.target.is_finite()
            || elapsed >= self.duration
        {
            return (self.target, false);
        }
        let t = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        let eased = t * t * (3. - 2. * t);
        // Weighted endpoints avoid overflowing target - from across opposite extremes.
        (self.from * (1. - eased) + self.target * eased, true)
    }

    fn update(
        &mut self,
        target: f64,
        duration: Duration,
        enabled: bool,
        now: Instant,
    ) -> (f64, bool) {
        let (current, _) = self.sample(now);
        if !enabled || duration.is_zero() || !current.is_finite() || !target.is_finite() {
            self.from = target;
            self.target = target;
        } else if self.target.to_bits() != target.to_bits() || self.duration != duration {
            self.from = current;
            self.target = target;
            self.started = now;
        }
        self.duration = duration;
        self.sample(now)
    }
}

impl IntoElement for AnimatedNumber {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for AnimatedNumber {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let now = cx.background_executor().now();
        let (value, moving) =
            window.with_element_state(id.unwrap(), |state: Option<NumberState>, _| {
                let mut state = state.unwrap_or(NumberState {
                    from: self.target,
                    target: self.target,
                    started: now,
                    duration: self.duration,
                });
                let sample = state.update(self.target, self.duration, self.enabled, now);
                (sample, state)
            });
        if moving {
            window.request_animation_frame();
        }
        let mut child = self
            .build
            .take()
            .expect("animated number layout requested twice")(value);
        (child.request_layout(window, cx), child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests;
