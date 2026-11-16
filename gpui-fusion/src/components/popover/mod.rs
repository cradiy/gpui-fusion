mod placement;
mod popup;
pub use placement::{PopoverAlign, PopoverPlacement, PopoverSide};

use crate::{
    components::{
        button::Button,
        selection_surface::{length, selection_surface},
    },
    style::{ComputedStyle, MaterialMode, Part, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::Styled as _;
use gpui::{
    AnyElement, App, Context, ElementBounds, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, MouseButton, Pixels, Render, Role, SharedString, Subscription, Task,
    Window, canvas, deferred_overlay, div, prelude::*, px,
};
use scheduler::Instant;
use std::{rc::Rc, time::Duration};

type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverCloseReason {
    Programmatic,
    Trigger,
    Escape,
    OutsideClick,
    FocusLost,
    AnchorLost,
    Disabled,
    WindowInactive,
    StyleError,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverEvent {
    Opened,
    /// The closing transition has finished.
    Closed(PopoverCloseReason),
}
#[derive(Clone, Copy, Debug)]
pub enum PopoverPart {
    Title,
    Body,
    Footer,
}
impl Part for PopoverPart {
    fn name(&self) -> &str {
        match self {
            Self::Title => "title",
            Self::Body => "body",
            Self::Footer => "footer",
        }
    }
}
#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    title: ComputedStyle,
    body: ComputedStyle,
    footer: ComputedStyle,
}

/// A button and an anchored interactive panel. Content views retain their own editing state.
pub struct Popover {
    trigger: Entity<Button>,
    label: SharedString,
    title: Option<SharedString>,
    content: Content,
    footer: Option<Content>,
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    cache: Option<(u64, MaterialMode, Styles)>,
    error: Option<StyleError>,
    anchor: ElementBounds,
    placement: PopoverPlacement,
    margin: Pixels,
    focus: FocusHandle,
    first: FocusHandle,
    last: FocusHandle,
    initial_focus: Option<FocusHandle>,
    focus_pending: bool,
    restore_focus: bool,
    disabled: bool,
    visible: bool,
    closing: Option<PopoverCloseReason>,
    close_on_escape: bool,
    close_on_outside: bool,
    duration: Duration,
    animated: bool,
    from: f32,
    target: f32,
    started: Instant,
    motion_duration: Duration,
    close_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<PopoverEvent> for Popover {}
impl Popover {
    pub fn new<E: IntoElement>(
        label: impl Into<SharedString>,
        content: impl Fn(&mut Window, &mut App) -> E + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let label = label.into();
        let weak = cx.weak_entity();
        let trigger = cx.new(|c| {
            Button::new(label.clone(), window, c).on_click(move |w, c| {
                let weak = weak.clone();
                w.defer(c, move |w, c| {
                    let _ = weak.update(c, |this: &mut Self, c| {
                        if this.is_open() {
                            this.dismiss(PopoverCloseReason::Trigger, true, false, w, c);
                        } else {
                            this.open(w, c);
                        }
                    });
                });
            })
        });
        trigger.update(cx, |b, c| b.set_expanded(false, c));
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, c| c.notify()),
            cx.on_focus_out(&focus, window, |this, _, w, c| {
                if this.is_open()
                    && !this.focus.contains_focused(w, c)
                    && !this.trigger.focus_handle(c).is_focused(w)
                {
                    this.dismiss(PopoverCloseReason::FocusLost, false, false, w, c);
                }
            }),
            cx.observe_window_activation(window, |this, w, c| {
                if !w.is_window_active() {
                    this.dismiss(PopoverCloseReason::WindowInactive, false, true, w, c);
                }
            }),
        ];
        Self {
            trigger,
            label,
            title: None,
            content: Rc::new(move |w, c| content(w, c).into_any_element()),
            footer: None,
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            cache: None,
            error: None,
            anchor: ElementBounds::default(),
            placement: PopoverPlacement::default(),
            margin: px(12.),
            focus,
            first: cx.focus_handle().tab_stop(true).tab_index(isize::MIN),
            last: cx.focus_handle().tab_stop(true).tab_index(isize::MAX),
            initial_focus: None,
            focus_pending: false,
            restore_focus: false,
            disabled: false,
            visible: false,
            closing: None,
            close_on_escape: true,
            close_on_outside: true,
            duration: Duration::from_millis(280),
            animated: true,
            from: 0.,
            target: 0.,
            started: cx.background_executor().now(),
            motion_duration: Duration::ZERO,
            close_task: None,
            _subscriptions: subscriptions,
        }
    }
    /// The persistent trigger can be styled independently of the panel.
    pub fn trigger(&self) -> &Entity<Button> {
        &self.trigger
    }
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
    pub fn footer<E: IntoElement>(
        mut self,
        render: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.footer = Some(Rc::new(move |w, c| render(w, c).into_any_element()));
        self
    }
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }
    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }
    pub fn set_placement(&mut self, placement: PopoverPlacement, cx: &mut Context<Self>) {
        self.placement = placement;
        cx.notify();
    }
    pub fn viewport_margin(mut self, margin: Pixels) -> Self {
        self.margin = margin.max(px(0.));
        self
    }
    pub fn close_on_escape(mut self, enabled: bool) -> Self {
        self.close_on_escape = enabled;
        self
    }
    /// Disables outside-pointer dismissal. Moving keyboard focus outside still closes the panel.
    pub fn close_on_outside(mut self, enabled: bool) -> Self {
        self.close_on_outside = enabled;
        self
    }
    pub fn animated(mut self, enabled: bool) -> Self {
        self.animated = enabled;
        self
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    /// Fixes both panel and trigger material. None follows the application theme.
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
        self.trigger.update(cx, |b, c| b.set_material_mode(mode, c));
    }
    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.trigger.update(cx, |b, c| b.set_disabled(disabled, c));
        if disabled {
            self.dismiss(PopoverCloseReason::Disabled, false, true, window, cx);
        }
        cx.notify();
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
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.cache = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    pub fn is_open(&self) -> bool {
        self.visible && self.closing.is_none()
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.is_open() {
            return;
        }
        if let Err(error) = self.resolve(cx) {
            self.error = Some(error);
            cx.notify();
            return;
        }
        self.error = None;
        self.close_task = None;
        if !self.visible {
            self.from = 0.;
            self.target = 0.;
            self.motion_duration = Duration::ZERO;
        }
        self.visible = true;
        self.closing = None;
        self.restore_focus = true;
        self.retarget(1., window, cx);
        self.focus.focus(window, cx);
        self.focus_pending = true;
        self.trigger.update(cx, |b, c| b.set_expanded(true, c));
        cx.emit(PopoverEvent::Opened);
        cx.notify();
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(PopoverCloseReason::Programmatic, true, false, window, cx);
    }
    fn dismiss(
        &mut self,
        reason: PopoverCloseReason,
        restore: bool,
        immediate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.visible || (self.closing.is_some() && !immediate) {
            return;
        }
        self.closing = Some(reason);
        self.restore_focus = restore;
        self.focus_pending = false;
        self.retarget(0., window, cx);
        self.trigger.update(cx, |b, c| b.set_expanded(false, c));
        if restore && self.focus.contains_focused(window, cx) {
            self.focus.focus(window, cx);
        }
        if immediate || self.motion_duration.is_zero() {
            self.finish_close(window, cx);
        } else {
            let timer = cx.background_executor().timer(self.motion_duration);
            self.close_task = Some(cx.spawn_in(window, async move |this, c| {
                timer.await;
                let _ = this.update_in(c, |this, w, c| this.finish_close(w, c));
            }));
        }
        cx.notify();
    }
    fn finish_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reason) = self.closing.take() else {
            return;
        };
        self.visible = false;
        self.close_task = None;
        if self.focus.contains_focused(window, cx) {
            if self.restore_focus && !self.disabled {
                self.trigger.focus_handle(cx).focus(window, cx);
            } else {
                window.blur();
            }
        }
        cx.emit(PopoverEvent::Closed(reason));
        cx.notify();
    }
    fn progress(&self, now: Instant) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.motion_duration.is_zero() || elapsed >= self.motion_duration {
            return (self.target, false);
        }
        let t = elapsed.as_secs_f32() / self.motion_duration.as_secs_f32();
        (
            self.from + (self.target - self.from) * t * t * (3. - 2. * t),
            true,
        )
    }
    fn retarget(&mut self, target: f32, window: &Window, cx: &App) {
        let now = cx.background_executor().now();
        self.from = self.progress(now).0;
        self.target = target;
        self.started = now;
        self.motion_duration = if self.animated && !window.prefers_reduced_motion() {
            self.duration.mul_f32((target - self.from).abs())
        } else {
            Duration::ZERO
        };
    }
    fn advance_focus(&self, backwards: bool, window: &mut Window, cx: &mut App) {
        if backwards {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
        if self.first.is_focused(window)
            || self.last.is_focused(window)
            || !self.focus.contains_focused(window, cx)
        {
            if backwards {
                self.last.focus(window, cx);
                window.focus_prev(cx);
            } else {
                self.first.focus(window, cx);
                window.focus_next(cx);
            }
        }
        if self.first.is_focused(window)
            || self.last.is_focused(window)
            || !self.focus.contains_focused(window, cx)
        {
            self.focus.focus(window, cx);
        }
    }
    fn resolve(&mut self, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let mode = self.material.mode(cx);
        if let Some((revision, material, styles)) = &self.cache
            && *revision == runtime.revision()
            && *material == mode
        {
            return Ok(styles.clone());
        }
        let mut context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style
            .validate(&context, &["title", "body", "footer"])?;
        let recipe = runtime.styles().recipe("popover")?;
        let resolve = |part| {
            recipe.resolve(
                self.variant.as_deref(),
                mode,
                &self.style,
                &context,
                &StateSelector::default(),
                part,
            )
        };
        let styles = Styles {
            root: resolve(None)?,
            title: resolve(Some("title"))?,
            body: resolve(Some("body"))?,
            footer: resolve(Some("footer"))?,
        };
        self.cache = Some((runtime.revision(), mode, styles.clone()));
        Ok(styles)
    }
    fn popup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.visible {
            return None;
        }
        let Some(anchor) = self.anchor.visible_bounds(window) else {
            self.dismiss(PopoverCloseReason::AnchorLost, false, true, window, cx);
            return None;
        };
        let styles = match self.resolve(cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error);
                self.dismiss(PopoverCloseReason::StyleError, true, true, window, cx);
                return None;
            }
        };
        if self.focus_pending {
            self.focus_pending = false;
            cx.defer_in(window, |this, w, c| {
                if !this.is_open() || !this.focus.is_focused(w) {
                    return;
                }
                if let Some(focus) = &this.initial_focus
                    && this.focus.contains(focus, w)
                {
                    focus.focus(w, c);
                } else {
                    this.first.focus(w, c);
                    this.advance_focus(false, w, c);
                }
            });
        } else if self.is_open() && self.focus.contains_focused(window, cx) {
            let focused = window.focused(cx);
            cx.defer_in(window, move |this, w, c| {
                if this.is_open() && w.focused(c) == focused && !this.focus.contains_focused(w, c) {
                    this.focus.focus(w, c);
                }
            });
        }
        let (mut progress, moving) = self.progress(cx.background_executor().now());
        if window.prefers_reduced_motion() {
            progress = self.target;
        } else if moving {
            window.request_animation_frame();
        }
        let viewport = window.viewport_size();
        let margin = self
            .margin
            .min(viewport.width / 4.)
            .min(viewport.height / 4.);
        let width = length(&styles.root.width, 320.).min(viewport.width - margin * 2.);
        let mut body = div()
            .id("popover-body")
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .on_scroll_wheel(|_, _, c| c.stop_propagation());
        body.style().refine(&styles.body.to_gpui());
        let mut panel = selection_surface(
            div().flex().flex_col().gap_3(),
            styles.root.clone(),
            styles.root,
            true,
            1.,
            1.,
            0.,
        )
        .id("popover-panel")
        .debug_selector(|| "fusion-popover-panel".into())
        .w(width)
        .max_h(viewport.height - margin * 2.)
        .min_h_0()
        .occlude()
        .track_focus(&self.focus)
        .tab_group()
        .tab_stop(false)
        .key_context("FusionPopover")
        .role(Role::Dialog)
        .aria_label(self.title.clone().unwrap_or_else(|| self.label.clone()))
        .on_mouse_down(MouseButton::Left, |_, _, c| c.stop_propagation())
        .capture_key_down(cx.listener(|this, _, _, c| {
            if this.closing.is_some() {
                c.stop_propagation();
            }
        }))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, c| {
            match event.keystroke.key.as_str() {
                "tab" => {
                    this.advance_focus(event.keystroke.modifiers.shift, w, c);
                    c.stop_propagation();
                }
                "escape" => {
                    if this.close_on_escape && !event.is_held {
                        this.dismiss(PopoverCloseReason::Escape, true, false, w, c);
                    }
                    c.stop_propagation();
                }
                _ => {}
            }
        }))
        .child(
            div()
                .id("popover-first")
                .absolute()
                .track_focus(&self.first)
                .size_0(),
        );
        if let Some(title) = &self.title {
            let mut label = div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .flex_shrink_0();
            label.style().refine(&styles.title.to_gpui());
            panel = panel.child(label.child(title.clone()));
        }
        panel = panel.child(body.child((self.content)(window, cx)));
        if let Some(footer) = &self.footer {
            let mut element = div().flex().justify_end().gap_2().flex_shrink_0();
            element.style().refine(&styles.footer.to_gpui());
            panel = panel.child(element.child(footer(window, cx)));
        }
        panel = panel.child(
            div()
                .id("popover-last")
                .absolute()
                .track_focus(&self.last)
                .size_0(),
        );
        if self.closing.is_some() {
            panel = panel.child(
                div()
                    .id("popover-exiting")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, c| c.stop_propagation())
                    .on_click(|_, _, c| c.stop_propagation()),
            );
        }
        let popup = popup::Popup {
            content: div().opacity(progress).child(panel).into_any_element(),
            anchor,
            placement: self.placement,
            margin,
            progress,
        };
        Some(
            div()
                .id("popover-overlay")
                .absolute()
                .top_0()
                .left_0()
                .w(viewport.width)
                .h(viewport.height)
                .on_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, w, c| {
                    if this.close_on_outside && !this.anchor.contains(event.position, w) {
                        this.dismiss(PopoverCloseReason::OutsideClick, false, false, w, c);
                    }
                }))
                .child(popup)
                .into_any_element(),
        )
    }
}
impl Styled for Popover {
    type Part = PopoverPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Focusable for Popover {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.trigger.focus_handle(cx)
    }
}
impl Render for Popover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let anchor = self.anchor.clone();
        div()
            .id("popover-anchor")
            .relative()
            .flex()
            .flex_none()
            .debug_selector(|| "fusion-popover-anchor".into())
            .child(self.trigger.clone())
            .child(
                canvas(
                    move |bounds, w, _| w.track_element_bounds(&anchor, bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                deferred_overlay(move |w, c| {
                    weak.update(c, |this, c| this.popup(w, c)).ok().flatten()
                })
                .with_priority(100),
            )
    }
}

#[cfg(test)]
mod tests;
