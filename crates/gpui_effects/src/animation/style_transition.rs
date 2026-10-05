use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Rgba, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
};
use scheduler::Instant;

/// A normal styled container with timed transitions for explicit paint properties.
/// Supply target styles on each render; interaction state belongs to the application.
pub fn animated_style(id: impl Into<ElementId>) -> AnimatedStyle {
    AnimatedStyle {
        id: id.into(),
        content: Some(div().id("content")),
        style: StyleRefinement::default(),
        duration: Duration::from_millis(180),
        enabled: true,
    }
}

/// Animates solid background, border and text colors, corner radii and opacity.
/// Other styles apply immediately. Native `.hover(...)` refinements are not animated;
/// use application state to update the base styles instead.
pub struct AnimatedStyle {
    id: ElementId,
    content: Option<Stateful<Div>>,
    style: StyleRefinement,
    duration: Duration,
    enabled: bool,
}

impl AnimatedStyle {
    /// Full duration after each target change. Zero snaps. Defaults to 180 ms.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    /// Snap to target styles when disabled, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PaintValues {
    background: Option<Hsla>,
    border: Option<Hsla>,
    text: Option<Hsla>,
    corners: [Option<Pixels>; 4],
    opacity: Option<f32>,
}

impl PaintValues {
    fn read(style: &StyleRefinement, rem_size: Pixels) -> Self {
        let finite_color = |color: Hsla| {
            [color.h, color.s, color.l, color.a]
                .iter()
                .all(|v| v.is_finite())
        };
        Self {
            background: style
                .background
                .as_ref()
                .and_then(|fill| fill.color())
                .and_then(|bg| bg.as_solid())
                .filter(|color| finite_color(*color)),
            border: style.border_color.filter(|color| finite_color(*color)),
            text: style.text.color.filter(|color| finite_color(*color)),
            corners: [
                style.corner_radii.top_left,
                style.corner_radii.top_right,
                style.corner_radii.bottom_right,
                style.corner_radii.bottom_left,
            ]
            .map(|radius| {
                radius
                    .map(|radius| radius.to_pixels(rem_size))
                    .filter(|radius| f32::from(*radius).is_finite())
            }),
            opacity: style.opacity.filter(|value| value.is_finite()),
        }
    }

    fn apply(self, style: &mut StyleRefinement) {
        if let Some(color) = self.background {
            style.background = Some(color.into());
        }
        if let Some(color) = self.border {
            style.border_color = Some(color);
        }
        if let Some(color) = self.text {
            style.text.color = Some(color);
        }
        style.corner_radii.top_left = self.corners[0]
            .map(Into::into)
            .or(style.corner_radii.top_left);
        style.corner_radii.top_right = self.corners[1]
            .map(Into::into)
            .or(style.corner_radii.top_right);
        style.corner_radii.bottom_right = self.corners[2]
            .map(Into::into)
            .or(style.corner_radii.bottom_right);
        style.corner_radii.bottom_left = self.corners[3]
            .map(Into::into)
            .or(style.corner_radii.bottom_left);
        if let Some(opacity) = self.opacity {
            style.opacity = Some(opacity);
        }
    }

    fn start_from(self, current: Self) -> Self {
        fn both<T: Copy>(current: Option<T>, target: Option<T>) -> Option<T> {
            if current.is_some() && target.is_some() {
                current
            } else {
                target
            }
        }
        Self {
            background: both(current.background, self.background),
            border: both(current.border, self.border),
            text: both(current.text, self.text),
            opacity: both(current.opacity, self.opacity),
            corners: std::array::from_fn(|i| both(current.corners[i], self.corners[i])),
        }
    }

    fn mix(self, target: Self, t: f32) -> Self {
        fn mix_option<T>(a: Option<T>, b: Option<T>, mix: impl FnOnce(T, T) -> T) -> Option<T> {
            match (a, b) {
                (Some(a), Some(b)) => Some(mix(a, b)),
                (_, b) => b,
            }
        }
        Self {
            background: mix_option(self.background, target.background, |a, b| {
                mix_color(a, b, t)
            }),
            border: mix_option(self.border, target.border, |a, b| mix_color(a, b, t)),
            text: mix_option(self.text, target.text, |a, b| mix_color(a, b, t)),
            corners: std::array::from_fn(|i| {
                mix_option(self.corners[i], target.corners[i], |a, b| {
                    a * (1. - t) + b * t
                })
            }),
            opacity: mix_option(self.opacity, target.opacity, |a, b| a * (1. - t) + b * t),
        }
    }
}

fn mix_color(a: Hsla, b: Hsla, t: f32) -> Hsla {
    if t == 0. {
        return a;
    }
    if t == 1. {
        return b;
    }
    let a = Rgba::from(a);
    let b = Rgba::from(b);
    let alpha = a.a * (1. - t) + b.a * t;
    if alpha <= 0. {
        return Rgba::default().into();
    }
    let channel = |a_channel, b_channel| (a_channel * a.a * (1. - t) + b_channel * b.a * t) / alpha;
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
    .into()
}

struct StyleState {
    from: PaintValues,
    target: PaintValues,
    started: Instant,
    duration: Duration,
}

impl StyleState {
    fn sample(&self, now: Instant) -> (PaintValues, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.from == self.target || elapsed >= self.duration {
            return (self.target, false);
        }
        let t = (elapsed.as_secs_f64() / self.duration.as_secs_f64()) as f32;
        (self.from.mix(self.target, t * t * (3. - 2. * t)), true)
    }
    fn update(
        &mut self,
        target: PaintValues,
        duration: Duration,
        enabled: bool,
        now: Instant,
    ) -> (PaintValues, bool) {
        let current = self.sample(now).0;
        if !enabled || duration.is_zero() {
            self.from = target;
            self.target = target;
        } else if target != self.target || duration != self.duration {
            self.from = target.start_from(current);
            self.target = target;
            self.started = now;
        }
        self.duration = duration;
        self.sample(now)
    }
}

impl Styled for AnimatedStyle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl ParentElement for AnimatedStyle {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.content
            .as_mut()
            .expect("cannot add children after layout")
            .extend(children);
    }
}
impl InteractiveElement for AnimatedStyle {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.content
            .as_mut()
            .expect("cannot change interactivity after layout")
            .interactivity()
    }
}
impl StatefulInteractiveElement for AnimatedStyle {}
impl IntoElement for AnimatedStyle {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for AnimatedStyle {
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
        let target = PaintValues::read(&self.style, window.rem_size());
        let now = cx.background_executor().now();
        let (values, moving) =
            window.with_element_state(id.unwrap(), |state: Option<StyleState>, _| {
                let mut state = state.unwrap_or(StyleState {
                    from: target,
                    target,
                    started: now,
                    duration: self.duration,
                });
                let result = state.update(target, self.duration, self.enabled, now);
                (result, state)
            });
        if moving {
            window.request_animation_frame();
            values.apply(&mut self.style);
        }
        let mut content = self
            .content
            .take()
            .expect("animated style layout requested twice");
        *content.style() = std::mem::take(&mut self.style);
        let mut content = content.into_any_element();
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
