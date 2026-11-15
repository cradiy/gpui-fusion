use crate::{
    runtime::ThemeRuntime,
    style::{Part, State, StateSelector, StylePatch, Styled},
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
    style_set: String,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    style: StylePatch,
    label: SharedString,
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
    transition: Option<crate::button_motion::MaterialTransition>,
    transition_duration: std::time::Duration,
    glass_motion: crate::button_motion::GlassMotion,
    pointer: Option<gpui::Point<gpui::Pixels>>,
    border_flow_started: Option<std::time::Instant>,
}

impl Button {
    pub fn new(
        label: impl Into<SharedString>,
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
                this.key_pressed = None;
                this.pointer_pressed = false;
                cx.notify();
            }),
        ];
        Self {
            theme,
            style_set: style_set.into(),
            variant: None,
            scopes: Vec::new(),
            style: StylePatch::default(),
            label: label.into(),
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
            glass_motion: crate::button_motion::GlassMotion::new(std::time::Instant::now()),
            pointer: None,
            border_flow_started: None,
        }
    }

    /// Zero duration disables material transitions and elastic motion.
    pub fn transition_duration(mut self, duration: std::time::Duration) -> Self {
        self.transition_duration = duration;
        self
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
        let store = self.theme.read(cx).store();
        let mut context = ResolveContext::new(store.theme(), store.mode(), &self.style_set)?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        if self.validated_revision != Some(store.revision()) {
            self.style.validate(&context, &["label", "icon"])?;
            let selected_style = self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.style_set.as_deref())
                .unwrap_or(&self.style_set);
            let mut current = Some(selected_style);
            while let Some(name) = current {
                let set = &store.theme().style_sets[name];
                if let Some(recipe) = set.recipes.get("button") {
                    if recipe
                        .parts
                        .iter()
                        .any(|part| part != "label" && part != "icon")
                    {
                        return Err(StyleError::new("button.parts", "unsupported button part"));
                    }
                }
                current = set.extends.as_deref();
            }
            self.validated_revision = Some(store.revision());
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
            context.resolve_recipe(
                "button",
                self.variant.as_deref(),
                &self.style,
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
        content = content.child(self.label.clone());
        let glass = root.material.map(|target| {
            let now = std::time::Instant::now();
            let transition = self
                .transition
                .get_or_insert_with(|| crate::button_motion::MaterialTransition::new(target, now));
            let (material, active) = transition.sample(target, now, self.transition_duration);
            if active {
                window.request_animation_frame();
            }
            material
        });
        if glass.is_none() {
            self.transition = None;
        }
        let glass_enabled = glass.is_some();
        let pressed = self.pointer_pressed || self.key_pressed.is_some();
        let scale_target = if !glass_enabled || self.disabled {
            [1., 1.]
        } else if pressed {
            [1.08, 1.08]
        } else if self.hovered {
            [1.04, 1.08]
        } else {
            [1., 1.]
        };
        let (scale, moving) = self.glass_motion.sample(
            scale_target,
            std::time::Instant::now(),
            !self.transition_duration.is_zero(),
        );
        if moving && glass_enabled {
            window.request_animation_frame();
        }
        let pointer = if self.hovered { self.pointer } else { None };
        let paint_style = root.to_gpui();
        let glass_fill =
            root.background_gradient
                .clone()
                .unwrap_or_else(|| match root.background {
                    crate::style::Resolved::Value(color) => color.into(),
                    _ => gpui::transparent_black().into(),
                });
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
                if let Some(mut glass) = glass {
                    let grown =
                        gpui::size(bounds.size.width * scale[0], bounds.size.height * scale[1]);
                    let origin = bounds.center() - gpui::point(grown.width / 2., grown.height / 2.);
                    let lens = gpui::Bounds::new(origin, grown);
                    let mut corners = style.corner_radii.to_pixels(window.rem_size());
                    let corner_scale = scale[0].min(scale[1]);
                    corners.top_left *= corner_scale;
                    corners.top_right *= corner_scale;
                    corners.bottom_left *= corner_scale;
                    corners.bottom_right *= corner_scale;
                    if let Some(pointer) = pointer {
                        let delta = pointer - bounds.center();
                        glass.optics.light_direction = gpui::point(
                            (delta.x / bounds.size.width.max(px(1.))).clamp(-1., 1.),
                            (delta.y / bounds.size.height.max(px(1.))).clamp(-1., 1.),
                        );
                        glass.optics.directional_light = 0.65;
                    }
                    gpui_effects::paint_liquid_glass(lens, corners, glass, window);
                    let mut overlay = gpui::quad(
                        lens,
                        corners,
                        glass_fill.clone(),
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
            .track_focus(&self.focus)
            .tab_stop(!self.disabled)
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
                    let mut graphic = gpui::svg().path(path.clone()).size(px(16.));
                    graphic.style().refine(&icon.to_gpui());
                    row = row.child(graphic);
                }
                row.child(content)
            });
        element.style().refine(&paint_style);
        if glass_enabled {
            // Fill and border are composited after the backdrop lens at its animated bounds.
            element.style().background = Some(gpui::transparent_black().into());
            element.style().border_color = Some(gpui::transparent_black());
            element.style().border_gradient = Some(gpui::BorderGradient::default());
        }
        if glass_enabled {
            element =
                element.on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                    if this.hovered {
                        this.pointer = Some(event.position);
                        cx.notify();
                    }
                }));
        }
        let weak = cx.weak_entity();
        if !self.disabled {
            element = element
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    if !hovered {
                        this.pointer_pressed = false;
                        this.pointer = None;
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
