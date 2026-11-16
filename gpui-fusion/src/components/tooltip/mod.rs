mod popup;

use crate::{
    components::{
        dropdown::{DropdownAlign, DropdownPlacement, DropdownSide},
        selection_surface::selection_surface,
    },
    style::{ComputedStyle, MaterialMode, Part, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    Animation, AnimationExt, AnyView, App, Context, ElementBounds, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled as _, Subscription, Task, Window, canvas, deferred_overlay,
    div, px,
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default)]
pub enum TooltipSide {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug)]
pub enum TooltipPart {
    Shortcut,
}
impl Part for TooltipPart {
    fn name(&self) -> &str {
        "shortcut"
    }
}
struct Styles {
    revision: u64,
    material: MaterialMode,
    body: ComputedStyle,
    shortcut: ComputedStyle,
}

/// A hover and keyboard-focus hint wrapping an existing focusable view.
pub struct Tooltip {
    target: AnyView,
    focus: FocusHandle,
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    text: SharedString,
    shortcut: Option<SharedString>,
    side: TooltipSide,
    gap: Pixels,
    max_width: Pixels,
    show_delay: Duration,
    hide_delay: Duration,
    transition_duration: Duration,
    anchor: ElementBounds,
    hovered: bool,
    popup_hovered: bool,
    focused: bool,
    dismissed: bool,
    disabled: bool,
    visible: bool,
    pending: Option<bool>,
    task: Option<Task<()>>,
    keystrokes: Option<Subscription>,
    generation: usize,
    cache: Option<Styles>,
    error: Option<StyleError>,
    _subscriptions: Vec<Subscription>,
}
impl Tooltip {
    pub fn new<T: Render + Focusable>(
        text: impl Into<SharedString>,
        target: &Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = target.focus_handle(cx);
        let theme = ThemeRuntime::global(cx);
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |this, window, cx| {
                this.focused = window.last_input_was_keyboard();
                this.sync(window, cx);
            }),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.focused = false;
                this.sync(window, cx);
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.hovered = false;
                    this.focused = false;
                    this.dismiss(window, cx);
                }
            }),
        ];
        Self {
            target: target.clone().into(),
            focus,
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            text: text.into(),
            shortcut: None,
            side: TooltipSide::Top,
            gap: px(8.),
            max_width: px(300.),
            show_delay: Duration::from_millis(500),
            hide_delay: Duration::from_millis(120),
            transition_duration: Duration::from_millis(120),
            anchor: ElementBounds::default(),
            hovered: false,
            popup_hovered: false,
            focused: false,
            dismissed: false,
            disabled: false,
            visible: false,
            pending: None,
            task: None,
            keystrokes: None,
            generation: 0,
            cache: None,
            error: None,
            _subscriptions: subscriptions,
        }
    }
    pub fn shortcut(mut self, text: impl Into<SharedString>) -> Self {
        self.shortcut = Some(text.into());
        self
    }
    pub fn side(mut self, side: TooltipSide) -> Self {
        self.side = side;
        self
    }
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap.max(px(0.));
        self
    }
    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = width.max(px(1.));
        self
    }
    pub fn show_delay(mut self, delay: Duration) -> Self {
        self.show_delay = delay;
        self
    }
    pub fn hide_delay(mut self, delay: Duration) -> Self {
        self.hide_delay = delay;
        self
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.transition_duration = duration;
        self
    }
    /// Suppresses the hint without disabling the wrapped control.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.cache = None;
        self
    }
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
        self.cache = None;
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.cache = None;
        cx.notify();
    }
    pub fn set_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text = text.into();
        self.sync(window, cx);
        cx.notify();
    }
    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.sync(window, cx);
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismissed = true;
        self.popup_hovered = false;
        self.cancel();
        self.set_visible(false, window, cx);
    }
    fn cancel(&mut self) {
        self.task = None;
        self.pending = None;
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let interested = self.hovered || self.popup_hovered || self.focused;
        if !interested {
            self.dismissed = false;
        }
        let wanted = interested && !self.dismissed && !self.disabled && !self.text.is_empty();
        if wanted == self.visible {
            self.cancel();
            return;
        }
        if self.pending == Some(wanted) && !(wanted && self.focused) {
            return;
        }
        self.cancel();
        let delay = if wanted && !self.focused {
            self.show_delay
        } else if !wanted && !self.disabled && !self.text.is_empty() {
            self.hide_delay
        } else {
            Duration::ZERO
        };
        if delay.is_zero() {
            self.set_visible(wanted, window, cx);
            return;
        }
        self.pending = Some(wanted);
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.pending = None;
                this.task = None;
                this.set_visible(wanted, window, cx);
            });
        }));
    }
    fn set_visible(&mut self, visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            self.generation = self.generation.wrapping_add(1);
            let owner_window = window.window_handle();
            self.keystrokes = Some(cx.observe_keystrokes(move |this, event, window, cx| {
                if window.window_handle() == owner_window && event.keystroke.key == "escape" {
                    this.dismiss(window, cx);
                }
            }));
        } else {
            self.popup_hovered = false;
            self.keystrokes = None;
        }
        cx.notify();
    }
    fn resolve(&mut self, cx: &App) -> Result<&Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        if self
            .cache
            .as_ref()
            .is_none_or(|s| s.revision != runtime.revision() || s.material != material)
        {
            let mut context = ResolveContext::new(
                runtime.store().theme(),
                runtime.store().mode(),
                runtime.style_set(),
            )?;
            for scope in &self.scopes {
                context = context.scoped(scope)?;
            }
            self.style.validate(&context, &["shortcut"])?;
            let recipe = runtime.styles().recipe("tooltip")?;
            let resolve = |part| {
                recipe.resolve(
                    self.variant.as_deref(),
                    material,
                    &self.style,
                    &context,
                    &StateSelector::default(),
                    part,
                )
            };
            self.cache = Some(Styles {
                revision: runtime.revision(),
                material,
                body: resolve(None)?,
                shortcut: resolve(Some("shortcut"))?,
            });
        }
        Ok(self.cache.as_ref().unwrap())
    }
    fn popup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.visible {
            return None;
        }
        let Some(anchor) = self.anchor.visible_bounds(window) else {
            self.hovered = false;
            self.focused = false;
            self.cancel();
            self.set_visible(false, window, cx);
            return None;
        };
        let (body, shortcut_style) = match self.resolve(cx) {
            Ok(styles) => (styles.body.clone(), styles.shortcut.clone()),
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        self.error = None;
        let opacity = body.to_gpui().opacity.unwrap_or(1.);
        let width = self.max_width.min(window.viewport_size().width).max(px(1.));
        let mut content = selection_surface(div(), body.clone(), body, false, 0., 1., 0.)
            .id("tooltip-bubble")
            .debug_selector(|| "fusion-tooltip-bubble".into())
            .role(gpui::Role::Tooltip)
            .aria_label(self.text.clone())
            .flex()
            .items_center()
            .max_w(width)
            .max_h(window.viewport_size().height)
            .overflow_hidden()
            .occlude()
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.popup_hovered = *hovered;
                this.sync(window, cx);
            }))
            .child(div().min_w_0().flex_shrink(1.).child(self.text.clone()));
        if let Some(shortcut) = &self.shortcut {
            use gpui::Refineable;
            let mut label = div().flex_none().child(shortcut.clone());
            label.style().refine(&shortcut_style.to_gpui());
            content = content.child(label);
        }
        let content = if self.transition_duration.is_zero() {
            content.into_any_element()
        } else {
            content
                .with_animation(
                    ("tooltip-fade", self.generation),
                    Animation::new(self.transition_duration),
                    move |s, t| s.opacity(opacity * (1. - (1. - t).powi(3))),
                )
                .into_any_element()
        };
        let side = match self.side {
            TooltipSide::Top => DropdownSide::Top,
            TooltipSide::Bottom => DropdownSide::Bottom,
            TooltipSide::Left => DropdownSide::Left,
            TooltipSide::Right => DropdownSide::Right,
        };
        Some(
            popup::Popup {
                content,
                anchor,
                placement: DropdownPlacement::new(side)
                    .align(DropdownAlign::Center)
                    .gap(self.gap),
            }
            .into_any_element(),
        )
    }
}
impl Styled for Tooltip {
    type Part = TooltipPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Focusable for Tooltip {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let overlay = self.visible.then(|| {
            deferred_overlay(move |window, cx| {
                weak.update(cx, |this, cx| this.popup(window, cx))
                    .ok()
                    .flatten()
            })
            .with_priority(200)
        });
        let anchor = self.anchor.clone();
        div()
            .id("tooltip-anchor")
            .relative()
            .flex()
            .flex_none()
            .debug_selector(|| "fusion-tooltip-anchor".into())
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.hovered = *hovered;
                this.sync(window, cx);
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
            )
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && (this.visible || this.pending.is_some()) {
                    this.dismiss(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.target.clone())
            .children(overlay)
            .child(
                canvas(
                    move |bounds, window, _| window.track_element_bounds(&anchor, bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
}

#[cfg(test)]
mod tests;
