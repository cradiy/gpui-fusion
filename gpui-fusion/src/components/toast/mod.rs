mod state;

use crate::{
    components::selection_surface::{color, selection_surface},
    style::{ComputedStyle, MaterialMode, Part, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Refineable, Render, SharedString, StatefulInteractiveElement,
    Styled as _, Subscription, Task, Transformation, Window, deferred_overlay, div, px, radians,
    svg,
};
use gpui_effects::animated_layout;
use gpui_icons::GpuiIcons;
use state::{Entry, Motion};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
    Loading,
}
impl ToastKind {
    fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Loading => "loading",
        }
    }
    fn icon(self) -> GpuiIcons {
        match self {
            Self::Info => GpuiIcons::LucideInfo,
            Self::Success => GpuiIcons::LucideCircleCheck,
            Self::Warning => GpuiIcons::LucideCircleAlert,
            Self::Error => GpuiIcons::LucideCircleX,
            Self::Loading => GpuiIcons::LucideLoader,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToastId(u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastPlacement {
    TopLeft,
    Top,
    TopRight,
    BottomLeft,
    Bottom,
    #[default]
    BottomRight,
}
impl ToastPlacement {
    fn top(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    message: SharedString,
    description: Option<SharedString>,
    kind: ToastKind,
    timeout: Option<Duration>,
}
impl Toast {
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            description: None,
            kind: ToastKind::Info,
            timeout: Some(Duration::from_secs(4)),
        }
    }
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    /// Loading messages persist until updated or dismissed. Set `timeout` afterward to override.
    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        if kind == ToastKind::Loading {
            self.timeout = None;
        }
        self
    }
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }
}
#[derive(Clone, Copy, Debug)]
pub enum ToastPart {
    Message,
    Description,
    Icon,
    Close,
}
impl Part for ToastPart {
    fn name(&self) -> &str {
        match self {
            Self::Message => "message",
            Self::Description => "description",
            Self::Icon => "icon",
            Self::Close => "close",
        }
    }
}
#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    message: ComputedStyle,
    description: ComputedStyle,
    icon: ComputedStyle,
    close: ComputedStyle,
}
struct Pending {
    id: ToastId,
    toast: Toast,
    restore_focus: Option<FocusHandle>,
}

