use super::glass::GlassMenu;
use super::motion::{MenuMotion, phase};
use super::placement::DropdownPlacement;
use super::presentation::{DropdownPresentation, MorphMenu, PlacedMenu, paint_glass_surface};
use crate::{
    components::button::Button,
    style::{ComputedStyle, MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::{ResolveContext, StyleError, material_binding::MaterialBinding, runtime::ThemeRuntime},
};
use gpui::{Styled as _, prelude::*, *};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

/// A stable menu command or a separator between command groups.
#[derive(Clone)]
pub enum MenuItem {
    Action {
        value: SharedString,
        label: SharedString,
        icon: Option<SharedString>,
        disabled: bool,
    },
    Separator,
}
impl MenuItem {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::Action {
            value: value.into(),
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        if let Self::Action { icon, .. } = &mut self {
            *icon = Some(path.into());
        }
        self
    }
    pub fn disabled(mut self, value: bool) -> Self {
        if let Self::Action { disabled, .. } = &mut self {
            *disabled = value;
        }
        self
    }
    fn enabled(&self) -> bool {
        matches!(
            self,
            Self::Action {
                disabled: false,
                ..
            }
        )
    }
}

#[derive(Clone, Copy)]
pub enum DropdownPart {
    Item,
    Icon,
    Separator,
}
impl Part for DropdownPart {
    fn name(&self) -> &str {
        match self {
            Self::Item => "item",
            Self::Icon => "icon",
            Self::Separator => "separator",
        }
    }
}
type Action = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// An anchored command menu with an animated, persistent button trigger.
pub struct Dropdown {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    trigger: Entity<Button>,
    closed_label: SharedString,
    presentation: DropdownPresentation,
    placement: Option<DropdownPlacement>,
    open_label: Option<SharedString>,
    items: Vec<MenuItem>,
    open: bool,
    disabled: bool,
    active: Option<usize>,
    focus: FocusHandle,
    anchor: ElementBounds,
    scroll: ScrollHandle,
    duration: Duration,
    motion: MenuMotion,
    on_action: Option<Action>,
    error: Option<StyleError>,
    _subscriptions: Vec<Subscription>,
}
impl Dropdown {
    pub fn new(
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = MenuItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let label = label.into();
        let owner = cx.weak_entity();
        let trigger = cx.new(|cx| {
            let mut button = Button::new(label.clone(), window, cx).on_click(move |window, cx| {
                let owner = owner.clone();
                window.defer(cx, move |window, cx| {
                    let _ = owner.update(cx, |menu: &mut Self, cx| {
                        menu.set_open(!menu.open, true, window, cx)
                    });
                });
            });
            button.set_expanded(false, cx);
            button
        });
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |this, window, cx| {
                if this.open && !this.trigger.focus_handle(cx).is_focused(window) {
                    this.set_open(false, false, window, cx);
                }
            }),
        ];
        Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            trigger,
            closed_label: label,
            presentation: DropdownPresentation::default(),
            placement: None,
            open_label: None,
            items: items.into_iter().collect(),
            open: false,
            disabled: false,
            active: None,
            focus,
            anchor: ElementBounds::default(),
            scroll: ScrollHandle::new(),
            duration: Duration::from_millis(600),
            motion: MenuMotion::new(Instant::now()),
            on_action: None,
            error: None,
            _subscriptions: subscriptions,
        }
    }
    pub fn presentation(mut self, presentation: DropdownPresentation) -> Self {
        self.presentation = presentation;
        self
    }
    pub fn placement(mut self, placement: DropdownPlacement) -> Self {
        self.placement = Some(placement);
        self
    }
    pub fn set_placement(&mut self, placement: Option<DropdownPlacement>, cx: &mut Context<Self>) {
        self.placement = placement;
        cx.notify();
    }
    /// Sets the trigger label while the menu is expanded.
    pub fn open_label(mut self, label: impl Into<SharedString>) -> Self {
        self.open_label = Some(label.into());
        self
    }
    /// Sets the menu and trigger transition duration; zero disables motion.
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    pub fn on_action(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(callback));
        self
    }
    /// Returns the persistent button for trigger-specific styling.
    pub fn trigger(&self) -> &Entity<Button> {
        &self.trigger
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(true, true, window, cx);
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(false, true, window, cx);
    }
    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.trigger
            .update(cx, |button, cx| button.set_disabled(disabled, cx));
        if disabled {
            self.set_open(false, false, window, cx);
            if self.focus.is_focused(window) {
                window.blur();
            }
        }
        cx.notify();
    }
    /// Fixes both the trigger and menu material; None resumes the application theme.
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
        self.trigger
            .update(cx, |button, cx| button.set_material_mode(mode, cx));
    }
    fn progress(&mut self) -> f32 {
        self.motion.sample(self.open, Instant::now(), self.duration)
    }
    fn set_open(
        &mut self,
        open: bool,
        restore_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if open == self.open || (open && self.disabled) {
            return;
        }
        self.progress();
        self.open = open;
        self.progress();
        let label = if open && self.presentation == DropdownPresentation::Popover {
            self.open_label.as_ref().unwrap_or(&self.closed_label)
        } else {
            &self.closed_label
        }
        .clone();
        self.trigger.update(cx, |button, cx| {
            button.set_transition_duration(self.duration, cx);
            button.set_label(label, cx);
            button.set_expanded(open, cx);
        });
        if open {
            self.active = self.items.iter().position(MenuItem::enabled);
            if let Some(index) = self.active {
                self.scroll.scroll_to_item(index);
            }
            self.focus.focus(window, cx);
        } else if restore_focus {
            self.trigger.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.disabled {
            return;
        }
        let Some(MenuItem::Action {
            value,
            disabled: false,
            ..
        }) = self.items.get(index)
        else {
            return;
        };
        let value = value.clone();
        self.close(window, cx);
        if let Some(callback) = &self.on_action {
            callback(value, window, cx);
        }
    }
    fn navigate(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let n = self.items.len();
        for offset in 1..=n {
            let start = self.active.unwrap_or(if backwards { 0 } else { n - 1 });
            let next = if backwards {
                (start + n - offset) % n
            } else {
                (start + offset) % n
            };
            if self.items[next].enabled() {
                self.active = Some(next);
                self.scroll.scroll_to_item(next);
                cx.notify();
                break;
            }
        }
    }
    fn resolve(
        &self,
        part: Option<&str>,
        states: &StateSelector,
        cx: &App,
    ) -> Result<ComputedStyle, StyleError> {
        let runtime = self.theme.read(cx);
        let context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        if part.is_none() {
            self.style
                .validate(&context, &["item", "icon", "separator"])?;
        }
        runtime.styles().recipe("dropdown")?.resolve(
            None,
            self.material.mode(cx),
            &self.style,
            &context,
            states,
            part,
        )
    }
    fn render_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.anchor.visible_bounds(window).is_none() {
            if self.open {
                let owner = cx.weak_entity();
                window.defer(cx, move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.focus.is_focused(window) {
                            window.blur();
                        }
                        this.set_open(false, false, window, cx);
                    });
                });
            }
            return None;
        }
        let bounds = self.anchor.bounds(window)?;
        let mut root = match self.resolve(None, &StateSelector::default(), cx) {
            Ok(style) => {
                self.error = None;
                style
            }
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        let trigger_style = self
            .trigger
            .update(cx, |button, cx| button.surface_style(window, cx))
            .ok()?;
        root.material = trigger_style.material;
        root.background = trigger_style.background.clone();
        root.background_gradient = trigger_style.background_gradient.clone();
        let morph = self.presentation == DropdownPresentation::Morph;
        let fused = root.material.is_some();
        let progress = self.progress();
        if self.motion.active() {
            window.request_animation_frame();
        }
        let open = self.open;
        let anchor = self.anchor.clone();
        let glass = if morph || fused { None } else { root.material };
        let mut surface_style = root.clone();
        surface_style.material = glass;
        let mut surface = div()
            .on_paint_before_children(move |bounds, style, window, _| {
                paint_glass_surface(
                    &surface_style,
                    bounds,
                    style.corner_radii.to_pixels(window.rem_size()),
                    window,
                );
            })
            .id("dropdown-menu")
            .debug_selector(|| "fusion-dropdown-menu".into())
            .track_focus(&self.focus)
            .role(Role::Menu)
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.).min(window.viewport_size().width))
            .max_w(window.viewport_size().width)
            .max_h((window.viewport_size().height - px(24.)).clamp(px(1.), px(320.)))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);

        surface.style().refine(&root.to_gpui());
        let content_progress = if fused {
            phase(progress, 0.82, 1.)
        } else if morph {
            ((progress - 0.15) / 0.7).clamp(0., 1.)
        } else {
            progress
        };
        let opacity = surface.style().opacity.unwrap_or(1.) * content_progress;
        surface = surface.opacity(opacity);
        surface.style().background = Some(transparent_black().into());
        surface.style().border_color = Some(transparent_black());
        if open {
            surface = surface
                .on_mouse_down_out(
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if morph || !anchor.contains(event.position, window) {
                            this.set_open(false, false, window, cx);
                        }
                    }),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" => this.close(window, cx),
                        "up" => this.navigate(true, cx),
                        "down" => this.navigate(false, cx),
                        "home" => {
                            this.active = this.items.iter().position(MenuItem::enabled);
                            if let Some(index) = this.active {
                                this.scroll.scroll_to_item(index);
                            }
                            cx.notify();
                        }
                        "end" => {
                            this.active = this.items.iter().rposition(MenuItem::enabled);
                            if let Some(index) = this.active {
                                this.scroll.scroll_to_item(index);
                            }
                            cx.notify();
                        }
                        "enter" | "space" => {
                            if let Some(index) = this.active {
                                this.choose(index, window, cx);
                            }
                        }
                        "tab" => {
                            this.close(window, cx);
                            if event.keystroke.modifiers.shift {
                                window.focus_prev(cx);
                            } else {
                                window.focus_next(cx);
                            }
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                }));
        }
        for (index, item) in self.items.iter().enumerate() {
            let MenuItem::Action {
                label,
                icon,
                disabled,
                ..
            } = item
            else {
                let mut separator = div().h(px(1.)).my(px(4.));
                if let Ok(style) = self.resolve(Some("separator"), &StateSelector::default(), cx) {
                    separator.style().refine(&style.to_gpui());
                }
                surface = surface.child(separator);
                continue;
            };
            let states = if *disabled {
                State::Disabled.into()
            } else if self.active == Some(index) {
                State::Hovered.into()
            } else {
                StateSelector::default()
            };
            let style = self.resolve(Some("item"), &states, cx).ok()?;
            let mut row = div()
                .id(index)
                .debug_selector(move || format!("fusion-dropdown-item-{index}"))
                .role(Role::MenuItem)
                .aria_label(label.clone())
                .aria_disabled(*disabled)
                .flex()
                .items_center()
                .gap(px(8.))
                .flex_none();
            row.style().refine(&style.to_gpui());
            if self.active == Some(index) {
                row = row.aria_active_descendant();
            }
            if let Some(path) = icon {
                let mut image = svg().path(path.clone()).size(px(16.));
                if let Ok(style) = self.resolve(Some("icon"), &states, cx) {
                    image.style().refine(&style.to_gpui());
                }
                row = row.child(image);
            }
            row = row.child(label.clone());
            if open && !disabled {
                let weak = cx.weak_entity();
                row = row
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        if *hovered {
                            this.active = Some(index);
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose(index, window, cx);
                        cx.stop_propagation();
                    }))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.choose(index, window, cx));
                    });
            }
            surface = surface.child(row);
        }
        let placement = self.placement.unwrap_or_default();
        if fused {
            return Some(
                GlassMenu {
                    content: surface.into_any_element(),
                    trigger_content: self
                        .trigger
                        .update(cx, |button, cx| button.transition_content(window, cx))
                        .ok()?,
                    anchor: bounds,
                    progress,
                    placement,
                    trigger_radius: trigger_style
                        .to_gpui()
                        .corner_radii
                        .top_left
                        .unwrap_or_default()
                        .to_pixels(window.rem_size()),
                    trigger_scale: self
                        .trigger
                        .update(cx, |button, _| button.surface_scale(window)),
                    keep_trigger: !morph,
                    style: root,
                }
                .into_any_element(),
            );
        }
        if morph {
            let radius = trigger_style
                .to_gpui()
                .corner_radii
                .top_left
                .unwrap_or_default()
                .to_pixels(window.rem_size());
            return Some(
                MorphMenu {
                    content: surface.into_any_element(),
                    trigger_content: self
                        .trigger
                        .update(cx, |button, cx| button.transition_content(window, cx))
                        .ok()?,
                    anchor: bounds,
                    progress,
                    placement,
                    trigger_radius: radius,
                    style: root,
                }
                .into_any_element(),
            );
        }
        Some(
            PlacedMenu {
                content: surface.into_any_element(),
                settled_trigger: self
                    .trigger
                    .update(cx, |button, cx| {
                        button.settled_content(
                            self.open_label
                                .as_ref()
                                .unwrap_or(&self.closed_label)
                                .clone(),
                            window,
                            cx,
                        )
                    })
                    .ok()?,
                anchor: bounds,
                placement,
                progress,
                style: root,
            }
            .into_any_element(),
        )
    }
}
impl Styled for Dropdown {
    type Part = DropdownPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        &mut self.style
    }
}
impl Focusable for Dropdown {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.trigger.focus_handle(cx)
    }
}
impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let progress = self.progress();
        let visible = self.open || self.motion.active() || progress > 0.;
        if self.motion.active() {
            window.request_animation_frame();
        }
        let anchor = self.anchor.clone();
        let owner = cx.weak_entity();
        let overlay = visible.then(|| {
            deferred_overlay(move |window, cx| {
                owner
                    .update(cx, |this, cx| this.render_menu(window, cx))
                    .ok()
                    .flatten()
            })
            .with_priority(100)
        });
        let morph = self.presentation == DropdownPresentation::Morph && visible;
        let glass = visible
            && self
                .trigger
                .update(cx, |button, cx| button.surface_style(window, cx))
                .is_ok_and(|style| style.material.is_some());
        let trigger_opacity = if morph || glass { 0. } else { 1. };
        div()
            .relative()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.open && matches!(event.keystroke.key.as_str(), "up" | "down") {
                    this.open(window, cx);
                    if event.keystroke.key == "up" {
                        this.active = this.items.iter().rposition(MenuItem::enabled);
                    }
                    if let Some(index) = this.active {
                        this.scroll.scroll_to_item(index);
                    }
                    cx.stop_propagation();
                }
            }))
            .child(div().opacity(trigger_opacity).child(self.trigger.clone()))
            .child(
                canvas(
                    move |bounds, window, _| window.track_element_bounds(&anchor, bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .children(overlay)
    }
}
