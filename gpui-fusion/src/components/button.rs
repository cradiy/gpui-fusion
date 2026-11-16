mod label;
mod motion;

use crate::{
    style::{MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::material_binding::MaterialBinding,
    theme::runtime::ThemeRuntime,
    theme::{ResolveContext, StyleError, ThemeScope},
};
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Refineable, Render, SharedString, StatefulInteractiveElement, Styled as _, Subscription,
    Window, div, px,
};
use std::rc::Rc;

type Activation = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Copy, Debug)]
pub enum ButtonPart {
    Label,
    Icon,
}
impl Part for ButtonPart {
    fn name(&self) -> &str {
        match self {
            Self::Label => "label",
            Self::Icon => "icon",
        }
    }
}

/// A persistent, focusable action control using the `button` theme recipe.
pub struct Button {
    theme: Entity<ThemeRuntime>,
    material_binding: MaterialBinding,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    style: StylePatch,
    label: SharedString,
    label_transition: Option<label::LabelTransition>,
    expanded: Option<bool>,
    icon: Option<SharedString>,
    disabled: bool,
    hovered: bool,
    pointer_pressed: bool,
    key_pressed: Option<String>,
    focus: FocusHandle,
    on_click: Option<Activation>,
    error: Option<StyleError>,
    _subscriptions: Vec<Subscription>,
    validated_revision: Option<u64>,
    transition: Option<super::motion::MaterialTransition>,
    transition_duration: std::time::Duration,
    glass_motion: super::motion::GlassMotion,
    button_motion: motion::ButtonMotion,
    pointer: Option<gpui::Point<gpui::Pixels>>,
    cursor_light: motion::CursorLight,
    light_active: bool,
    light_direction: gpui::Point<f32>,
    border_flow_started: Option<std::time::Instant>,
}

