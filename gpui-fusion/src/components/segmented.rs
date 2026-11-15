use crate::{
    components::motion::Spring,
    style::{
        ComputedStyle, MaterialMode, Part, Resolved, State, StateSelector, StylePatch, Styled,
    },
    theme::material_binding::MaterialBinding,
    theme::runtime::ThemeRuntime,
    theme::{ResolveContext, StyleError, ThemeScope},
};
use gpui::{Styled as _, prelude::*, *};
use gpui_effects::{LiquidGlassRegion, liquid_glass_content, paint_liquid_glass};
use std::{cell::Cell, collections::BTreeSet, rc::Rc, time::Instant};

/// A stable value with a visible label, an icon, or both. Hidden labels remain accessible.
#[derive(Clone, Debug)]
pub struct Segment {
    value: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    show_label: bool,
    disabled: bool,
}
impl Segment {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            show_label: true,
            disabled: false,
        }
    }
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
    pub fn icon_only(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self.show_label = false;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SegmentedPart {
    Item,
    Indicator,
}
impl Part for SegmentedPart {
    fn name(&self) -> &str {
        match self {
            Self::Item => "item",
            Self::Indicator => "indicator",
        }
    }
}

type Change = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
struct Drag {
    origin: Pixels,
    tapped: usize,
    position: f32,
    moved: bool,
}
struct ResolvedStyles {
    root: ComputedStyle,
    indicator: ComputedStyle,
    items: Vec<ComputedStyle>,
}

/// An equal-width, single-selection control with optional draggable glass material.
pub struct Segmented {
    theme: Entity<ThemeRuntime>,
    material_binding: MaterialBinding,
    items: Vec<Segment>,
    selected: Option<usize>,
    label: SharedString,
    focus: FocusHandle,
    disabled: bool,
    hovered: Option<usize>,
    drag: Option<Drag>,
    style: StylePatch,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    on_change: Option<Change>,
    _subscriptions: Vec<Subscription>,
    position: Spring,
    pressure: Spring,
    last_frame: Instant,
    animated: bool,
    revision: Option<u64>,
    resolved: Option<ResolvedStyles>,
    resolved_material: Option<MaterialMode>,
    error: Option<StyleError>,
}
impl Segmented {
    /// Values must be unique and nonempty. The first enabled item is initially selected.
    pub fn new(
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = Segment>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let items: Vec<_> = items.into_iter().collect();
        let mut values = BTreeSet::new();
        for item in &items {
            if item.value.trim().is_empty()
                || item.label.trim().is_empty()
                || !values.insert(item.value.clone())
            {
                return Err(StyleError::new(
                    "segmented.items",
                    "values must be unique and nonempty; labels must not be empty",
                ));
            }
        }
        let selected = items.iter().position(|item| !item.disabled);
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |this: &mut Self, theme, cx| {
                if this.revision != Some(theme.read(cx).revision()) {
                    this.invalidate(cx);
                }
            }),
            cx.on_focus(&focus, window, |this, _, cx| this.invalidate(cx)),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.drag = None;
                this.invalidate(cx);
            }),
        ];
        Ok(Self {
            theme,
            material_binding: MaterialBinding::new(cx),
            items,
            selected,
            label: label.into(),
            focus,
            disabled: false,
            hovered: None,
            drag: None,
            style: StylePatch::default(),
            variant: None,
            scopes: Vec::new(),
            on_change: None,
            _subscriptions: subscriptions,
            position: Spring {
                value: selected.unwrap_or(0) as f32,
                velocity: 0.,
            },
            pressure: Spring::default(),
            last_frame: Instant::now(),
            animated: true,
            revision: None,
            resolved: None,
            resolved_material: None,
            error: None,
        })
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material_binding.fix(mode);
        self
    }
    /// None follows the application; Some fixes the material for this control.
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material_binding.set(mode, cx);
        self.invalidate(cx);
    }
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
        self.resolved = None;
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.resolved = None;
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.map(|index| &self.items[index].value)
    }
    /// Selects an enabled value without emitting a user callback.
    pub fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) -> Result<(), StyleError> {
        let index = self
            .items
            .iter()
            .position(|item| item.value.as_ref() == value && !item.disabled)
            .ok_or_else(|| {
                StyleError::new("segmented.selection", "value is missing or disabled")
            })?;
        let cancelled = self.drag.take().is_some();
        if self.selected != Some(index) || cancelled {
            self.selected = Some(index);
            self.invalidate(cx);
        }
        Ok(())
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            self.drag = None;
            self.hovered = None;
            self.invalidate(cx);
        }
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.invalidate(cx);
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.resolved = None;
        cx.notify();
    }
    fn commit(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.disabled && !self.items[index].disabled && self.selected != Some(index) {
            self.selected = Some(index);
            self.invalidate(cx);
            if let Some(callback) = self.on_change.clone() {
                callback(self.items[index].value.clone(), window, cx);
            }
        }
    }
    fn nearest(&self, position: f32) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.disabled)
            .min_by(|(a, _), (b, _)| {
                ((*a as f32 - position).abs()).total_cmp(&(*b as f32 - position).abs())
            })
            .map(|(index, _)| index)
    }
    fn release(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            let index = if drag.moved {
                self.nearest(drag.position)
            } else {
                Some(drag.tapped)
            };
            if let Some(index) = index {
                self.commit(index, window, cx);
            }
            self.invalidate(cx);
        }
    }
    fn key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if key == "escape" {
            if self.drag.take().is_some() {
                self.invalidate(cx);
                cx.stop_propagation();
            }
            return;
        }
        let enabled: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| (!item.disabled).then_some(i))
            .collect();
        if enabled.is_empty() {
            return;
        }
        let current = enabled
            .iter()
            .position(|i| Some(*i) == self.selected)
            .unwrap_or(0);
        let next = match key {
            "left" | "up" => (current + enabled.len() - 1) % enabled.len(),
            "right" | "down" => (current + 1) % enabled.len(),
            "home" => 0,
            "end" => enabled.len() - 1,
            _ => return,
        };
        self.drag = None;
        self.commit(enabled[next], window, cx);
        self.invalidate(cx);
        cx.stop_propagation();
    }
    fn resolve(&self, window: &Window, cx: &App) -> Result<ResolvedStyles, StyleError> {
        let runtime = self.theme.read(cx);
        let store = runtime.store();
        let recipe = runtime.styles().recipe("segmented")?;
        if recipe
            .parts
            .iter()
            .any(|part| part != "item" && part != "indicator")
        {
            return Err(StyleError::new(
                "segmented.parts",
                "unsupported segmented part",
            ));
        }
        let mut context = ResolveContext::new(store.theme(), store.mode(), runtime.style_set())?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style.validate(&context, &["item", "indicator"])?;
        let mut states = StateSelector::default();
        if self.disabled {
            states.0.insert(State::Disabled);
        }
        if self.drag.is_some() {
            states.0.insert(State::Pressed);
        }
        if !self.disabled && self.focus.is_focused(window) && window.last_input_was_keyboard() {
            states.0.insert(State::FocusVisible);
        }
        let resolve = |states: &StateSelector, part| {
            recipe.resolve(
                self.variant.as_deref(),
                self.material_binding.mode(cx),
                &self.style,
                &context,
                states,
                part,
            )
        };
        let root = resolve(&states, None)?;
        let indicator = resolve(&states, Some("indicator"))?;
        let items = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut states = states.clone();
                if Some(index) == self.selected {
                    states.0.insert(State::Selected);
                }
                if item.disabled {
                    states.0.insert(State::Disabled);
                }
                if !self.disabled && !item.disabled && self.hovered == Some(index) {
                    states.0.insert(State::Hovered);
                }
                resolve(&states, Some("item"))
            })
            .collect::<Result<_, _>>()?;
        Ok(ResolvedStyles {
            root,
            indicator,
            items,
        })
    }
}
impl Styled for Segmented {
    type Part = SegmentedPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.resolved = None;
        &mut self.style
    }
}
impl Focusable for Segmented {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Segmented {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Material changes have a separate signal and do not increment the color revision.
        let material = self.material_binding.mode(cx);
        if self.resolved_material != Some(material) {
            self.resolved = None;
            self.resolved_material = Some(material);
        }
        if self.resolved.is_none() {
            match self.resolve(window, cx) {
                Ok(styles) => {
                    self.resolved = Some(styles);
                    self.error = None;
                    self.revision = Some(self.theme.read(cx).revision());
                }
                Err(error) => {
                    self.error = Some(error.clone());
                    return div()
                        .child(format!("Segmented style error: {error}"))
                        .into_any_element();
                }
            }
        }
        let styles = self.resolved.as_ref().unwrap();
        let root_style = styles.root.to_gpui();
        let indicator = styles.indicator.clone();
        let target = self
            .drag
            .as_ref()
            .filter(|d| d.moved)
            .map_or(self.selected.unwrap_or(0) as f32, |d| d.position);
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.032);
        self.last_frame = now;
        let pressed = f32::from(
            !self.disabled
                && self.selected.is_some()
                && (self.drag.is_some() || (self.position.value - target).abs() > 0.06),
        );
        if self.animated {
            self.position.advance(
                target,
                dt,
                if self.drag.is_some() { 36. } else { 22. },
                0.85,
            );
            self.pressure.advance(pressed, dt, 24., 0.8);
        } else {
            self.position = Spring {
                value: target,
                velocity: 0.,
            };
            self.pressure = Spring {
                value: f32::from(self.drag.is_some()),
                velocity: 0.,
            };
        }
        if self.animated
            && (self.position.value != target
                || self.position.velocity != 0.
                || self.pressure.value != pressed
                || self.pressure.velocity != 0.)
        {
            window.request_animation_frame();
        }
        let count = self.items.len();
        let position = self
            .position
            .value
            .clamp(0., count.saturating_sub(1) as f32);
        let pressure = self.pressure.value.clamp(0., 1.);
        let selected = self.selected;
        let refracting = indicator.material.is_some() && pressure > 0.;
        let outline = indicator.clone();
        let region = Rc::new(Cell::new(None));
        let lens = Rc::new(Cell::new(None));
        let painted_region = region.clone();
        let painted_lens = lens.clone();
        let events = cx.weak_entity();
        let insets = [
            length(&styles.root.padding_left, 0.),
            length(&styles.root.padding_right, 0.),
            length(&styles.root.padding_top, 0.),
            length(&styles.root.padding_bottom, 0.),
        ];
        let mut element = div()
            .on_paint_before_children(move |bounds, _, window, _| {
                let inner = Bounds::new(
                    bounds.origin + point(insets[0], insets[2]),
                    size(
                        (bounds.size.width - insets[0] - insets[1]).max(px(0.)),
                        (bounds.size.height - insets[2] - insets[3]).max(px(0.)),
                    ),
                );
                if count > 0 && selected.is_some() {
                    let width = inner.size.width / count as f32;
                    let center = point(inner.left() + width * (position + 0.5), inner.center().y);
                    let scale = if indicator.material.is_some() {
                        pressure
                    } else {
                        0.
                    };
                    let dimensions = size(
                        width * (1. + 0.10 * scale),
                        inner.size.height * (1. + 0.30 * scale),
                    );
                    let pane = Bounds::new(
                        center - point(dimensions.width / 2., dimensions.height / 2.),
                        dimensions,
                    );
                    let corners =
                        Corners::all(length(&indicator.radius, dimensions.height.as_f32() / 2.));
                    painted_region.set(Some(LiquidGlassRegion::new(pane, corners)));
                    if let Some(mut glass) = indicator.material {
                        if pressure < 1. {
                            let mut rest = glass;
                            rest.refraction = px(0.);
                            rest.dispersion = 0.;
                            rest.optics.magnification = 0.;
                            rest.optics.rim_strength = 0.;
                            rest.optics.top_reflection = 0.;
                            rest.optics.inner_shading = 0.;
                            rest.optics.caustic_strength = 0.;
                            rest.shadow.strength = 0.;
                            rest.blur_radius = rest.blur_radius.max(px(4.));
                            rest.optics.center_blur = 1.;
                            window.with_subtree_effect_chain(pane, &[], 1. - pressure, |window| {
                                paint_liquid_glass(pane, corners, rest, window)
                            });
                        }
                        if pressure > 0. {
                            glass.frost = 0.;
                            glass.refraction *= pressure;
                            glass.dispersion *= pressure;
                            glass.optics.magnification = 0.06 * pressure;
                            glass.optics.rim_strength *= pressure;
                            glass.optics.top_reflection *= pressure;
                            glass.optics.inner_shading *= pressure;
                            glass.optics.caustic_strength *= pressure;
                            glass.shadow.strength *= pressure;
                            glass.blur_radius *= pressure;
                            painted_lens.set(Some((pane, corners, glass)));
                        }
                    } else {
                        window.paint_quad(quad(
                            pane,
                            corners,
                            indicator.background_gradient.clone().unwrap_or_else(|| {
                                color(&indicator.background, transparent_black()).into()
                            }),
                            px(0.),
                            transparent_black(),
                            BorderStyle::Solid,
                        ));
                    }
                }
                let moving = events.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let Some(entity) = moving.upgrade() else {
                        return;
                    };
                    if entity.read(cx).drag.is_none() {
                        return;
                    }
                    entity.update(cx, |this, cx| {
                        if event.pressed_button != Some(MouseButton::Left) {
                            this.drag = None;
                            this.invalidate(cx);
                            return;
                        }
                        if let Some(drag) = &mut this.drag {
                            if (event.position.x - drag.origin).abs() >= px(3.) {
                                drag.moved = true;
                            }
                            if drag.moved && inner.size.width > px(0.) {
                                let next = ((event.position.x - inner.left()).as_f32()
                                    / inner.size.width.as_f32()
                                    * count as f32
                                    - 0.5)
                                    .clamp(0., count.saturating_sub(1) as f32);
                                if drag.position != next {
                                    drag.position = next;
                                    cx.notify();
                                }
                            }
                        }
                    });
                });
                let releasing = events.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        if let Some(entity) = releasing.upgrade() {
                            if entity.read(cx).drag.is_some() {
                                entity.update(cx, |this, cx| this.release(window, cx));
                            }
                        }
                    }
                });
            })
            .id("segmented")
            .debug_selector(|| "fusion-segmented".into())
            .track_focus(&self.focus)
            .tab_stop(!self.disabled && self.selected.is_some())
            .role(Role::RadioGroup)
            .aria_label(self.label.clone())
            .aria_disabled(self.disabled)
            .relative()
            .flex()
            .flex_shrink_0();
        element.style().refine(&root_style);
        for (index, item) in self.items.iter().enumerate() {
            let computed = &styles.items[index];
            let disabled = self.disabled || item.disabled;
            let region = region.clone();
            let foreground = color(&computed.foreground, rgb(0x202126).into());
            let selected_color = if disabled {
                foreground
            } else {
                color(&styles.indicator.foreground, foreground)
            };
            let mut content = div()
                .flex()
                .items_center()
                .justify_center()
                .gap(length(&computed.gap, 6.))
                .min_w_0()
                .overflow_hidden();
            if let Some(icon) = &item.icon {
                content = content.child(
                    svg()
                        .path(icon.clone())
                        .size(length(&computed.font_size, 14.) * 1.15)
                        .flex_shrink_0()
                        .text_color(foreground),
                );
            }
            if item.show_label {
                content = content.child(div().truncate().child(item.label.clone()));
            }
            let activate = cx.weak_entity();
            let mut option = div()
                .id(("segment", index))
                .debug_selector(move || format!("fusion-segment-{index}"))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .font_weight(FontWeight::MEDIUM)
                .role(Role::RadioButton)
                .aria_label(item.label.clone())
                .aria_toggled((self.selected == Some(index)).into())
                .aria_disabled(disabled)
                .when(self.selected == Some(index), |div| {
                    div.aria_active_descendant()
                })
                .child(if refracting {
                    liquid_glass_content(content, selected_color, move |_, _| region.get())
                        .into_any_element()
                } else {
                    content.into_any_element()
                });
            option.style().refine(&computed.to_gpui());
            if !disabled {
                option = option
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        let next = if *hovered {
                            Some(index)
                        } else {
                            this.hovered.filter(|i| *i != index)
                        };
                        if this.hovered != next {
                            this.hovered = next;
                            this.invalidate(cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.focus.focus(window, cx);
                            this.drag = Some(Drag {
                                origin: event.position.x,
                                tapped: index,
                                position: this.position.value,
                                moved: false,
                            });
                            this.invalidate(cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if !matches!(event, ClickEvent::Mouse(_)) {
                            this.commit(index, window, cx);
                        }
                    }))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = activate.update(cx, |this, cx| this.commit(index, window, cx));
                    });
            }
            element = element.child(option);
        }
        element = element.child(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    if let Some((pane, corners, glass)) = lens.get() {
                        paint_liquid_glass(pane, corners, glass, window);
                    }
                    if let Some(region) = region.get() {
                        let width = length(&outline.border_width, 0.);
                        if width > px(0.) {
                            let mut border = quad(
                                region.bounds,
                                region.corner_radii,
                                transparent_black(),
                                width,
                                color(&outline.border_color, transparent_black()),
                                outline.to_gpui().border_style.unwrap_or_default(),
                            );
                            if let Some(gradient) = &outline.border_gradient {
                                border.border_gradient = gradient.clone();
                            }
                            window.paint_quad(border);
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        );
        if !self.disabled {
            element = element.on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !event.keystroke.modifiers.modified() {
                    this.key(&event.keystroke.key, window, cx);
                }
            }));
        }
        element.into_any_element()
    }
}
fn length(value: &Resolved<Pixels>, default: f32) -> Pixels {
    match value {
        Resolved::Value(value) => *value,
        _ => px(default),
    }
}
fn color(value: &Resolved<Hsla>, default: Hsla) -> Hsla {
    match value {
        Resolved::Value(value) => *value,
        Resolved::Clear => transparent_black(),
        _ => default,
    }
}
