use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, size,
};
use scheduler::Instant;

/// Animates an absolute rectangle relative to its positioned parent.
/// Use a stable ID for each item. The first appearance uses the target immediately.
pub fn layout_transition(id: impl Into<ElementId>, target: Bounds<Pixels>) -> LayoutTransition {
    assert!(
        [
            target.origin.x,
            target.origin.y,
            target.size.width,
            target.size.height
        ]
        .into_iter()
        .all(|v| f32::from(v).is_finite())
            && f32::from(target.size.width) >= 0.
            && f32::from(target.size.height) >= 0.,
        "layout transition requires finite bounds and nonnegative dimensions"
    );
    LayoutTransition {
        id: id.into(),
        target,
        duration: Duration::from_millis(260),
        enabled: true,
        content: Some(div().id("surface")),
    }
}

/// A positioned container whose children reflow within its animated size.
/// The target owns position and size; style the surface and children normally.
/// Surrounding flow does not reserve space for this absolute element.
pub struct LayoutTransition {
    id: ElementId,
    target: Bounds<Pixels>,
    duration: Duration,
    enabled: bool,
    content: Option<Stateful<Div>>,
}

impl LayoutTransition {
    /// Duration of each transition, including a retargeted transition. Zero snaps.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Disable motion and immediately adopt the target, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub(super) struct LayoutState {
    from: Bounds<Pixels>,
    target: Bounds<Pixels>,
    started: Instant,
    duration: Duration,
}

impl LayoutState {
    pub(super) fn new(target: Bounds<Pixels>, duration: Duration, now: Instant) -> Self {
        Self {
            from: target,
            target,
            started: now,
            duration,
        }
    }

    fn sample(&self, now: Instant) -> (Bounds<Pixels>, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.from == self.target || elapsed >= self.duration {
            return (self.target, false);
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let t = 1. - (1. - t).powi(3);
        (
            Bounds {
                origin: self.from.origin + (self.target.origin - self.from.origin) * t,
                size: size(
                    self.from.size.width + (self.target.size.width - self.from.size.width) * t,
                    self.from.size.height + (self.target.size.height - self.from.size.height) * t,
                ),
            },
            true,
        )
    }

    pub(super) fn update(
        &mut self,
        target: Bounds<Pixels>,
        duration: Duration,
        enabled: bool,
        now: Instant,
    ) -> (Bounds<Pixels>, bool) {
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

impl IntoElement for LayoutTransition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Styled for LayoutTransition {
    fn style(&mut self) -> &mut StyleRefinement {
        self.content
            .as_mut()
            .expect("cannot style after layout")
            .style()
    }
}

impl ParentElement for LayoutTransition {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content
            .as_mut()
            .expect("cannot add children after layout")
            .extend(elements);
    }
}

impl InteractiveElement for LayoutTransition {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.content
            .as_mut()
            .expect("cannot change interactivity after layout")
            .interactivity()
    }
}

impl StatefulInteractiveElement for LayoutTransition {}

impl Element for LayoutTransition {
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
        let (bounds, moving) =
            window.with_element_state(id.unwrap(), |state: Option<LayoutState>, _| {
                let mut state =
                    state.unwrap_or_else(|| LayoutState::new(self.target, self.duration, now));
                let sampled = state.update(self.target, self.duration, self.enabled, now);
                (sampled, state)
            });
        if moving {
            window.request_animation_frame();
        }
        let mut content = self
            .content
            .take()
            .expect("layout requested twice")
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .min_w(bounds.size.width)
            .max_w(bounds.size.width)
            .min_h(bounds.size.height)
            .max_h(bounds.size.height)
            .into_any_element();
        (content.request_layout(window, cx), content)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        content: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        content.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        content: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        content.paint(window, cx);
    }
}

#[cfg(test)]
mod tests;
