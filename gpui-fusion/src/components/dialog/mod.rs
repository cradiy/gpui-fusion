use crate::{
    components::selection_surface::{color, length, selection_surface},
    style::{ComputedStyle, MaterialMode, Part, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::Styled as _;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, IntoElement, KeyDownEvent,
    MouseButton, Render, Role, SharedString, Subscription, Task, Window, deferred_overlay, div,
    prelude::*, px, svg,
};
use gpui_icons::GpuiIcons;
use scheduler::Instant;
use std::{rc::Rc, time::Duration};

type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type CloseGuard = Box<dyn Fn(DialogCloseReason, &mut Window, &mut App) -> bool>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogCloseReason {
    CloseButton,
    Escape,
    Backdrop,
    Programmatic,
    /// The active style could not be resolved. See `Dialog::style_error`.
    StyleError,
}

/// Emitted once after the closing transition completes.
#[derive(Clone, Copy, Debug)]
pub struct DialogClosed(pub DialogCloseReason);

#[derive(Clone, Copy, Debug)]
pub enum DialogPart {
    Backdrop,
    Title,
    Body,
    Footer,
    Close,
}
impl Part for DialogPart {
    fn name(&self) -> &str {
        match self {
            Self::Backdrop => "backdrop",
            Self::Title => "title",
            Self::Body => "body",
            Self::Footer => "footer",
            Self::Close => "close",
        }
    }
}

#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    backdrop: ComputedStyle,
    title: ComputedStyle,
    body: ComputedStyle,
    footer: ComputedStyle,
    close: ComputedStyle,
}

