use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, ContentMask, Display, Element, ElementId, GlobalElementId,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, Length, Overflow, Pixels, Position,
    Style, Window, div, point, prelude::*, px, relative, size,
};
use scheduler::Instant;

/// Animates an intrinsically sized child's height in normal document flow.
/// Keep the wrapper mounted with a stable ID, including when collapsed.
pub fn animated_collapse<E: IntoElement>(
    id: impl Into<ElementId>,
    expanded: bool,
    build: impl FnOnce() -> E + 'static,
) -> AnimatedCollapse {
    AnimatedCollapse {
        id: id.into(),
        expanded,
        duration: Duration::from_millis(260),
        enabled: true,
        build: Some(Box::new(move || build().into_any_element())),
    }
}

/// A full-width clipping container with measured-height transitions.
/// Style the content for padding and appearance; constrain width on the parent.
pub struct AnimatedCollapse {
    id: ElementId,
    expanded: bool,
    duration: Duration,
    enabled: bool,
    build: Option<Box<dyn FnOnce() -> AnyElement>>,
}

impl AnimatedCollapse {
    /// Time to reach a new target height. Zero uses natural layout immediately.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Disable interpolation, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

struct CollapseState {
    measured: Pixels,
    from: Pixels,
    target: Pixels,
    started: Instant,
    duration: Duration,
}

impl CollapseState {
    fn sample(&self, now: Instant) -> (Pixels, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.from == self.target || elapsed >= self.duration {
            return (self.target, false);
        }
        let t = (elapsed.as_secs_f64() / self.duration.as_secs_f64()) as f32;
        let eased = t * t * (3. - 2. * t);
        (self.from + (self.target - self.from) * eased, true)
    }

    fn retarget(&mut self, target: Pixels, duration: Duration, snap: bool, now: Instant) {
        let (current, _) = self.sample(now);
        if snap || duration.is_zero() {
            self.from = target;
            self.target = target;
        } else if target != self.target || duration != self.duration {
            self.from = current;
            self.target = target;
            self.started = now;
        }
        self.duration = duration;
    }
}

impl IntoElement for AnimatedCollapse {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for AnimatedCollapse {
    type RequestLayoutState = (Option<(AnyElement, LayoutId)>, bool, bool);
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
        let (height, moving, natural) =
            window.with_element_state(id.unwrap(), |state: Option<CollapseState>, _| {
                let natural =
                    self.expanded && (state.is_none() || !self.enabled || self.duration.is_zero());
                let mut state = state.unwrap_or(CollapseState {
                    measured: px(0.),
                    from: px(0.),
                    target: px(0.),
                    started: now,
                    duration: self.duration,
                });
                let target = if self.expanded {
                    state.measured
                } else {
                    px(0.)
                };
                state.retarget(target, self.duration, !self.enabled, now);
                let (height, moving) = state.sample(now);
                ((height, moving, natural), state)
            });
        if moving {
            window.request_animation_frame();
        }
        let hidden = !self.expanded && !moving;
        let mut content = if hidden {
            None
        } else {
            Some(
                div()
                    .id("content")
                    .w_full()
                    .flex()
                    .flex_col()
                    .when(!natural, |child| child.absolute().top_0().left_0())
                    .child(self.build.take().expect("collapse layout requested twice")())
                    .into_any_element(),
            )
        };
        let child_id = content
            .as_mut()
            .map(|child| child.request_layout(window, cx));
        let style = Style {
            display: if hidden { Display::None } else { Display::Flex },
            position: Position::Relative,
            size: size(
                relative(1.).into(),
                if natural { Length::Auto } else { height.into() },
            ),
            min_size: size(px(0.).into(), px(0.).into()),
            flex_shrink: 0.,
            overflow: point(Overflow::Hidden, Overflow::Hidden),
            ..Default::default()
        };
        let layout_id = window.request_layout(style, child_id, cx);
        (layout_id, (content.zip(child_id), natural, moving))
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some((child, child_id)) = &mut state.0 else {
            return;
        };
        let measured = window.layout_bounds(*child_id).size.height.max(px(0.));
        let now = cx.background_executor().now();
        let changed =
            window.with_element_state(id.unwrap(), |previous: Option<CollapseState>, _| {
                let mut previous = previous.expect("collapse state must exist after layout");
                let changed = previous.measured != measured;
                previous.measured = measured;
                previous.retarget(
                    if self.expanded { measured } else { px(0.) },
                    self.duration,
                    state.1,
                    now,
                );
                (changed, previous)
            });
        // Height is discovered after the parent's layout; adopt the new target next frame.
        if changed && !state.1 {
            window.request_animation_frame();
        }
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            child.prepaint(window, cx);
            if state.2 || !self.expanded {
                window.insert_hitbox(bounds, HitboxBehavior::BlockMouse);
            }
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some((child, _)) = &mut state.0 {
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                child.paint(window, cx)
            });
        }
    }
}

#[cfg(test)]
mod tests;
