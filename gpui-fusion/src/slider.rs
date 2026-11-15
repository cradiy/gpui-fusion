use crate::{
    button_motion::Spring,
    runtime::ThemeRuntime,
    style::{ComputedStyle, Part, Resolved, State, StateSelector, StylePatch, Styled},
    theme::{ResolveContext, StyleError, ThemeScope},
};
use gpui::{Styled as _, prelude::*, *};
use std::{cell::Cell, fmt, rc::Rc, time::Instant};

/// Finite limits and an optional step anchored at the minimum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderRange {
    min: f64,
    max: f64,
    step: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSliderRange;
impl fmt::Display for InvalidSliderRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("slider limits must be finite and increasing, with a finite representable positive step")
    }
}
impl std::error::Error for InvalidSliderRange {}
impl Default for SliderRange {
    fn default() -> Self {
        Self {
            min: 0.,
            max: 100.,
            step: Some(1.),
        }
    }
}
impl SliderRange {
    pub fn new(min: f64, max: f64, step: Option<f64>) -> Result<Self, InvalidSliderRange> {
        let span = max - min;
        if !min.is_finite()
            || !max.is_finite()
            || !span.is_finite()
            || span <= 0.
            || step.is_some_and(|step| {
                !step.is_finite()
                    || step <= 0.
                    || !(span / step).is_finite()
                    || min + step == min
                    || max - step == max
            })
        {
            return Err(InvalidSliderRange);
        }
        Ok(Self { min, max, step })
    }
    pub fn min(self) -> f64 {
        self.min
    }
    pub fn max(self) -> f64 {
        self.max
    }
    pub fn step(self) -> Option<f64> {
        self.step
    }
    fn snap(self, value: f64) -> f64 {
        if value <= self.min {
            return self.min;
        }
        if value >= self.max {
            return self.max;
        }
        self.step.map_or(value, |step| {
            (self.min + ((value - self.min) / step).round() * step).clamp(self.min, self.max)
        })
    }
    fn fraction(self, value: f64) -> f32 {
        ((value - self.min) / (self.max - self.min)) as f32
    }
    fn value_at(self, position: f32) -> f64 {
        self.snap(self.min + f64::from(position.clamp(0., 1.)) * (self.max - self.min))
    }
    fn increment(self) -> f64 {
        self.step.unwrap_or((self.max - self.min) / 100.)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SliderPart {
    Track,
    Fill,
    Thumb,
}
impl Part for SliderPart {
    fn name(&self) -> &str {
        match self {
            Self::Track => "track",
            Self::Fill => "fill",
            Self::Thumb => "thumb",
        }
    }
}

type Change = Rc<dyn Fn(f64, &mut Window, &mut App)>;
struct Drag {
    origin: Pixels,
    start: f32,
}
#[derive(Clone, Copy)]
struct Geometry {
    start: Pixels,
    travel: Pixels,
    thumb: Bounds<Pixels>,
}

/// A horizontal numeric control. The default range is 0..=100 with a step of 1.
pub struct Slider {
    theme: Entity<ThemeRuntime>,
    style_set: String,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    style: StylePatch,
    range: SliderRange,
    value: f64,
    disabled: bool,
    hovered: bool,
    key_pressed: bool,
    drag: Option<Drag>,
    geometry: Rc<Cell<Option<Geometry>>>,
    focus: FocusHandle,
    label: SharedString,
    on_change: Option<Change>,
    _subscriptions: Vec<Subscription>,
    position: Spring,
    expansion: Spring,
    last_frame: Instant,
    animated: bool,
    error: Option<StyleError>,
    validated_revision: Option<u64>,
    resolved: Option<(u64, u8, [ComputedStyle; 4])>,
}
impl Slider {
    /// Nonfinite initial values use the minimum; subsequent nonfinite updates are ignored.
    pub fn new(
        label: impl Into<SharedString>,
        value: f64,
        theme: Entity<ThemeRuntime>,
        style_set: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.drag = None;
                this.key_pressed = false;
                cx.notify();
            }),
        ];
        let range = SliderRange::default();
        let value = if value.is_finite() { value } else { range.min };
        Self {
            theme,
            style_set: style_set.into(),
            variant: None,
            scopes: Vec::new(),
            style: StylePatch::default(),
            range,
            value,
            disabled: false,
            hovered: false,
            key_pressed: false,
            drag: None,
            geometry: Rc::new(Cell::new(None)),
            focus,
            label: label.into(),
            on_change: None,
            _subscriptions: subscriptions,
            position: Spring {
                value: range.fraction(range.snap(value)),
                velocity: 0.,
            },
            expansion: Spring::default(),
            last_frame: Instant::now(),
            animated: true,
            error: None,
            validated_revision: None,
            resolved: None,
        }
    }
    pub fn range(mut self, range: SliderRange) -> Self {
        self.range = range;
        self.value = range.snap(self.value);
        self.position = Spring {
            value: range.fraction(self.value),
            velocity: 0.,
        };
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.validated_revision = None;
        self.variant = Some(name.into());
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.validated_revision = None;
        self.scopes.push(scope);
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn on_change(mut self, callback: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn value(&self) -> f64 {
        self.range.snap(self.value)
    }
    pub fn limits(&self) -> SliderRange {
        self.range
    }
    /// Updates application state without emitting a user change.
    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        if !value.is_finite() {
            return;
        }
        let value = self.range.snap(value);
        let cancelled = self.drag.take().is_some();
        if value != self.value || cancelled {
            self.value = value;
            cx.notify();
        }
    }
    pub fn set_range(&mut self, range: SliderRange, cx: &mut Context<Self>) {
        if self.range != range {
            self.range = range;
            self.value = range.snap(self.value);
            self.drag = None;
            cx.notify();
        }
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            self.drag = None;
            self.key_pressed = false;
            self.hovered = false;
            cx.notify();
        }
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.validated_revision = None;
        cx.notify();
    }
    pub fn set_variant(&mut self, variant: Option<String>, cx: &mut Context<Self>) {
        self.variant = variant;
        self.validated_revision = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn change(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || !value.is_finite() {
            return;
        }
        let value = self.range.snap(value);
        if self.value() != value {
            self.value = value;
            cx.notify();
            if let Some(callback) = self.on_change.clone() {
                callback(value, window, cx);
            }
        }
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<[ComputedStyle; 4], StyleError> {
        let store = self.theme.read(cx).store();
        let revision = store.revision();
        let flags = [
            (State::Disabled, self.disabled),
            (State::Hovered, !self.disabled && self.hovered),
            (
                State::Pressed,
                !self.disabled && (self.drag.is_some() || self.key_pressed),
            ),
            (
                State::FocusVisible,
                !self.disabled && self.focus.is_focused(window) && window.last_input_was_keyboard(),
            ),
        ];
        let key = flags
            .iter()
            .enumerate()
            .fold(0u8, |key, (bit, (_, active))| {
                key | (u8::from(*active) << bit)
            });
        if self.validated_revision == Some(revision) {
            if let Some((cached_revision, cached_key, styles)) = &self.resolved {
                if *cached_revision == revision && *cached_key == key {
                    return Ok(styles.clone());
                }
            }
        }
        self.resolved = None;
        let mut context = ResolveContext::new(store.theme(), store.mode(), &self.style_set)?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        if self.validated_revision != Some(revision) {
            self.style.validate(&context, &["track", "fill", "thumb"])?;
            self.validated_revision = Some(revision);
        }
        let states = StateSelector(
            flags
                .into_iter()
                .filter_map(|(state, active)| active.then_some(state))
                .collect(),
        );
        let resolve = |part| {
            context.resolve_recipe(
                "slider",
                self.variant.as_deref(),
                &self.style,
                &states,
                part,
            )
        };
        let styles = [
            resolve(None)?,
            resolve(Some("track"))?,
            resolve(Some("fill"))?,
            resolve(Some("thumb"))?,
        ];
        self.resolved = Some((revision, key, styles.clone()));
        Ok(styles)
    }
}
impl Styled for Slider {
    type Part = SliderPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.validated_revision = None;
        &mut self.style
    }
}
impl Focusable for Slider {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Slider {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [root, track, fill, thumb] = match self.resolve(window, cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.geometry.set(None);
                self.error = Some(error.clone());
                return div()
                    .child(format!("Slider style error: {error}"))
                    .into_any_element();
            }
        };
        let target = self.range.fraction(self.value());
        let pressed = f32::from(!self.disabled && (self.drag.is_some() || self.key_pressed));
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.032);
        self.last_frame = now;
        if self.animated {
            self.position.advance(target, dt, 30., 0.85);
            self.expansion.advance(pressed, dt, 18., 0.7);
        } else {
            self.position = Spring {
                value: target,
                velocity: 0.,
            };
            self.expansion = Spring {
                value: pressed,
                velocity: 0.,
            };
        }
        if self.position.value != target
            || self.position.velocity != 0.
            || self.expansion.value != pressed
            || self.expansion.velocity != 0.
        {
            window.request_animation_frame();
        }
        let fraction = self.position.value.clamp(0., 1.);
        let expansion = self.expansion.value.clamp(0., 1.1);
        let geometry = self.geometry.clone();
        let events = cx.weak_entity();
        let mut element = div()
            .on_paint_before_children(move |bounds, _, window, _| {
                let width = length(&thumb.width, px(22.)).min(bounds.size.width);
                let height = length(&thumb.height, px(22.)).min(bounds.size.height);
                let start = bounds.left() + width / 2.;
                let travel = (bounds.size.width - width).max(px(0.));
                let center = point(start + travel * fraction, bounds.center().y);
                let track_height = length(&track.height, px(6.)).min(bounds.size.height);
                let rail = Bounds::new(
                    point(start, center.y - track_height / 2.),
                    size(travel, track_height),
                );
                paint_part(rail, &track, window);
                if fraction > 0. {
                    paint_part(
                        Bounds::new(rail.origin, size(travel * fraction, track_height)),
                        &fill,
                        window,
                    );
                }
                let glass = thumb.material.is_some();
                let dimensions = size(
                    width * (1. + if glass { expansion * 0.35 } else { 0. }),
                    height * (1. + if glass { expansion * 0.25 } else { 0. }),
                );
                let lens = Bounds::new(
                    center - point(dimensions.width / 2., dimensions.height / 2.),
                    dimensions,
                );
                geometry.set(Some(Geometry {
                    start,
                    travel,
                    thumb: lens,
                }));
                if let Some(mut material) = thumb.material {
                    let press = expansion.min(1.);
                    material.frost *= 1. - press;
                    material.refraction *= press;
                    material.thickness = material.thickness.min(lens.size.height / 2.);
                    material.shadow.strength *= 0.25 + 0.75 * press;
                    let radius =
                        length(&thumb.radius, lens.size.height / 2.).min(lens.size.height / 2.);
                    gpui_effects::paint_liquid_glass(lens, Corners::all(radius), material, window);
                    // A keyboard focus outline remains outside the refractive surface.
                    if length(&thumb.border_width, px(0.)) > px(0.) {
                        let mut outline = thumb.clone();
                        outline.background = Resolved::Clear;
                        outline.background_gradient = None;
                        paint_part(lens, &outline, window);
                    }
                } else {
                    paint_part(lens, &thumb, window);
                }
                let moving = events.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.pressed_button != Some(MouseButton::Left)
                    {
                        return;
                    }
                    let Some(entity) = moving.upgrade() else {
                        return;
                    };
                    if entity.read(cx).drag.is_none() {
                        return;
                    }
                    entity.update(cx, |this, cx| {
                        if let Some(drag) = &this.drag {
                            let position = drag.start
                                + (event.position.x - drag.origin).as_f32()
                                    / travel.max(px(1.)).as_f32();
                            this.change(this.range.value_at(position), window, cx);
                        }
                    });
                });
                let release = events.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                        return;
                    }
                    let Some(entity) = release.upgrade() else {
                        return;
                    };
                    if entity.read(cx).drag.is_some() {
                        entity.update(cx, |this, cx| {
                            this.drag = None;
                            cx.notify();
                        });
                    }
                });
            })
            .id("slider")
            .debug_selector(|| "fusion-slider".into())
            .track_focus(&self.focus)
            .tab_stop(!self.disabled)
            .role(Role::Slider)
            .aria_label(self.label.clone())
            .aria_disabled(self.disabled)
            .aria_numeric_value(self.value())
            .aria_min_numeric_value(self.range.min)
            .aria_max_numeric_value(self.range.max)
            .aria_numeric_value_step(self.range.increment())
            .relative()
            .flex_shrink_0();
        element.style().refine(&root.to_gpui());
        if !self.disabled {
            let increment = cx.weak_entity();
            let decrement = cx.weak_entity();
            element = element
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        let Some(geometry) = this.geometry.get() else {
                            return;
                        };
                        this.focus.focus(window, cx);
                        if !geometry.thumb.contains(&event.position) {
                            let fraction = (event.position.x - geometry.start).as_f32()
                                / geometry.travel.max(px(1.)).as_f32();
                            this.change(this.range.value_at(fraction), window, cx);
                        }
                        this.drag = Some(Drag {
                            origin: event.position.x,
                            start: this.range.fraction(this.value()),
                        });
                        cx.notify();
                        cx.stop_propagation();
                    }),
                )
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let value = this.value();
                    let page =
                        ((this.range.max - this.range.min) / 10.).max(this.range.increment());
                    let next = match event.keystroke.key.as_str() {
                        "left" | "down" => value - this.range.increment(),
                        "right" | "up" => value + this.range.increment(),
                        "home" => this.range.min,
                        "end" => this.range.max,
                        "pageup" => value + page,
                        "pagedown" => value - page,
                        _ => return,
                    };
                    this.drag = None;
                    this.key_pressed = true;
                    this.change(next, window, cx);
                    cx.notify();
                    cx.stop_propagation();
                }))
                .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, cx| {
                    if this.key_pressed {
                        this.key_pressed = false;
                        cx.notify();
                    }
                }))
                .on_a11y_action(AccessibleAction::Increment, move |_, window, cx| {
                    let _ = increment.update(cx, |this, cx| {
                        this.change(this.value() + this.range.increment(), window, cx)
                    });
                })
                .on_a11y_action(AccessibleAction::Decrement, move |_, window, cx| {
                    let _ = decrement.update(cx, |this, cx| {
                        this.change(this.value() - this.range.increment(), window, cx)
                    });
                });
        }
        element.into_any_element()
    }
}

fn length(value: &Resolved<Pixels>, default: Pixels) -> Pixels {
    match value {
        Resolved::Value(value) => *value,
        Resolved::Clear => px(0.),
        _ => default,
    }
}
fn paint_part(bounds: Bounds<Pixels>, style: &ComputedStyle, window: &mut Window) {
    let native = style.to_gpui();
    let background = style
        .background_gradient
        .clone()
        .unwrap_or_else(|| match style.background {
            Resolved::Value(color) => color.into(),
            _ => transparent_black().into(),
        });
    let mut painted = quad(
        bounds,
        Corners::all(length(&style.radius, bounds.size.height / 2.)),
        background.opacity(match style.opacity {
            Resolved::Value(opacity) => opacity,
            _ => 1.,
        }),
        length(&style.border_width, px(0.)),
        native.border_color.unwrap_or_else(transparent_black),
        native.border_style.unwrap_or_default(),
    );
    if let Some(gradient) = &style.border_gradient {
        painted.border_gradient = gradient.clone();
    }
    window.paint_quad(painted);
}
