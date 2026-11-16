use crate::{
    components::selection_surface::{color, selection_surface},
    style::{ComputedStyle, MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    AccessibleAction, AnyView, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, Render, Role, SharedString, Styled as _, Subscription, Transformation, Window,
    div, prelude::*, px, radians, svg,
};
use gpui_icons::GpuiIcons;
use std::{collections::BTreeSet, rc::Rc, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccordionMode {
    #[default]
    Single,
    Multiple,
}

/// A stable disclosure identity and its retained content view.
#[derive(Clone)]
pub struct AccordionItem {
    value: SharedString,
    label: SharedString,
    content: AnyView,
    icon: Option<SharedString>,
    disabled: bool,
}
impl AccordionItem {
    pub fn new(
        value: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: impl Into<AnyView>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            content: content.into(),
            icon: None,
            disabled: false,
        }
    }
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
#[derive(Clone, Copy)]
pub enum AccordionPart {
    Item,
    Header,
    Label,
    Icon,
    Indicator,
    Content,
}
impl Part for AccordionPart {
    fn name(&self) -> &str {
        match self {
            Self::Item => "item",
            Self::Header => "header",
            Self::Label => "label",
            Self::Icon => "icon",
            Self::Indicator => "indicator",
            Self::Content => "content",
        }
    }
}
struct Item {
    definition: AccordionItem,
    focus: FocusHandle,
    content_focus: FocusHandle,
}
type Change = Rc<dyn Fn(Vec<SharedString>, &mut Window, &mut App)>;
#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    items: Vec<[ComputedStyle; 6]>,
}
struct Cache {
    revision: u64,
    material: MaterialMode,
    focused: Option<usize>,
    styles: Styles,
}

/// Retained disclosure panels with intrinsic-height transitions.
pub struct Accordion {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    label: SharedString,
    items: Vec<Item>,
    expanded: BTreeSet<usize>,
    mode: AccordionMode,
    fallback_focus: FocusHandle,
    hovered: Option<usize>,
    pressed: Option<usize>,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    duration: Duration,
    animated: bool,
    on_change: Option<Change>,
    cache: Option<Cache>,
    error: Option<StyleError>,
    _subscriptions: Vec<Subscription>,
}
impl Accordion {
    /// Values must be unique and nonempty; all sections start collapsed.
    pub fn new(
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = AccordionItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let items: Vec<_> = items.into_iter().collect();
        let mut values = BTreeSet::new();
        if items.iter().any(|i| {
            i.value.trim().is_empty()
                || i.label.trim().is_empty()
                || !values.insert(i.value.clone())
        }) {
            return Err(StyleError::new(
                "accordion.items",
                "values must be unique and nonempty; labels must not be empty",
            ));
        }
        let theme = ThemeRuntime::global(cx);
        let mut subscriptions = vec![cx.observe(&theme, |_, _, cx| cx.notify())];
        let items = items
            .into_iter()
            .map(|definition| {
                let focus = cx.focus_handle();
                subscriptions.push(cx.on_focus(&focus, window, |_, _, cx| cx.notify()));
                subscriptions.push(cx.on_blur(&focus, window, |this, _, cx| {
                    this.pressed = None;
                    this.invalidate(cx);
                }));
                Item {
                    definition,
                    focus,
                    content_focus: cx.focus_handle(),
                }
            })
            .collect();
        Ok(Self {
            theme,
            material: MaterialBinding::new(cx),
            label: label.into(),
            items,
            expanded: BTreeSet::new(),
            mode: AccordionMode::Single,
            fallback_focus: cx.focus_handle(),
            hovered: None,
            pressed: None,
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            duration: Duration::from_millis(320),
            animated: true,
            on_change: None,
            cache: None,
            error: None,
            _subscriptions: subscriptions,
        })
    }
    pub fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }
    pub fn expanded(&self) -> Vec<SharedString> {
        self.expanded
            .iter()
            .map(|i| self.items[*i].definition.value.clone())
            .collect()
    }
    /// Validates the whole selection before mutation; programmatic changes do not emit `on_change`.
    /// Closing focused content returns focus to its section header.
    pub fn set_expanded<S: AsRef<str>>(
        &mut self,
        values: impl IntoIterator<Item = S>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let mut expanded = BTreeSet::new();
        for value in values {
            let index = self
                .items
                .iter()
                .position(|item| {
                    item.definition.value == value.as_ref() && !item.definition.disabled
                })
                .ok_or_else(|| {
                    StyleError::new("accordion.expanded", "values must name enabled sections")
                })?;
            if !expanded.insert(index) {
                return Err(StyleError::new("accordion.expanded", "duplicate section"));
            }
        }
        if self.mode == AccordionMode::Single && expanded.len() > 1 {
            return Err(StyleError::new(
                "accordion.expanded",
                "single mode allows at most one expanded section",
            ));
        }
        self.apply(expanded, window, cx);
        Ok(())
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(Vec<SharedString>, &mut Window, &mut App) + 'static,
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
        self.invalidate(cx);
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.cache = None;
        cx.notify();
    }
    fn apply(&mut self, expanded: BTreeSet<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if self.expanded == expanded {
            return;
        }
        for index in self.expanded.difference(&expanded) {
            let item = &self.items[*index];
            if item.content_focus.contains_focused(window, cx) {
                item.focus.focus(window, cx);
            }
        }
        self.expanded = expanded;
        self.invalidate(cx);
    }
    fn toggle(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.items[index].definition.disabled {
            return;
        }
        self.items[index].focus.focus(window, cx);
        let mut expanded = self.expanded.clone();
        if !expanded.remove(&index) {
            if self.mode == AccordionMode::Single {
                expanded.clear();
            }
            expanded.insert(index);
        }
        self.apply(expanded, window, cx);
        if let Some(callback) = self.on_change.clone() {
            callback(self.expanded(), window, cx);
        }
    }
    fn navigate(
        &mut self,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let count = self.items.len();
        let next = match event.keystroke.key.as_str() {
            "up" => (1..=count)
                .map(|step| (index + count - step) % count)
                .find(|i| !self.items[*i].definition.disabled),
            "down" => (1..=count)
                .map(|step| (index + step) % count)
                .find(|i| !self.items[*i].definition.disabled),
            "home" => self.items.iter().position(|i| !i.definition.disabled),
            "end" => self.items.iter().rposition(|i| !i.definition.disabled),
            "enter" | "space" => {
                if !event.is_held {
                    self.toggle(index, window, cx);
                }
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        if let Some(index) = next {
            self.items[index].focus.focus(window, cx);
        }
        cx.stop_propagation();
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let focused = window
            .last_input_was_keyboard()
            .then(|| self.items.iter().position(|i| i.focus.is_focused(window)))
            .flatten();
        if let Some(cache) = &self.cache {
            if cache.revision == runtime.revision()
                && cache.material == material
                && cache.focused == focused
            {
                return Ok(cache.styles.clone());
            }
        }
        let mut context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style.validate(
            &context,
            &["item", "header", "label", "icon", "indicator", "content"],
        )?;
        let recipe = runtime.styles().recipe("accordion")?;
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
        let items = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let disabled = item.definition.disabled;
                let states = StateSelector(
                    [
                        (State::Selected, self.expanded.contains(&index)),
                        (State::Disabled, disabled),
                        (State::Hovered, !disabled && self.hovered == Some(index)),
                        (State::Pressed, !disabled && self.pressed == Some(index)),
                        (State::FocusVisible, !disabled && focused == Some(index)),
                    ]
                    .into_iter()
                    .filter_map(|(s, active)| active.then_some(s))
                    .collect(),
                );
                Ok([
                    resolve(&states, Some("item"))?,
                    resolve(&states, Some("header"))?,
                    resolve(&states, Some("label"))?,
                    resolve(&states, Some("icon"))?,
                    resolve(&states, Some("indicator"))?,
                    resolve(&states, Some("content"))?,
                ])
            })
            .collect::<Result<_, StyleError>>()?;
        let styles = Styles {
            root: resolve(&StateSelector::default(), None)?,
            items,
        };
        self.cache = Some(Cache {
            revision: runtime.revision(),
            material,
            focused,
            styles: styles.clone(),
        });
        Ok(styles)
    }
}
impl Styled for Accordion {
    type Part = AccordionPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Focusable for Accordion {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.items
            .iter()
            .find(|i| !i.definition.disabled)
            .map(|i| i.focus.clone())
            .unwrap_or_else(|| self.fallback_focus.clone())
    }
}
impl Render for Accordion {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let styles = match self.resolve(window, cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Accordion style error: {error}"))
                    .into_any_element();
            }
        };
        let animated = self.animated && !window.prefers_reduced_motion();
        let duration = self.duration;
        let mut root = div()
            .id("accordion")
            .role(Role::Group)
            .aria_label(self.label.clone())
            .flex()
            .flex_col()
            .min_w_0();
        root.style().refine(&styles.root.to_gpui());
        for (index, (item, [surface, header, label, icon, indicator, content])) in
            self.items.iter().zip(styles.items).enumerate()
        {
            let expanded = self.expanded.contains(&index);
            let disabled = item.definition.disabled;
            let mut trigger = div()
                .id(("header", index))
                .debug_selector(move || format!("fusion-accordion-header-{index}"))
                .role(Role::Button)
                .aria_label(item.definition.label.clone())
                .aria_expanded(expanded)
                .aria_disabled(disabled)
                .track_focus(&item.focus)
                .tab_stop(!disabled)
                .flex()
                .items_center()
                .min_w_0();
            trigger.style().refine(&header.to_gpui());
            if let Some(path) = &item.definition.icon {
                let mut graphic = svg()
                    .path(path.clone())
                    .size(px(18.))
                    .text_color(color(&header.foreground));
                graphic.style().refine(&icon.to_gpui());
                trigger = trigger.child(graphic);
            }
            let mut title = div()
                .flex_1()
                .min_w_0()
                .child(item.definition.label.clone());
            title.style().refine(&label.to_gpui());
            trigger = trigger.child(title);
            let arrow_color = indicator
                .to_gpui()
                .text
                .color
                .unwrap_or_else(|| color(&header.foreground));
            trigger = trigger.child(
                gpui_effects::animated_number(
                    ("chevron", index),
                    if expanded { 1. } else { 0. },
                    move |amount| {
                        let mut arrow = svg()
                            .path(GpuiIcons::LucideChevronRight.path())
                            .size(px(18.))
                            .text_color(arrow_color)
                            .with_transformation(Transformation::rotate(radians(
                                amount as f32 * std::f32::consts::FRAC_PI_2,
                            )));
                        arrow.style().refine(&indicator.to_gpui());
                        arrow
                    },
                )
                .duration(duration)
                .enabled(animated),
            );
            if !disabled {
                let weak = cx.weak_entity();
                trigger = trigger
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.toggle(index, window, cx)),
                    )
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered = hovered.then_some(index);
                        this.invalidate(cx);
                    }))
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        this.navigate(index, event, window, cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pressed = Some(index);
                            this.invalidate(cx);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed = None;
                            this.invalidate(cx);
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed = None;
                            this.invalidate(cx);
                        }),
                    )
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.toggle(index, window, cx));
                    });
            }
            let panel = item.definition.content.clone();
            let focus = item.content_focus.clone();
            let name = item.definition.label.clone();
            let body = gpui_effects::animated_collapse(("content", index), expanded, move || {
                let mut body = div()
                    .id("content")
                    .debug_selector(move || format!("fusion-accordion-content-{index}"))
                    .role(Role::Group)
                    .aria_label(name)
                    .track_focus(&focus)
                    .tab_stop(false)
                    .child(panel);
                body.style().refine(&content.to_gpui());
                body
            })
            .duration(duration)
            .enabled(animated);
            root = root.child(
                selection_surface(
                    div().flex().flex_col().min_w_0(),
                    surface.clone(),
                    surface,
                    true,
                    1.,
                    1.,
                    0.,
                )
                .child(trigger)
                .child(body),
            );
        }
        root.into_any_element()
    }
}
#[cfg(test)]
mod tests;
