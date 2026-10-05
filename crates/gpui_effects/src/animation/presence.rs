use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, Styled, Window, div,
};
use scheduler::Instant;

/// Lifecycle of a mounted presence child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresencePhase {
    Entering,
    Visible,
    Exiting,
}

/// Current visibility weight and lifecycle for an application-defined animation.
#[derive(Clone, Copy, Debug)]
pub struct PresenceFrame {
    /// Zero is hidden and one is fully visible.
    pub progress: f32,
    pub phase: PresencePhase,
}

/// Keeps building a child until its exit completes. Keep this element mounted
/// with a stable ID and change `visible`; styling belongs on the built child.
pub fn animated_presence<E: IntoElement>(
    id: impl Into<ElementId>,
    visible: bool,
    build: impl FnOnce(PresenceFrame) -> E + 'static,
) -> AnimatedPresence {
    AnimatedPresence {
        id: id.into(),
        visible,
        duration: Duration::from_millis(240),
        enabled: true,
        animate_initial: false,
        build: Some(Box::new(move |frame| build(frame).into_any_element())),
    }
}

/// Caller-styled entrance and exit animation driven by a boolean target.
/// Hidden children have no layout, painting or hitboxes. Ordinary mouse input
/// is blocked during transitions; the application owns keyboard focus.
pub struct AnimatedPresence {
    id: ElementId,
    visible: bool,
    duration: Duration,
    enabled: bool,
    animate_initial: bool,
    build: Option<Box<dyn FnOnce(PresenceFrame) -> AnyElement>>,
}

impl AnimatedPresence {
    /// Full hidden-to-visible travel time. Reversals take proportional time.
    /// Zero snaps immediately. Interpolation uses a cubic smoothstep.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Snaps to the target when disabled, including immediate removal on exit.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Animate a visible child on its first appearance. Defaults to false.
    pub fn animate_initial(mut self, animate: bool) -> Self {
        self.animate_initial = animate;
        self
    }
}

struct PresenceState {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}

impl PresenceState {
    fn sample(&self, now: Instant) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        let travel = self.duration.as_secs_f64() * f64::from((self.target - self.from).abs());
        if elapsed >= travel {
            return (self.target, false);
        }
        let t = (elapsed / travel) as f32;
        let eased = t * t * (3. - 2. * t);
        (self.from + (self.target - self.from) * eased, true)
    }

    fn update(
        &mut self,
        target: f32,
        duration: Duration,
        enabled: bool,
        now: Instant,
    ) -> (f32, bool) {
        let (current, _) = self.sample(now);
        if !enabled || duration.is_zero() {
            self.from = target;
            self.target = target;
        } else if self.target != target || self.duration != duration {
            self.from = current;
            self.target = target;
            self.started = now;
        }
        self.duration = duration;
        self.sample(now)
    }
}

impl IntoElement for AnimatedPresence {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for AnimatedPresence {
    type RequestLayoutState = (AnyElement, bool);
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let now = cx.background_executor().now();
        let target = if self.visible { 1. } else { 0. };
        let (progress, moving) =
            window.with_element_state(id.unwrap(), |state: Option<PresenceState>, _| {
                let mut state = state.unwrap_or(PresenceState {
                    from: if self.animate_initial { 0. } else { target },
                    target,
                    started: now,
                    duration: self.duration,
                });
                let result = state.update(target, self.duration, self.enabled, now);
                (result, state)
            });
        if moving {
            window.request_animation_frame();
        }
        let mut child = if !moving && !self.visible {
            div().hidden().into_any_element()
        } else {
            let phase = if !moving {
                PresencePhase::Visible
            } else if self.visible {
                PresencePhase::Entering
            } else {
                PresencePhase::Exiting
            };
            self.build.take().expect("presence layout requested twice")(PresenceFrame {
                progress,
                phase,
            })
        };
        (child.request_layout(window, cx), (child, moving))
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.0.prepaint(window, cx);
        if state.1 {
            window.insert_hitbox(bounds, HitboxBehavior::BlockMouse);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        state.0.paint(window, cx);
    }
}

#[cfg(test)]
mod tests;