/// A window-local modal overlay. Mount it once alongside the window content.
/// Only one independent dialog should be open in a window at a time.
pub struct Dialog {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    cache: Option<(u64, MaterialMode, Styles)>,
    error: Option<StyleError>,
    title: SharedString,
    content: Content,
    footer: Option<Content>,
    focus: FocusHandle,
    first: FocusHandle,
    last: FocusHandle,
    close_focus: FocusHandle,
    initial_focus: Option<FocusHandle>,
    previous_focus: Option<FocusHandle>,
    focus_pending: bool,
    visible: bool,
    closing: Option<DialogCloseReason>,
    from: f32,
    target: f32,
    started: Instant,
    motion_duration: Duration,
    duration: Duration,
    animated: bool,
    close_on_escape: bool,
    close_on_backdrop: bool,
    close_button: bool,
    close_guard: Option<CloseGuard>,
    close_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<DialogClosed> for Dialog {}
impl Dialog {
    pub fn new<E: IntoElement>(
        title: impl Into<SharedString>,
        content: impl Fn(&mut Window, &mut App) -> E + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_focus_out(&focus, window, |this, _, window, cx| {
                if this.visible && !this.focus.contains_focused(window, cx) {
                    this.focus.focus(window, cx);
                }
            }),
        ];
        Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            cache: None,
            error: None,
            title: title.into(),
            content: Rc::new(move |w, c| content(w, c).into_any_element()),
            footer: None,
            focus,
            first: cx.focus_handle().tab_stop(true).tab_index(isize::MIN),
            last: cx.focus_handle().tab_stop(true).tab_index(isize::MAX),
            close_focus: cx.focus_handle().tab_stop(true),
            initial_focus: None,
            previous_focus: None,
            focus_pending: false,
            visible: false,
            closing: None,
            from: 0.,
            target: 0.,
            started: cx.background_executor().now(),
            motion_duration: Duration::ZERO,
            duration: Duration::from_millis(320),
            animated: true,
            close_on_escape: true,
            close_on_backdrop: true,
            close_button: true,
            close_guard: None,
            close_task: None,
            _subscriptions: subscriptions,
        }
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
    pub fn close_on_escape(mut self, enabled: bool) -> Self {
        self.close_on_escape = enabled;
        self
    }
    pub fn close_on_backdrop(mut self, enabled: bool) -> Self {
        self.close_on_backdrop = enabled;
        self
    }
    pub fn close_button(mut self, enabled: bool) -> Self {
        self.close_button = enabled;
        self
    }
    /// Return false to keep the dialog open, including for programmatic close requests.
    pub fn on_close_request(
        mut self,
        guard: impl Fn(DialogCloseReason, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.close_guard = Some(Box::new(guard));
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
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.cache = None;
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
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
    /// Includes the closing transition, during which background input remains blocked.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open() {
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
            self.previous_focus = window.focused(cx);
            self.from = 0.;
            self.target = 0.;
            self.motion_duration = Duration::ZERO;
        }
        self.visible = true;
        self.closing = None;
        self.retarget(1., window, cx);
        self.focus.focus(window, cx);
        self.focus_pending = true;
        cx.notify();
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.request_close(DialogCloseReason::Programmatic, window, cx)
    }
    fn request_close(
        &mut self,
        reason: DialogCloseReason,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_open()
            || self
                .close_guard
                .as_ref()
                .is_some_and(|guard| !guard(reason, window, cx))
        {
            return false;
        }
        self.closing = Some(reason);
        self.retarget(0., window, cx);
        self.focus.focus(window, cx);
        if self.motion_duration.is_zero() {
            self.finish_close(window, cx);
        } else {
            let timer = cx.background_executor().timer(self.motion_duration);
            self.close_task = Some(cx.spawn_in(window, async move |this, cx| {
                timer.await;
                let _ = this.update_in(cx, |this, w, c| this.finish_close(w, c));
            }));
        }
        cx.notify();
        true
    }
    fn finish_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reason) = self.closing.take() else {
            return;
        };
        self.visible = false;
        self.close_task = None;
        if self.focus.contains_focused(window, cx) {
            if let Some(focus) = self.previous_focus.take() {
                focus.focus(window, cx);
            } else {
                window.blur();
            }
        } else {
            self.previous_focus = None;
        }
        cx.emit(DialogClosed(reason));
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
            .validate(&context, &["backdrop", "title", "body", "footer", "close"])?;
        let recipe = runtime.styles().recipe("dialog")?;
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
            backdrop: resolve(Some("backdrop"))?,
            title: resolve(Some("title"))?,
            body: resolve(Some("body"))?,
            footer: resolve(Some("footer"))?,
            close: resolve(Some("close"))?,
        };
        self.cache = Some((runtime.revision(), mode, styles.clone()));
        Ok(styles)
    }
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.visible {
            return None;
        }
        let styles = match self.resolve(cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error);
                self.closing = Some(DialogCloseReason::StyleError);
                self.finish_close(window, cx);
                return None;
            }
        };
        let (mut progress, moving) = self.progress(cx.background_executor().now());
        if window.prefers_reduced_motion() {
            progress = self.target;
        } else if moving {
            window.request_animation_frame();
        }
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
        }
        let viewport = window.viewport_size();
        let margin = px(24.).min(viewport.width / 8.).min(viewport.height / 8.);
        let width =
            length(&styles.root.width, 480.).min((viewport.width - margin * 2.).max(px(0.)));
        let content = (self.content)(window, cx);
        let mut title = div()
            .flex_1()
            .min_w_0()
            .font_weight(gpui::FontWeight::SEMIBOLD);
        title.style().refine(&styles.title.to_gpui());
        let mut header = div()
            .flex()
            .items_center()
            .gap_4()
            .flex_shrink_0()
            .child(title.child(self.title.clone()));
        if self.close_button {
            let mut close = div()
                .id("dialog-close")
                .debug_selector(|| "fusion-dialog-close".into())
                .track_focus(&self.close_focus)
                .tab_stop(true)
                .role(Role::Button)
                .aria_label("Close dialog")
                .size(px(28.))
                .rounded(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, w, c| {
                    this.request_close(DialogCloseReason::CloseButton, w, c);
                    c.stop_propagation();
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, c| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") && !event.is_held {
                        this.request_close(DialogCloseReason::CloseButton, w, c);
                        c.stop_propagation();
                    }
                }));
            close.style().refine(&styles.close.to_gpui());
            if self.close_focus.is_focused(window) && window.last_input_was_keyboard() {
                close = close
                    .border_1()
                    .border_color(color(&styles.close.border_color));
            }
            header = header.child(
                close.child(
                    svg()
                        .path(GpuiIcons::LucideX.path())
                        .size(px(16.))
                        .text_color(color(&styles.close.foreground)),
                ),
            );
        }
        let mut body = div()
            .id("dialog-body")
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .on_scroll_wheel(|_, _, c| c.stop_propagation());
        body.style().refine(&styles.body.to_gpui());
        let mut panel = selection_surface(
            div().flex().flex_col().gap_4(),
            styles.root.clone(),
            styles.root.clone(),
            true,
            1.,
            1.,
            0.,
        )
        .id("dialog-panel")
        .debug_selector(|| "fusion-dialog-panel".into())
        .w(width)
        .max_h((viewport.height - margin * 2.).max(px(0.)))
        .min_h_0()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, c| c.stop_propagation())
        .child(
            div()
                .id("dialog-first")
                .absolute()
                .track_focus(&self.first)
                .tab_index(isize::MIN)
                .size_0(),
        )
        .child(header)
        .child(body.child(content));
        if let Some(footer) = &self.footer {
            let mut container = div().flex().justify_end().gap_3().flex_shrink_0();
            container.style().refine(&styles.footer.to_gpui());
            panel = panel.child(container.child(footer(window, cx)));
        }
        panel = panel.child(
            div()
                .id("dialog-last")
                .absolute()
                .track_focus(&self.last)
                .tab_index(isize::MAX)
                .size_0(),
        );
        let mut backdrop = div()
            .id("dialog-backdrop")
            .debug_selector(|| "fusion-dialog-backdrop".into())
            .absolute()
            .top_0()
            .left_0()
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .track_focus(&self.focus)
            .tab_group()
            .tab_stop(false)
            .key_context("FusionDialog")
            .role(Role::Dialog)
            .aria_label(self.title.clone())
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
                            this.request_close(DialogCloseReason::Escape, w, c);
                        }
                        c.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, w, c| {
                    if this.close_on_backdrop {
                        this.request_close(DialogCloseReason::Backdrop, w, c);
                    }
                    c.defer_in(w, |this, w, c| {
                        if this.visible && !this.focus.contains_focused(w, c) {
                            this.focus.focus(w, c);
                        }
                    });
                    c.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, c| c.stop_propagation());
        backdrop.style().refine(&styles.backdrop.to_gpui());
        let mut overlay = backdrop
            .bg(color(&styles.backdrop.background).opacity(progress))
            .child(
                div()
                    .relative()
                    .top(px(16. * (1. - progress)))
                    .opacity(progress)
                    .child(panel),
            );
        if self.closing.is_some() {
            overlay = overlay.child(
                div()
                    .id("dialog-exiting")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, c| c.stop_propagation())
                    .on_click(|_, _, c| c.stop_propagation()),
            );
        }
        Some(overlay.into_any_element())
    }
}
impl Styled for Dialog {
    type Part = DialogPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Render for Dialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        deferred_overlay(move |w, c| weak.update(c, |this, c| this.overlay(w, c)).ok().flatten())
            .with_priority(50)
    }
}

#[cfg(test)]
mod tests;