impl Button {
    pub fn new(
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |this, theme, cx| {
                if this.validated_revision != Some(theme.read(cx).revision()) {
                    cx.notify();
                }
            }),
            cx.on_focus(&focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.key_pressed = None;
                this.pointer_pressed = false;
                cx.notify();
            }),
        ];
        Self {
            theme,
            material_binding: MaterialBinding::new(cx),
            variant: None,
            scopes: Vec::new(),
            style: StylePatch::default(),
            label: label.into(),
            label_transition: None,
            expanded: None,
            icon: None,
            disabled: false,
            hovered: false,
            pointer_pressed: false,
            key_pressed: None,
            focus,
            on_click: None,
            error: None,
            _subscriptions: subscriptions,
            validated_revision: None,
            transition: None,
            transition_duration: std::time::Duration::from_millis(160),
            glass_motion: super::motion::GlassMotion::new(std::time::Instant::now()),
            button_motion: motion::ButtonMotion::new(std::time::Instant::now()),
            pointer: None,
            cursor_light: motion::CursorLight::new(std::time::Instant::now()),
            light_active: false,
            light_direction: gpui::point(0., -1.),
            border_flow_started: None,
        }
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
    /// Changes the label with a crossfade and intrinsic-width transition.
    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        let label = label.into();
        if label == self.label {
            return;
        }
        let now = std::time::Instant::now();
        if self.transition_duration.is_zero() {
            self.label_transition = None;
        } else if let Some(transition) = &mut self.label_transition {
            transition.retarget(label.clone(), now, self.transition_duration);
        } else {
            self.label_transition = Some(label::LabelTransition::new(
                self.label.clone(),
                label.clone(),
                now,
            ));
        }
        self.label = label;
        cx.notify();
    }
    pub(crate) fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.expanded = Some(expanded);
        cx.notify();
    }
    /// Zero duration disables label, material, and elastic transitions.
    pub fn transition_duration(mut self, duration: std::time::Duration) -> Self {
        self.transition_duration = duration;
        self
    }
    /// Updates the duration for label, material, and elastic transitions.
    pub fn set_transition_duration(
        &mut self,
        duration: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        self.transition_duration = duration;
        if duration.is_zero() {
            self.label_transition = None;
        }
        cx.notify();
    }
    /// Fixes the material and unsubscribes from application material changes.
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material_binding.fix(mode);
        self
    }
    /// None follows the application's material; Some fixes this component's material.
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material_binding.set(mode, cx);
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self
    }
    pub fn icon(mut self, asset: impl Into<SharedString>) -> Self {
        self.icon = Some(asset.into());
        self
    }
    pub fn on_click(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            self.pointer_pressed = false;
            self.key_pressed = None;
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

    pub(crate) fn transition_content(
        &mut self,
        window: &mut Window,
        cx: &App,
    ) -> Result<gpui::AnyElement, StyleError> {
        self.content(self.label.clone(), true, window, cx)
    }

    pub(crate) fn settled_content(
        &mut self,
        label: SharedString,
        window: &mut Window,
        cx: &App,
    ) -> Result<gpui::AnyElement, StyleError> {
        self.content(label, false, window, cx)
    }

    fn content(
        &mut self,
        text_label: SharedString,
        animated: bool,
        window: &mut Window,
        cx: &App,
    ) -> Result<gpui::AnyElement, StyleError> {
        let [root, label, icon] = self.resolve(window, cx)?;
        let mut text = div();
        text = if let Some(transition) = self.label_transition.as_ref().filter(|t| {
            animated && !t.finished(std::time::Instant::now(), self.transition_duration)
        }) {
            let mut style = window.text_style();
            style.refine(&root.to_gpui().text);
            style.refine(&label.to_gpui().text);
            text.overflow_hidden().child(transition.render(
                style,
                window,
                self.transition_duration,
                1.,
            ))
        } else {
            text.child(text_label)
        };
        text.style().refine(&label.to_gpui());
        let mut row = div().flex().items_center().justify_center();
        row.style().refine(&root.to_gpui());
        row = row
            .bg(gpui::transparent_black())
            .border_color(gpui::transparent_black());
        let gap = match root.gap {
            crate::style::Resolved::Value(gap) => gap,
            _ => px(0.),
        };
        let mut content = div().flex().items_center().gap(gap);
        if let Some(path) = &self.icon {
            let mut graphic = gpui::svg().path(path.clone()).size(px(16.)).text_color(
                root.to_gpui()
                    .text
                    .color
                    .unwrap_or(window.text_style().color),
            );
            graphic.style().refine(&icon.to_gpui());
            content = content.child(graphic);
        }
        Ok(row.child(content.child(text)).into_any_element())
    }

    pub(crate) fn surface_style(
        &mut self,
        window: &Window,
        cx: &App,
    ) -> Result<crate::style::ComputedStyle, StyleError> {
        let [surface, _, _] = self.resolve(window, cx)?;
        Ok(surface)
    }

    pub(crate) fn surface_scale(&mut self, window: &mut Window) -> [f32; 2] {
        self.sample_glass_scale(true, 1.05, window)
    }

    fn sample_glass_scale(
        &mut self,
        glass_enabled: bool,
        pressed_scale: f32,
        window: &mut Window,
    ) -> [f32; 2] {
        let pressed = self.pointer_pressed || self.key_pressed.is_some();
        let scale_target = if !glass_enabled || self.disabled {
            1.
        } else if pressed {
            pressed_scale
        } else if self.hovered {
            1.03
        } else {
            1.
        };
        let (scale, moving) = self.glass_motion.sample(
            scale_target,
            std::time::Instant::now(),
            !self.transition_duration.is_zero(),
        );
        if moving && glass_enabled {
            window.request_animation_frame();
        }
        [scale; 2]
    }

    fn activate(&self, window: &mut Window, cx: &mut App) {
        if !self.disabled {
            if let Some(callback) = &self.on_click {
                callback(window, cx);
            }
        }
    }

    fn resolve(
        &mut self,
        window: &Window,
        cx: &App,
    ) -> Result<[crate::style::ComputedStyle; 3], StyleError> {
        let runtime = self.theme.read(cx);
        let store = runtime.store();
        let recipe = runtime.styles().recipe("button")?;
        let material = if matches!(self.variant.as_deref(), Some("text" | "link")) {
            MaterialMode::Plain
        } else {
            self.material_binding.mode(cx)
        };
        let mut context = ResolveContext::new(store.theme(), store.mode(), runtime.style_set())?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        if self.validated_revision != Some(runtime.revision()) {
            self.style.validate(&context, &["label", "icon"])?;
            if recipe
                .parts
                .iter()
                .any(|part| part != "label" && part != "icon")
            {
                return Err(StyleError::new("button.parts", "unsupported button part"));
            }
            self.validated_revision = Some(runtime.revision());
        }
        let mut states = StateSelector::default();
        if self.disabled {
            states.0.insert(State::Disabled);
        } else {
            if self.hovered {
                states.0.insert(State::Hovered);
            }
            if self.pointer_pressed || self.key_pressed.is_some() {
                states.0.insert(State::Pressed);
            }
            if self.focus.is_focused(window) && window.last_input_was_keyboard() {
                states.0.insert(State::FocusVisible);
            }
        }
        let resolve = |part| {
            recipe.resolve(
                self.variant.as_deref(),
                material,
                &self.style,
                &context,
                &states,
                part,
            )
        };
        Ok([
            resolve(None)?,
            resolve(Some("label"))?,
            resolve(Some("icon"))?,
        ])
    }
}