/// A window-local notification overlay. Mount one host in the window's root view.
pub struct ToastHost {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    cache: BTreeMap<ToastKind, Styles>,
    cache_key: Option<(u64, MaterialMode)>,
    error: Option<StyleError>,
    entries: Vec<Entry>,
    pending: VecDeque<Pending>,
    next_id: u64,
    max_visible: usize,
    placement: ToastPlacement,
    margin: Pixels,
    duration: Duration,
    animated: bool,
    active: bool,
    timer: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl ToastHost {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = ThemeRuntime::global(cx);
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |host, window, cx| {
                host.active = window.is_window_active();
                host.pump(window, cx);
            }),
        ];
        Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            cache: BTreeMap::new(),
            cache_key: None,
            error: None,
            entries: Vec::new(),
            pending: VecDeque::new(),
            next_id: 1,
            max_visible: 3,
            placement: ToastPlacement::default(),
            margin: px(20.),
            duration: Duration::from_millis(320),
            animated: true,
            active: window.is_window_active(),
            timer: None,
            _subscriptions: subscriptions,
        }
    }
    pub fn placement(mut self, placement: ToastPlacement) -> Self {
        self.placement = placement;
        self
    }
    pub fn set_placement(&mut self, placement: ToastPlacement, cx: &mut Context<Self>) {
        self.placement = placement;
        cx.notify();
    }
    pub fn max_visible(mut self, count: usize) -> Self {
        self.max_visible = count.max(1);
        self
    }
    pub fn margin(mut self, margin: Pixels) -> Self {
        self.margin = margin.max(px(0.));
        self
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    pub fn animated(mut self, enabled: bool) -> Self {
        self.animated = enabled;
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
        self.cache.clear();
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self.cache.clear();
        self
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.cache.clear();
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    pub fn contains(&self, id: ToastId) -> bool {
        self.entries.iter().any(|e| e.id == id) || self.pending.iter().any(|e| e.id == id)
    }
    /// Returns a stable identifier for updating or dismissing this notification.
    pub fn show(&mut self, toast: Toast, window: &mut Window, cx: &mut Context<Self>) -> ToastId {
        let id = ToastId(self.next_id);
        self.next_id += 1;
        self.pending.push_back(Pending {
            id,
            toast,
            restore_focus: window.focused(cx),
        });
        self.pump(window, cx);
        id
    }
    /// Replaces a queued or visible notification and restarts its timeout.
    pub fn update_toast(
        &mut self,
        id: ToastId,
        toast: Toast,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let now = cx.background_executor().now();
        let duration = self.effective_duration(window);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.remaining = toast.timeout;
            entry.deadline = None;
            entry.toast = toast;
            if entry.motion.exiting() {
                entry.motion.retarget(1., now, duration);
            }
        } else if let Some(entry) = self.pending.iter_mut().find(|e| e.id == id) {
            entry.toast = toast;
        } else {
            return false;
        }
        self.pump(window, cx);
        true
    }
    pub fn dismiss(&mut self, id: ToastId, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let now = cx.background_executor().now();
        let duration = self.effective_duration(window);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            if entry.motion.exiting() {
                return false;
            }
            if entry.focus.is_focused(window) {
                if let Some(focus) = &entry.restore_focus {
                    focus.focus(window, cx);
                } else {
                    window.blur();
                }
            }
            entry.motion.retarget(0., now, duration);
            entry.deadline = None;
        } else if let Some(index) = self.pending.iter().position(|e| e.id == id) {
            self.pending.remove(index);
        } else {
            return false;
        }
        self.pump(window, cx);
        true
    }
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.clear();
        let ids = self.entries.iter().map(|e| e.id).collect::<Vec<_>>();
        for id in ids {
            self.dismiss(id, window, cx);
        }
    }
    fn effective_duration(&self, window: &Window) -> Duration {
        if self.animated && !window.prefers_reduced_motion() {
            self.duration
        } else {
            Duration::ZERO
        }
    }
    fn pause(
        &mut self,
        id: ToastId,
        value: bool,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            if focus {
                entry.focused = value;
            } else {
                entry.hovered = value;
            }
            self.pump(window, cx);
        }
    }
    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.timer = None;
        let now = cx.background_executor().now();
        let duration = self.effective_duration(window);
        for entry in &mut self.entries {
            entry.sync_deadline(now, self.active);
            if entry.deadline.is_some_and(|deadline| deadline <= now) {
                entry.deadline = None;
                entry.motion.retarget(0., now, duration);
            }
        }
        self.entries
            .retain(|entry| !entry.motion.exiting() || entry.motion.end() > now);
        while self.entries.len() < self.max_visible {
            let Some(pending) = self.pending.pop_front() else {
                break;
            };
            let id = pending.id;
            let focus = cx.focus_handle();
            let subscriptions = vec![
                cx.on_focus(&focus, window, move |host, w, c| {
                    host.pause(id, true, true, w, c)
                }),
                cx.on_blur(&focus, window, move |host, w, c| {
                    host.pause(id, false, true, w, c)
                }),
            ];
            let mut entry = Entry {
                id,
                remaining: pending.toast.timeout,
                deadline: None,
                toast: pending.toast,
                focus,
                restore_focus: pending.restore_focus,
                hovered: false,
                focused: false,
                motion: Motion::new(now, duration),
                _subscriptions: subscriptions,
            };
            entry.sync_deadline(now, self.active);
            self.entries.push(entry);
        }
        let wake = self
            .entries
            .iter()
            .filter_map(|entry| {
                if entry.motion.exiting() {
                    Some(entry.motion.end())
                } else {
                    entry.deadline
                }
            })
            .min();
        if let Some(wake) = wake {
            self.timer = Some(cx.spawn_in(window, async move |host, cx| {
                cx.background_executor()
                    .timer(wake.saturating_duration_since(cx.background_executor().now()))
                    .await;
                let _ = host.update_in(cx, |host, w, c| host.pump(w, c));
            }));
        }
        cx.notify();
    }
    fn resolve(&mut self, kind: ToastKind, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let key = (runtime.revision(), material);
        if self.cache_key != Some(key) {
            self.cache.clear();
            self.cache_key = Some(key);
        }
        if let Some(styles) = self.cache.get(&kind) {
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
            .validate(&context, &["message", "description", "icon", "close"])?;
        let recipe = runtime.styles().recipe("toast")?;
        let resolve = |part| {
            recipe.resolve(
                Some(self.variant.as_deref().unwrap_or(kind.name())),
                material,
                &self.style,
                &context,
                &StateSelector::default(),
                part,
            )
        };
        let styles = Styles {
            root: resolve(None)?,
            message: resolve(Some("message"))?,
            description: resolve(Some("description"))?,
            icon: resolve(Some("icon"))?,
            close: resolve(Some("close"))?,
        };
        self.cache.insert(kind, styles.clone());
        Ok(styles)
    }
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.entries.is_empty() {
            return None;
        }
        let now = cx.background_executor().now();
        let available = (window.viewport_size().width - self.margin * 2.).max(px(0.));
        let max_height = ((window.viewport_size().height
            - self.margin * 2.
            - px(10.) * (self.entries.len().saturating_sub(1) as f32))
            / (self.entries.len() as f32))
            .max(px(40.));
        let mut width = px(0.);
        let mut cards = Vec::new();
        let animate = self.animated && !window.prefers_reduced_motion();
        for index in 0..self.entries.len() {
            let entry = &self.entries[index];
            let (id, toast, focus, progress, moving, exiting) = (
                entry.id,
                entry.toast.clone(),
                entry.focus.clone(),
                entry.motion.sample(now).0,
                entry.motion.sample(now).1,
                entry.motion.exiting(),
            );
            let styles = match self.resolve(toast.kind, cx) {
                Ok(styles) => {
                    self.error = None;
                    styles
                }
                Err(error) => {
                    self.error = Some(error.clone());
                    return Some(
                        div()
                            .child(format!("Toast style error: {error}"))
                            .into_any_element(),
                    );
                }
            };
            let card_width = crate::components::selection_surface::length(&styles.root.width, 360.)
                .min(available);
            width = width.max(card_width);
            let mut icon = svg()
                .path(toast.kind.icon().path())
                .size(px(20.))
                .flex_shrink_0()
                .text_color(color(&styles.icon.foreground));
            icon.style().refine(&styles.icon.to_gpui());
            if toast.kind == ToastKind::Loading && animate && !exiting {
                let phase = now
                    .duration_since(self.entries[index].motion.started())
                    .as_secs_f32()
                    / 0.9;
                icon = icon.with_transformation(Transformation::rotate(radians(
                    phase * std::f32::consts::TAU,
                )));
                window.request_animation_frame();
            }
            if moving && animate {
                window.request_animation_frame();
            }
            let progress = if animate {
                progress
            } else if exiting {
                0.
            } else {
                1.
            };
            let mut message =
                div().debug_selector(move || format!("fusion-toast-message-{}", id.0));
            message.style().refine(&styles.message.to_gpui());
            let mut text = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(message.child(toast.message.clone()));
            if let Some(description) = toast.description {
                let mut label = div();
                label.style().refine(&styles.description.to_gpui());
                text = text.child(label.child(description));
            }
            let weak = cx.weak_entity();
            let weak_key = weak.clone();
            let mut close = div()
                .id(("toast-close", id.0))
                .debug_selector(move || format!("fusion-toast-close-{}", id.0))
                .track_focus(&focus)
                .tab_stop(!exiting)
                .role(gpui::Role::Button)
                .aria_label("Dismiss notification")
                .size(px(24.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, w, c| {
                    let _ = weak.update(c, |h, c| h.dismiss(id, w, c));
                    c.stop_propagation();
                })
                .on_key_down(move |event, w, c| {
                    if matches!(event.keystroke.key.as_str(), "escape" | "enter" | "space")
                        && !event.is_held
                    {
                        let _ = weak_key.update(c, |h, c| h.dismiss(id, w, c));
                        c.stop_propagation();
                    }
                });
            close.style().refine(&styles.close.to_gpui());
            if focus.is_focused(window) && window.last_input_was_keyboard() {
                close = close
                    .border_1()
                    .border_color(color(&styles.close.border_color));
            }
            let mut card = selection_surface(
                div().flex().items_start().gap_3(),
                styles.root.clone(),
                styles.root,
                true,
                1.,
                1.,
                0.,
            )
            .id(("toast", id.0))
            .debug_selector(move || format!("fusion-toast-{}", id.0))
            .w(card_width)
            .max_h(max_height)
            .overflow_y_scroll()
            .min_w_0()
            .occlude()
            .role(gpui::Role::Status)
            .aria_label(toast.message)
            .on_hover(cx.listener(move |host, hovered, w, c| host.pause(id, *hovered, false, w, c)))
            .child(icon)
            .child(text);
            card = if exiting {
                card.child(div().size(px(24.)).flex_shrink_0())
            } else {
                card.child(
                    close.child(
                        svg()
                            .path(GpuiIcons::LucideX.path())
                            .size(px(14.))
                            .text_color(color(&styles.close.foreground)),
                    ),
                )
            };
            let offset = px(14. * (1. - progress)) * if self.placement.top() { -1. } else { 1. };
            cards.push((
                id,
                div().child(div().relative().top(offset).opacity(progress).child(card)),
            ));
        }
        let mut stack = animated_layout("toast-stack")
            .duration(self.effective_duration(window))
            .enabled(animate)
            .absolute()
            .w(width)
            .flex()
            .flex_col()
            .gap(px(10.));
        stack = if self.placement.top() {
            stack.top(self.margin)
        } else {
            stack.bottom(self.margin)
        };
        stack = match self.placement {
            ToastPlacement::TopLeft | ToastPlacement::BottomLeft => stack.left(self.margin),
            ToastPlacement::Top | ToastPlacement::Bottom => {
                stack.left((window.viewport_size().width - width) / 2.)
            }
            _ => stack.right(self.margin),
        };
        if !self.placement.top() {
            cards.reverse();
        }
        for (id, card) in cards {
            stack = stack.item(id.0 as usize, card);
        }
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(window.viewport_size().width)
                .h(window.viewport_size().height)
                .child(stack)
                .into_any_element(),
        )
    }
}
impl Styled for ToastHost {
    type Part = ToastPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache.clear();
        &mut self.style
    }
}
impl Render for ToastHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = cx.weak_entity();
        deferred_overlay(move |window, cx| {
            host.update(cx, |host, cx| host.overlay(window, cx))
                .ok()
                .flatten()
        })
        .with_priority(300)
    }
}
#[cfg(test)]
mod tests;
