use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, StyleRefinement, Styled, Window, div,
};
use scheduler::Instant;

use super::{TransitionKind, presence::PresenceState, transition::subtree_transition_with_ids};

/// Switches between values using a timed two-content transition.
/// Supply a stable ID and explicit dimensions, or fill a bounded parent.
pub fn animated_switch<K: Clone + Eq + 'static, E: IntoElement>(
    id: impl Into<ElementId>,
    value: K,
    build: impl FnMut(&K) -> E + 'static,
) -> AnimatedSwitch<K> {
    let mut build = build;
    AnimatedSwitch {
        id: id.into(),
        value,
        build: Box::new(move |value| build(value).into_any_element()),
        duration: Duration::from_millis(240),
        enabled: true,
        kind: TransitionKind::CrossFade,
        style: StyleRefinement::default(),
    }
}

/// An application-controlled value switch. Builders must support the previous and
/// current values; each is rebuilt while visible. Keep values small and immutable.
pub struct AnimatedSwitch<K> {
    id: ElementId,
    value: K,
    build: Box<dyn FnMut(&K) -> AnyElement>,
    duration: Duration,
    enabled: bool,
    kind: TransitionKind,
    style: StyleRefinement,
}

impl<K> AnimatedSwitch<K> {
    /// Full transition duration. Reversals take proportional time. Zero snaps.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    /// Snap to the latest requested value when disabled, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Select a built-in appearance. Defaults to `CrossFade`.
    pub fn kind(mut self, kind: TransitionKind) -> Self {
        self.kind = kind;
        self
    }
}

struct SwitchState<K> {
    from: K,
    to: Option<K>,
    from_slot: usize,
    to_slot: usize,
    next_slot: usize,
    goal: f32,
    motion: PresenceState,
}

impl<K: Clone + Eq> SwitchState<K> {
    fn new(value: K, duration: Duration, now: Instant) -> Self {
        Self {
            from: value,
            to: None,
            from_slot: 0,
            to_slot: 1,
            next_slot: 1,
            goal: 1.,
            motion: PresenceState::transition(1., 1., duration, Duration::ZERO, now),
        }
    }

    fn settle(&mut self, progress: f32) {
        if let Some(to) = self.to.take()
            && progress == 1.
        {
            self.from = to;
            self.from_slot = self.to_slot;
        }
    }

    fn update(
        &mut self,
        value: &K,
        duration: Duration,
        enabled: bool,
        now: Instant,
    ) -> (f32, bool) {
        if !enabled || duration.is_zero() {
            if self.to.as_ref() == Some(value) {
                self.from_slot = self.to_slot;
            } else if &self.from != value {
                self.from_slot = self.next_slot;
                self.next_slot = self.next_slot.wrapping_add(1);
            }
            self.from = value.clone();
            self.to = None;
            self.motion = PresenceState::transition(1., 1., duration, Duration::ZERO, now);
            return (1., false);
        }
        let (progress, active) = self.motion.sample(now);
        if !active {
            self.settle(progress);
        }
        if let Some(to) = &self.to {
            if value == &self.from {
                self.goal = 0.;
            } else if value == to {
                self.goal = 1.;
            }
            let (progress, active) = self.motion.update(self.goal, duration, true, now);
            if active {
                return (progress, true);
            }
            self.settle(progress);
        }
        if value != &self.from {
            self.to = Some(value.clone());
            self.to_slot = self.next_slot;
            self.next_slot = self.next_slot.wrapping_add(1);
            self.goal = 1.;
            self.motion = PresenceState::transition(0., 1., duration, Duration::ZERO, now);
            (0., true)
        } else {
            (1., false)
        }
    }
}

impl<K: Clone + Eq + 'static> IntoElement for AnimatedSwitch<K> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<K> Styled for AnimatedSwitch<K> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<K: Clone + Eq + 'static> Element for AnimatedSwitch<K> {
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
        let (from, to, slots, progress, active) =
            window.with_element_state(id.unwrap(), |state: Option<SwitchState<K>>, _| {
                let mut state = state
                    .unwrap_or_else(|| SwitchState::new(self.value.clone(), self.duration, now));
                let (progress, active) =
                    state.update(&self.value, self.duration, self.enabled, now);
                (
                    (
                        state.from.clone(),
                        state.to.clone(),
                        [state.from_slot, state.to_slot],
                        progress,
                        active,
                    ),
                    state,
                )
            });
        if active {
            window.request_animation_frame();
        }
        let from = (self.build)(&from);
        let (to, progress) = if let Some(to) = to {
            ((self.build)(&to), progress)
        } else {
            (div().into_any_element(), 0.)
        };
        let mut transition = subtree_transition_with_ids(
            "transition",
            slots.map(|slot| ("slot", slot).into()),
            from,
            to,
        )
        .progress(progress)
        .kind(self.kind);
        *transition.style() = std::mem::take(&mut self.style);
        let mut child = transition.into_any_element();
        (child.request_layout(window, cx), (child, active))
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
