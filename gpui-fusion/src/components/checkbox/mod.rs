use crate::{
    components::motion::{GlassMotion, Spring},
    style::{
        ComputedStyle, MaterialMode, Part, Resolved, State, StateSelector, StylePatch, Styled,
    },
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{Styled as _, prelude::*, *};
use gpui_icons::GpuiIcons;
use std::{rc::Rc, time::Instant};

/// Mixed represents a partially selected group. User activation selects the group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckboxState {
    #[default]
    Unchecked,
    Checked,
    Mixed,
}
impl From<bool> for CheckboxState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CheckboxPart {
    Indicator,
    Mark,
    Label,
}
impl Part for CheckboxPart {
    fn name(&self) -> &str {
        match self {
            Self::Indicator => "indicator",
            Self::Mark => "mark",
            Self::Label => "label",
        }
    }
}
type Change = Rc<dyn Fn(CheckboxState, &mut Window, &mut App)>;

/// A labeled checkbox with theme-bound material and an application-controlled mixed state.
pub struct Checkbox {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    label: SharedString,
    show_label: bool,
    state: CheckboxState,
    disabled: bool,
    hovered: bool,
    pointer_pressed: bool,
    key_pressed: Option<String>,
    focus: FocusHandle,
    on_change: Option<Change>,
    _subscriptions: Vec<Subscription>,
    resolved: Option<(u64, u8, [ComputedStyle; 5])>,
    error: Option<StyleError>,
    selection: Spring,
    mixed: Spring,
    motion: GlassMotion,
    last_frame: Instant,
    animated: bool,
}
impl Checkbox {
    pub fn new(
        label: impl Into<SharedString>,
        state: impl Into<CheckboxState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.pointer_pressed = false;
                this.key_pressed = None;
                cx.notify();
            }),
        ];
        let state = state.into();
        let now = Instant::now();
        Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            variant: None,
            scopes: Vec::new(),
            label: label.into(),
            show_label: true,
            state,
            disabled: false,
            hovered: false,
            pointer_pressed: false,
            key_pressed: None,
            focus,
            on_change: None,
            _subscriptions: subscriptions,
            resolved: None,
            error: None,
            selection: Spring {
                value: f32::from(state != CheckboxState::Unchecked),
                velocity: 0.,
            },
            mixed: Spring {
                value: f32::from(state == CheckboxState::Mixed),
                velocity: 0.,
            },
            motion: GlassMotion::new(now),
            last_frame: now,
            animated: true,
        }
    }
    pub fn state(&self) -> CheckboxState {
        self.state
    }
    /// Updates the value without emitting a user activation.
    pub fn set_state(&mut self, state: impl Into<CheckboxState>, cx: &mut Context<Self>) {
        self.state = state.into();
        self.pointer_pressed = false;
        self.key_pressed = None;
        cx.notify();
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.pointer_pressed = false;
        self.key_pressed = None;
        self.hovered = false;
        cx.notify();
    }
    /// Hides the visible label while retaining its accessible name.
    pub fn label_visible(mut self, visible: bool) -> Self {
        self.show_label = visible;
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(CheckboxState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self.resolved = None;
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.resolved = None;
        self
    }
    pub fn set_variant(&mut self, variant: Option<String>, cx: &mut Context<Self>) {
        self.variant = variant;
        self.resolved = None;
        cx.notify();
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.resolved = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.state = if self.state == CheckboxState::Checked {
            CheckboxState::Unchecked
        } else {
            CheckboxState::Checked
        };
        cx.notify();
        if let Some(callback) = self.on_change.clone() {
            callback(self.state, window, cx);
        }
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<[ComputedStyle; 5], StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let flags = [
            (State::Selected, self.state != CheckboxState::Unchecked),
            (State::Indeterminate, self.state == CheckboxState::Mixed),
            (State::Disabled, self.disabled),
            (State::Hovered, !self.disabled && self.hovered),
            (
                State::Pressed,
                !self.disabled && (self.pointer_pressed || self.key_pressed.is_some()),
            ),
            (
                State::FocusVisible,
                !self.disabled && self.focus.is_focused(window) && window.last_input_was_keyboard(),
            ),
        ];
        let key = (u8::from(material == MaterialMode::Glass) << 6)
            | flags
                .iter()
                .enumerate()
                .fold(0, |key, (i, (_, active))| key | (u8::from(*active) << i));
        let revision = runtime.revision();
        if let Some((cached_revision, cached_key, styles)) = &self.resolved {
            if *cached_revision == revision && *cached_key == key {
                return Ok(styles.clone());
            }
        }
        let store = runtime.store();
        let mut context = ResolveContext::new(store.theme(), store.mode(), runtime.style_set())?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style
            .validate(&context, &["indicator", "mark", "label"])?;
        let recipe = runtime.styles().recipe("checkbox")?;
        let states = StateSelector(
            flags
                .into_iter()
                .filter_map(|(state, active)| active.then_some(state))
                .collect(),
        );
        let mut off = states.clone();
        off.0.remove(&State::Selected);
        off.0.remove(&State::Indeterminate);
        let mut on = states.clone();
        on.0.insert(State::Selected);
        let resolve = |states: &StateSelector, part| {
            recipe.resolve(
                self.variant.as_deref(),
                material,
                &self.style,
                &context,
                states,
                part,
            )
        };
        let styles = [
            resolve(&states, None)?,
            resolve(&off, Some("indicator"))?,
            resolve(&on, Some("indicator"))?,
            resolve(&states, Some("mark"))?,
            resolve(&states, Some("label"))?,
        ];
        self.resolved = Some((revision, key, styles.clone()));
        Ok(styles)
    }
}
impl Styled for Checkbox {
    type Part = CheckboxPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.resolved = None;
        &mut self.style
    }
}
impl Focusable for Checkbox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Checkbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [root, off, on, mark, label] = match self.resolve(window, cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Checkbox style error: {error}"))
                    .into_any_element();
            }
        };
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.032);
        self.last_frame = now;
        let selected = f32::from(self.state != CheckboxState::Unchecked);
        if self.state == CheckboxState::Unchecked {
            self.mixed.velocity = 0.;
        }
        let mixed = if self.state == CheckboxState::Unchecked {
            self.mixed.value
        } else {
            f32::from(self.state == CheckboxState::Mixed)
        };
        for (spring, target) in [(&mut self.selection, selected), (&mut self.mixed, mixed)] {
            if self.animated {
                spring.advance(target, dt, 24., 0.9);
            } else {
                spring.value = target;
                spring.velocity = 0.;
            }
            if spring.value != target || spring.velocity != 0. {
                window.request_animation_frame();
            }
        }
        let amount = self.selection.value.clamp(0., 1.);
        let mixed = self.mixed.value.clamp(0., 1.);
        let pressed = !self.disabled && (self.pointer_pressed || self.key_pressed.is_some());
        let (scale, moving) = self.motion.sample(
            if pressed {
                if off.material.is_some() { 1.12 } else { 0.94 }
            } else {
                1.
            },
            now,
            self.animated && !self.disabled,
        );
        if moving {
            window.request_animation_frame();
        }
        let pressure = ((scale - 1.) / 0.12).clamp(0., 1.);
        let active = if self.state == CheckboxState::Unchecked {
            &off
        } else {
            &on
        };
        let indicator_style = active.to_gpui();
        let material = active.material;
        let border_width = length(&active.border_width, 0.);
        let border_gradient = active.border_gradient.clone();
        let mut indicator = div()
            .relative()
            .flex_none()
            .debug_selector(|| "fusion-checkbox-indicator".into());
        indicator.style().refine(&indicator_style);
        indicator = indicator
            .bg(transparent_black())
            .border_color(transparent_black())
            .on_paint_before_children(move |bounds, style, window, _| {
                let dimensions = size(bounds.size.width * scale, bounds.size.height * scale);
                let bounds = Bounds::new(
                    bounds.center() - point(dimensions.width / 2., dimensions.height / 2.),
                    dimensions,
                );
                let corners = style
                    .corner_radii
                    .to_pixels(window.rem_size())
                    .map(|r| *r * scale);
                let background = mix(color(&off.background), color(&on.background), amount);
                let edge = mix(color(&off.border_color), color(&on.border_color), amount);
                let gradient =
                    off.background_gradient.is_some() || on.background_gradient.is_some();
                if let Some(mut glass) = material {
                    if !gradient {
                        glass.tint = background.composite_over(glass.tint);
                    }
                    glass.thickness *= 1. + pressure * 0.25;
                    glass.refraction *= 1. + pressure * 0.5;
                    glass.shadow.spread = bounds.size.height.min(bounds.size.width) * 0.15;
                    glass.shadow.offset.y = px(1.) * scale;
                    gpui_effects::paint_liquid_glass(bounds, corners, glass, window);
                }
                if gradient {
                    for (style, opacity) in [(&off, 1. - amount), (&on, amount)] {
                        let fill = style
                            .background_gradient
                            .clone()
                            .unwrap_or_else(|| color(&style.background).into());
                        if opacity > 0. {
                            window.paint_quad(quad(
                                bounds,
                                corners,
                                fill.opacity(opacity),
                                px(0.),
                                transparent_black(),
                                BorderStyle::Solid,
                            ));
                        }
                    }
                }
                let fill: Background = if material.is_some() || gradient {
                    transparent_black().into()
                } else {
                    background.into()
                };
                let mut border = quad(
                    bounds,
                    corners,
                    fill,
                    border_width,
                    edge,
                    indicator_style.border_style.unwrap_or_default(),
                );
                if let Some(gradient) = &border_gradient {
                    border.border_gradient = gradient.clone();
                }
                window.paint_quad(border);
            });
        let mut marks = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center();
        let mark_size = size(length(&mark.width, 14.), length(&mark.height, 14.));
        marks.style().refine(&mark.to_gpui());
        marks = marks.size_full();
        for (icon, opacity) in [
            (GpuiIcons::LucideCheck, amount * (1. - mixed)),
            (GpuiIcons::LucideMinus, amount * mixed),
        ] {
            if opacity > 0. {
                marks = marks.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(opacity)
                        .child(
                            svg()
                                .path(icon.path())
                                .text_color(color(&mark.foreground))
                                .w(mark_size.width)
                                .h(mark_size.height)
                                .with_transformation(Transformation::scale(size(
                                    scale * (0.8 + 0.2 * amount),
                                    scale * (0.8 + 0.2 * amount),
                                ))),
                        ),
                );
            }
        }
        indicator = indicator.child(marks);
        let weak = cx.weak_entity();
        let mut control = indicator
            .id("checkbox")
            .track_focus(&self.focus.clone().tab_stop(!self.disabled))
            .role(Role::CheckBox)
            .aria_toggled(match self.state {
                CheckboxState::Unchecked => Toggled::False,
                CheckboxState::Checked => Toggled::True,
                CheckboxState::Mixed => Toggled::Mixed,
            })
            .aria_disabled(self.disabled)
            .aria_label(self.label.clone());
        if !self.disabled {
            control = control
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    if !hovered {
                        this.pointer_pressed = false;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus.focus(window, cx);
                        this.pointer_pressed = true;
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pointer_pressed = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pointer_pressed = false;
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(|this, _, window, cx| this.activate(window, cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    this.key_pressed = (matches!(event.keystroke.key.as_str(), "space" | "enter")
                        && !event.keystroke.modifiers.modified())
                    .then(|| event.keystroke.key.clone());
                    cx.notify();
                }))
                .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, cx| {
                    if this.key_pressed.take().is_some() {
                        cx.notify();
                    }
                }))
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.activate(window, cx));
                });
        }
        let mut element = div()
            .debug_selector(|| "fusion-checkbox".into())
            .flex()
            .items_center()
            .child(control);
        if self.show_label {
            let mut text = div()
                .debug_selector(|| "fusion-checkbox-label".into())
                .child(self.label.clone());
            text.style().refine(&label.to_gpui());
            element = element.child(text);
        }
        element.style().refine(&root.to_gpui());
        element.into_any_element()
    }
}
fn color(value: &Resolved<Hsla>) -> Hsla {
    match value {
        Resolved::Value(color) => *color,
        _ => transparent_black(),
    }
}
fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let a: Rgba = a.into();
    let b: Rgba = b.into();
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .into()
}

fn length(value: &Resolved<Pixels>, default: f32) -> Pixels {
    match value {
        Resolved::Value(value) => *value,
        _ => px(default),
    }
}