impl Styled for Button {
    type Part = ButtonPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.validated_revision = None;
        &mut self.style
    }
}
impl Focusable for Button {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Button {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let computed = match self.resolve(window, cx) {
            Ok(style) => {
                self.error = None;
                style
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Button style error: {error}"))
                    .into_any_element();
            }
        };
        let [mut root, label, icon] = computed;
        let glass_enabled = root.material.is_some();
        let (motion, moving) = self.button_motion.sample(
            glass_enabled && !self.disabled && (self.pointer_pressed || self.key_pressed.is_some()),
            glass_enabled && !self.disabled && self.hovered,
            std::time::Instant::now(),
            glass_enabled && !self.disabled && !self.transition_duration.is_zero(),
        );
        if moving {
            window.request_animation_frame();
        }
        let flow = match root.border_motion {
            Some(crate::style::BorderMotion::Flow { seconds_per_cycle })
                if !self.disabled
                    && !self.transition_duration.is_zero()
                    && matches!(root.border_width, crate::style::Resolved::Value(width) if width > px(0.)) =>
            {
                Some(seconds_per_cycle)
            }
            _ => None,
        };
        if let (Some(period), Some(gradient)) = (flow, root.border_gradient.clone()) {
            let now = std::time::Instant::now();
            let started = *self.border_flow_started.get_or_insert(now);
            root.border_gradient =
                Some(gradient.phase(now.duration_since(started).as_secs_f32() / period));
            window.request_animation_frame();
        } else {
            self.border_flow_started = None;
        }
        let mut content = div();
        content.style().refine(&label.to_gpui());
        if self.label_transition.as_ref().is_some_and(|transition| {
            transition.finished(std::time::Instant::now(), self.transition_duration)
        }) {
            self.label_transition = None;
        }
        content = if let Some(transition) = &self.label_transition {
            let mut text_style = window.text_style();
            text_style.refine(&root.to_gpui().text);
            text_style.refine(&label.to_gpui().text);
            content.child(transition.render(
                text_style,
                window,
                self.transition_duration,
                motion.label_scale,
            ))
        } else if glass_enabled {
            let mut text_style = window.text_style();
            text_style.refine(&root.to_gpui().text);
            text_style.refine(&label.to_gpui().text);
            content.child(label::scaled_label(
                self.label.clone(),
                text_style,
                motion.label_scale,
                window,
            ))
        } else {
            content.child(self.label.clone())
        };
        let glass = root.material.map(|target| {
            let now = std::time::Instant::now();
            let transition = self
                .transition
                .get_or_insert_with(|| super::motion::MaterialTransition::new(target, now));
            let (material, active) = transition.sample(target, now, self.transition_duration);
            if active {
                window.request_animation_frame();
            }
            material
        });
        if glass.is_none() {
            self.transition = None;
        }
        let cursor_light_enabled = !self.disabled
            && glass.is_some_and(|material| {
                material.optics.environment_reflection > 0. || material.optics.rim_strength > 0.
            });
        let (light_amount, relighting) = self.cursor_light.sample(
            cursor_light_enabled && self.light_active && window.is_window_hovered(),
            std::time::Instant::now(),
            glass_enabled && !self.disabled && !self.transition_duration.is_zero(),
        );
        if relighting {
            window.request_animation_frame();
        }
        let light_direction = self.light_direction;
        let light_owner = cx.weak_entity();
        let paint_style = root.to_gpui();
        let glass_fill = root.background_gradient.clone();
        let glass_tint = match root.background {
            crate::style::Resolved::Value(color) => color,
            _ => gpui::transparent_black(),
        };
        let glass_border = match root.border_color {
            crate::style::Resolved::Value(color) => color,
            _ => gpui::transparent_black(),
        };
        let glass_width = match root.border_width {
            crate::style::Resolved::Value(width) => width,
            _ => px(0.),
        };
        let glass_border_style = paint_style.border_style.unwrap_or_default();
        let glass_gradient = root.border_gradient.clone();
        let mut element = div()
            .on_paint_before_children(move |bounds, style, window, _| {
                if cursor_light_enabled {
                    let owner = light_owner.clone();
                    window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture {
                            let _ = owner.update(cx, |this, cx| {
                                if this.pointer != Some(event.position) || !this.light_active {
                                    this.pointer = Some(event.position);
                                    this.light_active = true;
                                    let delta = event.position - bounds.center();
                                    let distance = delta.x.as_f32().hypot(delta.y.as_f32());
                                    if distance > 0.5 {
                                        this.light_direction = gpui::point(
                                            delta.x.as_f32() / distance,
                                            delta.y.as_f32() / distance,
                                        );
                                    }
                                    cx.notify();
                                }
                            });
                        }
                    });
                    let owner = light_owner.clone();
                    window.on_mouse_event(move |_: &gpui::MouseExitEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture {
                            let _ = owner.update(cx, |this, cx| {
                                if this.light_active {
                                    this.light_active = false;
                                    cx.notify();
                                }
                            });
                        }
                    });
                }
                if let Some(mut glass) = glass {
                    let grown = motion.lens_size(bounds.size);
                    let origin = bounds.center() - gpui::point(grown.width / 2., grown.height / 2.);
                    let lens = gpui::Bounds::new(origin, grown);
                    let mut corners = style.corner_radii.to_pixels(window.rem_size());
                    let corner_scale = grown.height / bounds.size.height.max(px(1.));
                    corners.top_left *= corner_scale;
                    corners.top_right *= corner_scale;
                    corners.bottom_left *= corner_scale;
                    corners.bottom_right *= corner_scale;
                    glass.optics.light_direction = light_direction;
                    if light_amount > 0. && window.is_window_hovered() {
                        let delta = window.mouse_position() - bounds.center();
                        let distance = delta.x.as_f32().hypot(delta.y.as_f32());
                        if distance > 0.5 {
                            glass.optics.light_direction = gpui::point(
                                delta.x.as_f32() / distance,
                                delta.y.as_f32() / distance,
                            );
                        }
                    }
                    glass.optics.directional_light = light_amount;
                    if glass_fill.is_none() {
                        glass.tint = glass_tint.composite_over(glass.tint);
                    }
                    gpui_effects::paint_liquid_glass(lens, corners, glass, window);
                    let mut overlay = gpui::quad(
                        lens,
                        corners,
                        glass_fill
                            .clone()
                            .unwrap_or_else(|| gpui::transparent_black().into()),
                        glass_width,
                        glass_border,
                        glass_border_style,
                    );
                    if let Some(gradient) = &glass_gradient {
                        overlay.border_gradient = gradient.clone();
                    }
                    window.paint_quad(overlay);
                }
            })
            .id("button")
            .debug_selector(|| "fusion-button".into())
            .track_focus(&self.focus.clone().tab_stop(!self.disabled))
            .aria_disabled(self.disabled)
            .role(gpui::Role::Button)
            .aria_label(self.label.clone())
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .child({
                let gap = match root.gap {
                    crate::style::Resolved::Value(gap) => gap,
                    _ => px(0.),
                };
                let mut row = div().flex().items_center().gap(gap);
                if let Some(path) = &self.icon {
                    let mut graphic = gpui::svg()
                        .path(path.clone())
                        .size(px(16.))
                        .text_color(
                            root.to_gpui()
                                .text
                                .color
                                .unwrap_or(window.text_style().color),
                        )
                        .with_transformation(gpui::Transformation::scale(gpui::size(
                            motion.label_scale,
                            motion.label_scale,
                        )));
                    graphic.style().refine(&icon.to_gpui());
                    row = row.child(graphic);
                }
                row.child(content)
            });
        if let Some(expanded) = self.expanded {
            element = element.aria_expanded(expanded);
        }
        element.style().refine(&paint_style);
        if glass_enabled {
            // Solid fill is the lens tint; gradients and borders share its animated bounds.
            element.style().background = Some(gpui::transparent_black().into());
            element.style().border_color = Some(gpui::transparent_black());
            element.style().border_gradient = Some(gpui::BorderGradient::default());
        }
        let weak = cx.weak_entity();
        if !self.disabled {
            element = element
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    if !hovered {
                        this.pointer_pressed = false;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus.focus(window, cx);
                        this.pointer_pressed = true;
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pointer_pressed = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pointer_pressed = false;
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(|this, _, window, cx| this.activate(window, cx)))
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                    let next = (matches!(event.keystroke.key.as_str(), "space" | "enter")
                        && !event.keystroke.modifiers.modified())
                    .then(|| event.keystroke.key.clone());
                    if this.key_pressed != next {
                        this.key_pressed = next;
                        cx.notify();
                    }
                }))
                .on_key_up(cx.listener(|this, _: &gpui::KeyUpEvent, _, cx| {
                    if this.key_pressed.take().is_some() {
                        cx.notify();
                    }
                }))
                .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.activate(window, cx));
                });
        }
        element.into_any_element()
    }
}
