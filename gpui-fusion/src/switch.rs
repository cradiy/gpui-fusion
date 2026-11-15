use crate::{
    button_motion::Spring,
    runtime::ThemeRuntime,
    style::{ComputedStyle, Part, Resolved, State, StateSelector, StylePatch, Styled},
    theme::{ResolveContext, StyleError, ThemeScope},
};
use gpui::{Styled as _, prelude::*, *};
use std::{rc::Rc, time::Instant};

/// Foreground content centered on the thumb, above its material.
#[derive(Clone, Debug)]
pub enum SwitchContent {
    Text(SharedString),
    Icon(SharedString),
}

type Change = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug)]
pub enum SwitchPart {
    Thumb,
}
impl Part for SwitchPart {
    fn name(&self) -> &str {
        "thumb"
    }
}

struct Drag {
    origin: Pixels,
    start: f32,
    position: f32,
    moved: bool,
}

/// A binary control with a draggable thumb and a theme-defined track.
pub struct Switch {
    theme: Entity<ThemeRuntime>,
    style_set: String,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    style: StylePatch,
    checked: bool,
    content: Option<[SwitchContent; 2]>,
    disabled: bool,
    hovered: bool,
    key_pressed: bool,
    drag: Option<Drag>,
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
impl Switch {
    pub fn new(
        label: impl Into<SharedString>,
        checked: bool,
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
        Self {
            theme,
            style_set: style_set.into(),
            variant: None,
            scopes: Vec::new(),
            style: StylePatch::default(),
            checked,
            content: None,
            disabled: false,
            hovered: false,
            key_pressed: false,
            drag: None,
            focus,
            label: label.into(),
            on_change: None,
            _subscriptions: subscriptions,
            position: Spring {
                value: f32::from(checked),
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
    pub fn on_change(mut self, callback: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn content(mut self, unchecked: SwitchContent, checked: SwitchContent) -> Self {
        self.content = Some([unchecked, checked]);
        self
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
    /// Updates application state without emitting a user activation.
    pub fn set_checked(&mut self, checked: bool, cx: &mut Context<Self>) {
        let cancelled = self.drag.take().is_some();
        if self.checked != checked || cancelled {
            self.checked = checked;
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
    fn commit(&mut self, checked: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.disabled && self.checked != checked {
            self.checked = checked;
            cx.notify();
            if let Some(callback) = self.on_change.clone() {
                callback(checked, window, cx);
            }
        }
    }
    fn release(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            let checked = if drag.moved {
                drag.position >= 0.5
            } else {
                !self.checked
            };
            self.commit(checked, window, cx);
            cx.notify();
        }
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<[ComputedStyle; 4], StyleError> {
        let store = self.theme.read(cx).store();
        let revision = store.revision();
        let flags = [
            (State::Selected, self.checked),
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
            self.style.validate(&context, &["thumb"])?;
            self.validated_revision = Some(revision);
        }
        let states = StateSelector(
            flags
                .into_iter()
                .filter_map(|(state, active)| active.then_some(state))
                .collect(),
        );
        let mut off = states.clone();
        off.0.remove(&State::Selected);
        let mut on = states.clone();
        on.0.insert(State::Selected);
        let styles = [
            context.resolve_recipe(
                "switch",
                self.variant.as_deref(),
                &self.style,
                &states,
                None,
            )?,
            context.resolve_recipe(
                "switch",
                self.variant.as_deref(),
                &self.style,
                &states,
                Some("thumb"),
            )?,
            context.resolve_recipe("switch", self.variant.as_deref(), &self.style, &off, None)?,
            context.resolve_recipe("switch", self.variant.as_deref(), &self.style, &on, None)?,
        ];
        self.resolved = Some((revision, key, styles.clone()));
        Ok(styles)
    }
}
impl Styled for Switch {
    type Part = SwitchPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.validated_revision = None;
        &mut self.style
    }
}
impl Focusable for Switch {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Switch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [track, thumb, off, on] = match self.resolve(window, cx) {
            Ok(style) => {
                self.error = None;
                style
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Switch style error: {error}"))
                    .into_any_element();
            }
        };
        let target = self
            .drag
            .as_ref()
            .filter(|d| d.moved)
            .map_or(f32::from(self.checked), |d| d.position);
        let now = Instant::now();
        // An input received after a long idle starts a new transition at its current position.
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.032);
        self.last_frame = now;
        let travelling = (self.position.value - target).abs() > 0.02;
        let expand =
            f32::from(!self.disabled && (self.drag.is_some() || self.key_pressed || travelling));
        if self.animated {
            self.position.advance(target, dt, 24., 0.85);
            self.expansion.advance(expand, dt, 22., 0.7);
        } else {
            self.position = Spring {
                value: target,
                velocity: 0.,
            };
            self.expansion = Spring {
                value: 0.,
                velocity: 0.,
            };
        }
        if self.position.value != target
            || self.position.velocity != 0.
            || self.expansion.value != expand && self.animated
            || self.expansion.velocity != 0.
        {
            window.request_animation_frame();
        }
        let position = self.position.value.clamp(0., 1.);
        let expansion = self.expansion.value.clamp(0., 1.1);
        let thumb_style = thumb.to_gpui();
        let thumb_material = thumb.material;
        let fill = thumb
            .background_gradient
            .clone()
            .unwrap_or_else(|| match thumb.background {
                Resolved::Value(color) => color.into(),
                Resolved::Clear => transparent_black().into(),
                _ => rgb(0xffffff).into(),
            });
        let edge = thumb_style.border_color.unwrap_or_else(transparent_black);
        let edge_width = match thumb.border_width {
            Resolved::Value(w) => w,
            _ => px(0.),
        };
        let mut track_style = track.to_gpui();
        track_style.background = off.to_gpui().background;
        let on_fill = on
            .background_gradient
            .unwrap_or_else(|| match on.background {
                Resolved::Value(color) => color.into(),
                _ => transparent_black().into(),
            });
        let track_border = match track.border_width {
            Resolved::Value(w) => w,
            _ => px(0.),
        };
        let foreground = self.content.as_ref().map(|content| {
            let selected = content[usize::from(position >= 0.5)].clone();
            let thumb = thumb.clone();
            canvas(
                move |bounds, window, cx| {
                    let (height, width, _, center) = thumb_geometry(bounds, &thumb, position);
                    let color = match thumb.foreground {
                        Resolved::Value(color) => color,
                        _ => rgb(0x202126).into(),
                    };
                    let font_size = match thumb.font_size {
                        Resolved::Value(size) => size,
                        _ => px(11.),
                    };
                    let mut content = div()
                        .w(width)
                        .h(height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .text_color(color)
                        .text_size(font_size)
                        .child(match selected {
                            SwitchContent::Text(text) => div().child(text).into_any_element(),
                            SwitchContent::Icon(path) => svg()
                                .path(path)
                                .text_color(color)
                                .size(font_size * 1.25)
                                .into_any_element(),
                        })
                        .into_any_element();
                    content.layout_as_root(
                        size(
                            AvailableSpace::Definite(width),
                            AvailableSpace::Definite(height),
                        ),
                        window,
                        cx,
                    );
                    content.prepaint_at(center - point(width / 2., height / 2.), window, cx);
                    content
                },
                |_, mut content, window, cx| content.paint(window, cx),
            )
            .absolute()
            .inset_0()
        });
        let weak = cx.weak_entity();
        let events = cx.weak_entity();
        let mut element = div()
            .on_paint_before_children(move |bounds, style, window, _| {
                let inset = track_border
                    .min(bounds.size.height / 2.)
                    .min(bounds.size.width / 2.);
                let inner = Bounds::new(
                    bounds.origin + point(inset, inset),
                    size(
                        (bounds.size.width - inset * 2.).max(px(0.)),
                        (bounds.size.height - inset * 2.).max(px(0.)),
                    ),
                );
                let mut corners = style.corner_radii.to_pixels(window.rem_size());
                corners.top_left = (corners.top_left - inset).max(px(0.));
                corners.top_right = (corners.top_right - inset).max(px(0.));
                corners.bottom_left = (corners.bottom_left - inset).max(px(0.));
                corners.bottom_right = (corners.bottom_right - inset).max(px(0.));
                window.paint_quad(quad(
                    inner,
                    corners,
                    on_fill.opacity(position),
                    px(0.),
                    transparent_black(),
                    BorderStyle::Solid,
                ));
                let (height, width, travel, center) = thumb_geometry(bounds, &thumb, position);
                let scale = if thumb_material.is_some() {
                    1. + expansion * 0.55
                } else {
                    1.
                };
                let dimensions = size(width * scale, height * scale);
                let lens = Bounds::new(
                    center - point(dimensions.width / 2., dimensions.height / 2.),
                    dimensions,
                );
                let radius = match thumb.radius {
                    Resolved::Value(radius) => radius * scale,
                    _ => lens.size.height / 2.,
                };
                if let Some(mut material) = thumb_material {
                    let press = expansion.min(1.);
                    material.frost *= 1. - press;
                    material.refraction *= 0.1 + 0.9 * press;
                    let resting_bevel = height * 0.42;
                    material.thickness =
                        resting_bevel + (material.thickness - resting_bevel) * press;
                    material.shadow.strength *= press;
                    gpui_effects::paint_liquid_glass(lens, Corners::all(radius), material, window);
                } else {
                    let mut painted = quad(
                        lens,
                        Corners::all(radius),
                        fill.clone(),
                        Edges::all(edge_width),
                        edge,
                        thumb_style.border_style.unwrap_or_default(),
                    );
                    if let Some(gradient) = &thumb.border_gradient {
                        painted.border_gradient = gradient.clone();
                    }
                    window.paint_quad(painted);
                }
                let moving = events.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.pressed_button != Some(MouseButton::Left)
                    {
                        return;
                    }
                    let Some(moving) = moving.upgrade() else {
                        return;
                    };
                    if moving.read(cx).drag.is_none() {
                        return;
                    }
                    moving.update(cx, |this, cx| {
                        if let Some(drag) = &mut this.drag {
                            let dx = event.position.x - drag.origin;
                            if dx.abs() >= px(3.) {
                                drag.moved = true;
                            }
                            if drag.moved {
                                let next = (drag.start + dx.as_f32() / travel.max(px(1.)).as_f32())
                                    .clamp(0., 1.);
                                if drag.position != next {
                                    drag.position = next;
                                    cx.notify();
                                }
                            }
                        }
                    });
                });
                let release = events.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        let Some(release) = release.upgrade() else {
                            return;
                        };
                        if release.read(cx).drag.is_some() {
                            release.update(cx, |this, cx| this.release(window, cx));
                        }
                    }
                });
            })
            .id("switch")
            .debug_selector(|| "fusion-switch".into())
            .track_focus(&self.focus)
            .tab_stop(!self.disabled)
            .role(Role::Switch)
            .aria_toggled(self.checked.into())
            .aria_disabled(self.disabled)
            .aria_label(self.label.clone())
            .relative()
            .flex_shrink_0();
        element = element.children(foreground);
        element.style().refine(&track_style);
        if !self.disabled {
            element = element
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        this.focus.focus(window, cx);
                        this.drag = Some(Drag {
                            origin: event.position.x,
                            start: this.position.value,
                            position: this.position.value,
                            moved: false,
                        });
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(|this, event, window, cx| {
                    if !matches!(event, ClickEvent::Mouse(_)) {
                        this.commit(!this.checked, window, cx);
                    }
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "space" | "enter")
                        && !event.keystroke.modifiers.modified()
                    {
                        this.key_pressed = true;
                        cx.notify();
                    }
                }))
                .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, cx| {
                    this.key_pressed = false;
                    cx.notify();
                }))
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.commit(!this.checked, window, cx));
                });
        }
        element.into_any_element()
    }
}

fn thumb_geometry(
    bounds: Bounds<Pixels>,
    thumb: &ComputedStyle,
    position: f32,
) -> (Pixels, Pixels, Pixels, Point<Pixels>) {
    let padding = bounds.size.height * 0.08;
    let height = match thumb.height {
        Resolved::Value(height) => height.min(bounds.size.height - padding * 2.),
        _ => bounds.size.height - padding * 2.,
    };
    let width = match thumb.width {
        Resolved::Value(width) => width,
        _ if thumb.material.is_some() => height * 1.58,
        _ => height,
    }
    .min(bounds.size.width - padding * 2.)
    .max(px(0.));
    let travel = (bounds.size.width - width - padding * 2.).max(px(0.));
    let center = point(
        bounds.left() + padding + width / 2. + travel * position,
        bounds.center().y,
    );
    (height, width, travel, center)
}
