use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Point, StyleRefinement,
    Styled, Window, div, point, prelude::*, px,
};

use super::presence::PresenceState;

/// A styled container that fades and slides into view inside its ancestor clips.
/// Keep a stable ID and mounted layout slot to retain its playback history.
pub fn scroll_reveal(id: impl Into<ElementId>) -> ScrollReveal {
    ScrollReveal {
        id: id.into(),
        style: StyleRefinement::default(),
        children: Vec::new(),
        duration: Duration::from_millis(360),
        delay: Duration::ZERO,
        offset: point(px(0.), px(18.)),
        threshold: 0.15,
        once: true,
        enabled: true,
    }
}

/// Preserves normal layout while animating opacity and a visual offset.
/// Visibility uses the unanimated layout bounds and rectangular content clips;
/// sibling occlusion and captured transforms are not visibility signals.
pub struct ScrollReveal {
    id: ElementId,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    duration: Duration,
    delay: Duration,
    offset: Point<Pixels>,
    threshold: f32,
    once: bool,
    enabled: bool,
}

impl ScrollReveal {
    /// Entrance duration, sampled when an entrance begins. Defaults to 360 ms.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Delay before each entrance, sampled when it begins. Defaults to zero.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Initial visual displacement. It does not change layout or trigger bounds.
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        if f32::from(offset.x).is_finite() && f32::from(offset.y).is_finite() {
            self.offset = offset;
        }
        self
    }

    /// Required visible fraction of the layout area, clamped to `0..=1`.
    /// Zero still requires positive intersection. Defaults to 0.15.
    pub fn threshold(mut self, threshold: f32) -> Self {
        if threshold.is_finite() {
            self.threshold = threshold.clamp(0., 1.);
        }
        self
    }

    /// Play only on the first entrance. False rearms after leaving the clip fully.
    pub fn once(mut self, once: bool) -> Self {
        self.once = once;
        self
    }

    /// Show content immediately without motion, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Default)]
struct RevealState {
    entrance: Option<PresenceState>,
}

impl Styled for ScrollReveal {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for ScrollReveal {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}

impl IntoElement for ScrollReveal {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ScrollReveal {
    type RequestLayoutState = (AnyElement, f32);
    type PrepaintState = bool;

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
        let progress = window.with_element_state(id.unwrap(), |state: Option<RevealState>, _| {
            let state = state.unwrap_or_default();
            let progress = if !self.enabled || self.duration.is_zero() {
                1.
            } else {
                state
                    .entrance
                    .as_ref()
                    .map_or(0., |entry| entry.sample(now).0)
            };
            (progress, state)
        });
        let mut content = div()
            .id("content")
            .children(std::mem::take(&mut self.children));
        *content.style() = std::mem::take(&mut self.style);
        let opacity = content.style().opacity.unwrap_or(1.);
        let mut content = content.opacity(opacity * progress).into_any_element();
        (content.request_layout(window, cx), (content, progress))
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let clip = window.content_mask().bounds.intersect(&Bounds {
            origin: Point::default(),
            size: window.viewport_size(),
        });
        let visible = bounds.intersect(&clip);
        let area =
            f64::from(f32::from(bounds.size.width)) * f64::from(f32::from(bounds.size.height));
        let visible_area =
            f64::from(f32::from(visible.size.width)) * f64::from(f32::from(visible.size.height));
        let in_view = area > 0. && visible_area > 0.;
        let eligible = in_view && visible_area / area >= f64::from(self.threshold);
        let now = cx.background_executor().now();
        let snap = !self.enabled || self.duration.is_zero();
        let needs_frame =
            window.with_element_state(id.unwrap(), |previous: Option<RevealState>, _| {
                let mut previous = previous.expect("reveal state must exist after layout");
                if !in_view && !self.once {
                    previous.entrance = None;
                } else if in_view && snap {
                    previous.entrance = Some(PresenceState::transition(
                        1.,
                        1.,
                        Duration::ZERO,
                        Duration::ZERO,
                        now,
                    ));
                } else if eligible && previous.entrance.is_none() {
                    previous.entrance = Some(PresenceState::transition(
                        0.,
                        1.,
                        self.duration,
                        self.delay,
                        now,
                    ));
                }
                let (progress, moving) = previous
                    .entrance
                    .as_ref()
                    .map_or((0., false), |entry| entry.sample(now));
                (
                    in_view && (moving || (!snap && progress != state.1)),
                    previous,
                )
            });
        // Visibility is known after layout. Start playback on the next frame.
        if needs_frame {
            window.request_animation_frame();
        }
        if !in_view || state.1 <= 0. {
            return false;
        }
        let offset = self.offset * (1. - state.1);
        window.with_element_offset(offset, |window| {
            state.0.prepaint(window, cx);
            if state.1 < 1. {
                window.insert_hitbox(
                    Bounds {
                        origin: bounds.origin + offset,
                        ..bounds
                    },
                    HitboxBehavior::BlockMouseExceptScroll,
                );
            }
        });
        true
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        painted: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *painted {
            state.0.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests;
