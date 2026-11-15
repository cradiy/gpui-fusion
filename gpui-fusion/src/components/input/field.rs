use super::{InputMode, InputState, scrollbar::Scrollbar};
use crate::{
    style::{
        ComputedStyle, MaterialMode, Part, Resolved, State, StateSelector, StylePatch, Styled,
    },
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{Styled as _, prelude::*, *};
use std::rc::Rc;

#[path = "adornment.rs"]
mod adornment;
use adornment::InputAction;
pub use adornment::{InputIcon, InputSide};

/// Semantic paint targets; editing content always stays above the material surface.
#[derive(Clone, Copy, Debug)]
pub enum InputPart {
    Placeholder,
    Caret,
    Selection,
    Scrollbar,
    ActionButton,
    ResizeHandle,
}
impl Part for InputPart {
    fn name(&self) -> &str {
        match self {
            Self::Placeholder => "placeholder",
            Self::Caret => "caret",
            Self::Selection => "selection",
            Self::Scrollbar => "scrollbar",
            Self::ActionButton => "action_button",
            Self::ResizeHandle => "resize_handle",
        }
    }
}

type EditCallback = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

enum Adornment {
    Custom(Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>),
    Icon(InputIcon),
}

/// A themed surface around a persistent editor. Keep the editor entity when changing themes.
pub struct Input {
    editor: Entity<InputState>,
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    rows: usize,
    password_toggle: bool,
    clearable: bool,
    clear_focus: FocusHandle,
    leading_focus: FocusHandle,
    trailing_focus: FocusHandle,
    visibility_focus: FocusHandle,
    hovered: bool,
    editor_state: (InputMode, bool, bool, bool, bool),
    resize: super::resize::ResizeState,
    blur_on_click_outside: bool,
    prefix: Option<Adornment>,
    suffix: Option<Adornment>,
    _subscriptions: Vec<Subscription>,
    on_change: Option<EditCallback>,
    on_submit: Option<EditCallback>,
    resolved: Option<(u64, MaterialMode, StateSelector, [ComputedStyle; 9])>,
    error: Option<StyleError>,
}

impl Input {
    pub fn new(editor: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        super::init(cx);
        let theme = ThemeRuntime::global(cx);
        let focus = editor.focus_handle(cx);
        let visibility_focus = cx.focus_handle();
        let clear_focus = cx.focus_handle();
        let leading_focus = cx.focus_handle();
        let trailing_focus = cx.focus_handle();
        let editor_state = (
            editor.read(cx).mode,
            editor.read(cx).disabled,
            editor.read(cx).is_password_visible(),
            editor.read(cx).read_only,
            editor.read(cx).value().is_empty(),
        );
        let subscriptions = vec![
            cx.observe(&theme, |this, theme, cx| {
                if this.resolved.as_ref().map(|cache| cache.0) != Some(theme.read(cx).revision()) {
                    cx.notify();
                }
            }),
            // State changes may affect focusability, mode, or semantic styling.
            cx.observe(editor, |this, editor, cx| {
                let editor = editor.read(cx);
                let state = (
                    editor.mode,
                    editor.disabled,
                    editor.is_password_visible(),
                    editor.read_only,
                    editor.value().is_empty(),
                );
                if this.editor_state != state {
                    this.editor_state = state;
                    cx.notify();
                }
            }),
            cx.subscribe_in(editor, window, |this, _, event, window, cx| {
                let callback = match event {
                    super::InputEvent::Change(_) => &this.on_change,
                    super::InputEvent::Submit(_) => &this.on_submit,
                };
                if let Some(callback) = callback {
                    callback(event.text().clone(), window, cx);
                }
            }),
            cx.on_focus(&clear_focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&clear_focus, window, |_, _, cx| cx.notify()),
            cx.on_focus(&trailing_focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&trailing_focus, window, |_, _, cx| cx.notify()),
            cx.on_focus(&leading_focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&leading_focus, window, |_, _, cx| cx.notify()),
            cx.on_focus(&visibility_focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&visibility_focus, window, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |_, _, cx| cx.notify()),
        ];
        Self {
            editor: editor.clone(),
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            rows: 4,
            password_toggle: true,
            clearable: false,
            clear_focus,
            leading_focus,
            trailing_focus,
            visibility_focus,
            hovered: false,
            editor_state,
            resize: Default::default(),
            blur_on_click_outside: true,
            prefix: None,
            suffix: None,
            _subscriptions: subscriptions,
            on_change: None,
            on_submit: None,
            resolved: None,
            error: None,
        }
    }
    /// Enables dragging both dimensions of multiline fields. Disabled by default.
    pub fn resizable(self, enabled: bool) -> Self {
        self.resize(if enabled {
            super::InputResize::Both
        } else {
            super::InputResize::Disabled
        })
    }
    pub fn resize(mut self, axes: super::InputResize) -> Self {
        self.resize.axes = axes;
        self
    }
    /// Sets the outer size limits applied during dragging.
    pub fn resize_limits(mut self, min: Size<Pixels>, max: Size<Pixels>) -> Self {
        self.resize.limits(min, max);
        self
    }
    pub fn set_resizable(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.resize.axes = if enabled {
            super::InputResize::Both
        } else {
            super::InputResize::Disabled
        };
        self.resize.dragging = None;
        cx.notify();
    }
    pub fn reset_size(&mut self, cx: &mut Context<Self>) {
        self.resize.dimensions = None;
        self.resize.dragging = None;
        cx.notify();
    }
    pub fn state(&self) -> &Entity<InputState> {
        &self.editor
    }
    /// Visible rows for multiline input. An explicit height takes precedence.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }
    pub fn set_rows(&mut self, rows: usize, cx: &mut Context<Self>) {
        self.rows = rows.max(1);
        cx.notify();
    }
    /// Whether password fields include a built-in visibility control. Enabled by default.
    pub fn password_toggle(mut self, enabled: bool) -> Self {
        self.password_toggle = enabled;
        self
    }
    fn toggle_password(&mut self, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor.set_password_visible(!editor.is_password_visible(), cx)
        });
    }
    pub fn blur_on_click_outside(mut self, enabled: bool) -> Self {
        self.blur_on_click_outside = enabled;
        self
    }
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.resolved = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    /// Builds content outside the editable viewport, within the field's focus boundary.
    pub fn prefix(
        mut self,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.prefix = Some(Adornment::Custom(Rc::new(render)));
        self
    }
    pub fn suffix(
        mut self,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.suffix = Some(Adornment::Custom(Rc::new(render)));
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn on_submit(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(callback));
        self
    }
    fn resolve(&self, states: &StateSelector, cx: &App) -> Result<[ComputedStyle; 9], StyleError> {
        let runtime = self.theme.read(cx);
        let recipe = runtime.styles().recipe("input")?;
        const PARTS: &[&str] = &[
            "placeholder",
            "caret",
            "selection",
            "scrollbar",
            "action_button",
            "resize_handle",
        ];
        if recipe
            .parts
            .iter()
            .any(|part| !PARTS.contains(&part.as_str()))
        {
            return Err(StyleError::new("input.parts", "unsupported input part"));
        }
        let mut context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style.validate(&context, PARTS)?;
        let resolve = |part| {
            recipe.resolve(
                self.variant.as_deref(),
                self.material.mode(cx),
                &self.style,
                &context,
                states,
                part,
            )
        };
        let scroll = |state: Option<State>| {
            let mut states = states.clone();
            states.0.remove(&State::Hovered);
            if let Some(state) = state {
                states.0.insert(state);
            }
            recipe.resolve(
                self.variant.as_deref(),
                self.material.mode(cx),
                &self.style,
                &context,
                &states,
                Some("scrollbar"),
            )
        };
        Ok([
            resolve(None)?,
            resolve(Some("placeholder"))?,
            resolve(Some("caret"))?,
            resolve(Some("selection"))?,
            scroll(None)?,
            scroll(Some(State::Hovered))?,
            scroll(Some(State::Pressed))?,
            resolve(Some("action_button"))?,
            resolve(Some("resize_handle"))?,
        ])
    }
}
impl Styled for Input {
    type Part = InputPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.resolved = None;
        &mut self.style
    }
}
impl Focusable for Input {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}
impl Render for Input {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let disabled = editor.disabled;
        let multiline = editor.mode == InputMode::Multiline;
        let password = editor.mode == InputMode::Password;
        let has_value = !editor.value().is_empty();
        let read_only = editor.read_only;
        let focus = editor.focus_handle(cx);
        let focused = focus.is_focused(window)
            || self.visibility_focus.is_focused(window)
            || self.clear_focus.is_focused(window)
            || self.leading_focus.is_focused(window)
            || self.trailing_focus.is_focused(window);
        if focused && disabled {
            window.blur();
        }
        let scroll = editor.scroll_handle.clone();
        let scroll_state = editor.scrollbar_state.clone();
        let states = StateSelector(
            [
                (State::Disabled, disabled),
                (State::Hovered, self.hovered && !disabled),
                (State::FocusVisible, focused && !disabled),
            ]
            .into_iter()
            .filter_map(|(state, active)| active.then_some(state))
            .collect(),
        );
        let revision = self.theme.read(cx).revision();
        let material = self.material.mode(cx);
        if !self
            .resolved
            .as_ref()
            .is_some_and(|cache| cache.0 == revision && cache.1 == material && cache.2 == states)
        {
            match self.resolve(&states, cx) {
                Ok(styles) => {
                    self.resolved = Some((revision, material, states, styles));
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(error.clone());
                    return div()
                        .child(format!("Input style error: {error}"))
                        .into_any_element();
                }
            }
        }
        let [
            root,
            placeholder,
            caret,
            selection,
            scrollbar,
            scrollbar_hover,
            scrollbar_pressed,
            action_button,
            resize_handle,
        ] = &self.resolved.as_ref().unwrap().3;
        let font_size = length(&root.font_size, px(14.));
        let line_height = font_size * 1.5;
        self.editor.update(cx, |editor, _| {
            editor.placeholder_style.clone_from(placeholder);
            editor.caret_style.clone_from(caret);
            editor.selection_style.clone_from(selection);
        });
        let radius = length(&root.radius, px(6.));
        let border = length(&root.border_width, px(1.));
        let height = length(
            &root.height,
            if multiline {
                line_height * self.rows as f32
                    + length(&root.padding_top, px(8.))
                    + length(&root.padding_bottom, px(8.))
                    + border * 2.
            } else {
                line_height
                    + length(&root.padding_top, px(8.))
                    + length(&root.padding_bottom, px(8.))
                    + border * 2.
            },
        );
        let resizable = multiline && !disabled && self.resize.axes != super::InputResize::Disabled;
        if !resizable {
            self.resize.dragging = None;
        }
        let glass = root.material;
        let mut style = root.to_gpui();
        let border_style = style.border_style.unwrap_or(BorderStyle::Solid);
        let border_gradient = style.border_gradient.clone().unwrap_or_default();
        if glass.is_some() {
            style.background = None;
            style.border_gradient = None;
            // Borders are painted over the material, before the editable content.
            style.border_color = Some(transparent_black());
        }
        let border_color = color(&root.border_color);
        let has_border = |patch: &StylePatch| {
            !matches!(patch.properties.border_color, crate::style::Property::Unset)
                || patch.properties.border_gradient.is_some()
        };
        let glass_outline = has_border(&self.style)
            || self.style.states.iter().any(|rule| {
                self.resolved.as_ref().unwrap().2.contains(&rule.when) && has_border(&rule.style)
            });
        let resize_owner = cx.weak_entity();
        let resize_color = color(&resize_handle.foreground);
        let resize_cursor = self.resize.axes.cursor();
        let mut field = div()
            .on_paint_before_children(move |bounds, _, window, cx| {
                let _ = resize_owner.update(cx, |this, _| this.resize.bounds = bounds);
                if resizable {
                    let owner = resize_owner.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture {
                            let _ = owner.update(cx, |this, cx| {
                                if this.resize.dragging.is_some() {
                                    if event.pressed_button == Some(MouseButton::Left) {
                                        if this.resize.move_to(event.position) {
                                            cx.notify();
                                        }
                                    } else {
                                        this.resize.dragging = None;
                                    }
                                    cx.stop_propagation();
                                }
                            });
                        }
                    });
                    let owner = resize_owner.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            let _ = owner.update(cx, |this, cx| {
                                if this.resize.dragging.take().is_some() {
                                    cx.stop_propagation();
                                }
                            });
                        }
                    });
                }
                if let Some(glass) = glass {
                    gpui_effects::paint_liquid_glass(bounds, Corners::all(radius), glass, window);
                    if glass_outline {
                        window.paint_quad(
                            quad(
                                bounds,
                                radius,
                                transparent_black(),
                                border,
                                border_color,
                                border_style,
                            )
                            .border_gradient(border_gradient.clone()),
                        );
                    }
                }
            })
            .id(("fusion-input", cx.entity_id()))
            .debug_selector(|| "fusion-input".into())
            .relative()
            .flex()
            .w_full()
            .h(height)
            .line_height(line_height)
            .min_w_0()
            .when(multiline, |field| field.items_start())
            .when(!multiline, |field| field.items_center())
            .cursor(if disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                let editor = this.editor.read(cx);
                if editor.mode == InputMode::Multiline
                    && !editor.disabled
                    && editor.focus_handle(cx).is_focused(window)
                {
                    // The editor consumes events over its viewport; handle field padding here.
                    let delta = event.delta.pixel_delta(window.line_height());
                    this.editor.update(cx, |editor, cx| {
                        editor.scroll_text_input(delta, window, cx);
                    });
                    cx.stop_propagation();
                }
            }))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.hovered != *hovered {
                    this.hovered = *hovered;
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !disabled {
                    window.focus(&focus, cx);
                }
            });
        if self.blur_on_click_outside {
            let focus = self.editor.focus_handle(cx);
            let adornment_focus = [
                self.visibility_focus.clone(),
                self.clear_focus.clone(),
                self.leading_focus.clone(),
                self.trailing_focus.clone(),
            ];
            field = field.on_mouse_down_out(move |event, window, _| {
                if event.button == MouseButton::Left
                    && (focus.is_focused(window)
                        || adornment_focus.iter().any(|focus| focus.is_focused(window)))
                {
                    window.blur();
                }
            });
        }
        field.style().refine(&style);
        if multiline && let Some(dimensions) = self.resize.dimensions {
            if matches!(
                self.resize.axes,
                super::InputResize::Both
                    | super::InputResize::Horizontal
                    | super::InputResize::Disabled
            ) {
                field = field.w(dimensions.width);
            }
            if matches!(
                self.resize.axes,
                super::InputResize::Both
                    | super::InputResize::Vertical
                    | super::InputResize::Disabled
            ) {
                field = field.h(dimensions.height);
            }
        }
        if let Some(prefix) = &self.prefix {
            field = field.child(self.render_adornment(
                prefix,
                InputSide::Leading,
                action_button,
                window,
                cx,
            ));
        }
        field = field.child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .when(multiline, |view| view.h_full())
                .child(self.editor.clone()),
        );
        if let Some(suffix) = &self.suffix {
            field = field.child(self.render_adornment(
                suffix,
                InputSide::Trailing,
                action_button,
                window,
                cx,
            ));
        }
        if self.clearable && has_value && !read_only {
            field = field.child(self.adornment(InputAction::Clear, action_button, cx));
        }
        if password && self.password_toggle {
            field = field.child(self.adornment(InputAction::Visibility, action_button, cx));
        }
        if multiline {
            field = field.child(
                Scrollbar::vertical(
                    ("fusion-input-scrollbar", self.editor.entity_id()),
                    &scroll_state,
                    &scroll,
                )
                .auto_hide(false)
                .thumb_styles([
                    scrollbar.clone(),
                    scrollbar_hover.clone(),
                    scrollbar_pressed.clone(),
                ])
                .absolute()
                .right(px(2.))
                .top(px(4.))
                .bottom(if resizable { px(17.) } else { px(4.) })
                .h_auto(),
            );
        }
        if resizable {
            field = field.child(
                div()
                    .on_paint_before_children(move |bounds, _, window, _| {
                        for (row, count) in [(0., 1), (4., 2), (8., 3)] {
                            for col in 0..count {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(
                                            bounds.right() - px(4. + col as f32 * 4.),
                                            bounds.top() + px(2. + row),
                                        ),
                                        size(px(1.5), px(1.5)),
                                    ),
                                    resize_color,
                                ));
                            }
                        }
                    })
                    .id("resize-handle")
                    .debug_selector(|| "fusion-input-resize".into())
                    .absolute()
                    .right(px(2.))
                    .bottom(px(2.))
                    .size(px(16.))
                    .cursor(resize_cursor)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.resize.begin(event.position);
                            window.focus(&this.editor.focus_handle(cx), cx);
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        field.into_any_element()
    }
}
pub(super) fn length(value: &Resolved<Pixels>, fallback: Pixels) -> Pixels {
    if let Resolved::Value(value) = value {
        *value
    } else {
        fallback
    }
}
pub(super) fn color(value: &Resolved<Hsla>) -> Hsla {
    if let Resolved::Value(value) = value {
        *value
    } else {
        transparent_black()
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
