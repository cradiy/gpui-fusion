use crate::{
    components::selection_surface::{color, length, selection_surface},
    style::{ComputedStyle, MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    AccessibleAction, AnyView, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Render,
    Role, ScrollHandle, SharedString, Styled as _, Subscription, Window, div, prelude::*, px, svg,
};
use std::{collections::BTreeSet, rc::Rc, time::Duration};

/// A stable page identity with an optional retained content view.
#[derive(Clone)]
pub struct Tab {
    value: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    disabled: bool,
    panel: Option<AnyView>,
}
impl Tab {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            disabled: false,
            panel: None,
        }
    }
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Hidden panels retain their entity state but are not laid out or painted.
    pub fn panel(mut self, panel: impl Into<AnyView>) -> Self {
        self.panel = Some(panel.into());
        self
    }
}
#[derive(Clone, Copy)]
pub enum TabsPart {
    List,
    Tab,
    Indicator,
    Icon,
    Panel,
}
impl Part for TabsPart {
    fn name(&self) -> &str {
        match self {
            Self::List => "list",
            Self::Tab => "tab",
            Self::Indicator => "indicator",
            Self::Icon => "icon",
            Self::Panel => "panel",
        }
    }
}
type Change = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
struct Item {
    tab: Tab,
    focus: FocusHandle,
}
#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    list: ComputedStyle,
    panel: ComputedStyle,
    items: Vec<[ComputedStyle; 4]>,
}
struct Cache {
    revision: u64,
    material: MaterialMode,
    focused: Option<usize>,
    styles: Styles,
}

/// Horizontal page navigation with roving focus and retained panels.
pub struct Tabs {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    items: Vec<Item>,
    selected: Option<usize>,
    label: SharedString,
    fallback_focus: FocusHandle,
    panel_focus: FocusHandle,
    scroll: ScrollHandle,
    manual_activation: bool,
    hovered: Option<usize>,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    on_change: Option<Change>,
    cache: Option<Cache>,
    error: Option<StyleError>,
    animated: bool,
    transition_duration: Duration,
    reveal: Option<usize>,
    _subscriptions: Vec<Subscription>,
}
impl Tabs {
    /// Values must be unique and nonempty; the first enabled tab is selected.
    pub fn new(
        label: impl Into<SharedString>,
        tabs: impl IntoIterator<Item = Tab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let tabs: Vec<_> = tabs.into_iter().collect();
        let mut values = BTreeSet::new();
        if tabs.iter().any(|tab| {
            tab.value.trim().is_empty()
                || tab.label.trim().is_empty()
                || !values.insert(tab.value.clone())
        }) {
            return Err(StyleError::new(
                "tabs.items",
                "values must be unique and nonempty; labels must not be empty",
            ));
        }
        let selected = tabs.iter().position(|tab| !tab.disabled);
        let theme = ThemeRuntime::global(cx);
        let mut subscriptions = vec![cx.observe(&theme, |_, _, cx| cx.notify())];
        let items = tabs
            .into_iter()
            .map(|tab| {
                let focus = cx.focus_handle();
                subscriptions.push(cx.on_focus(&focus, window, |_, _, cx| cx.notify()));
                subscriptions.push(cx.on_blur(&focus, window, |_, _, cx| cx.notify()));
                Item { tab, focus }
            })
            .collect();
        Ok(Self {
            theme,
            material: MaterialBinding::new(cx),
            items,
            selected,
            label: label.into(),
            fallback_focus: cx.focus_handle(),
            panel_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            manual_activation: false,
            hovered: None,
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            on_change: None,
            cache: None,
            error: None,
            animated: true,
            transition_duration: Duration::from_millis(320),
            reveal: selected,
            _subscriptions: subscriptions,
        })
    }
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.map(|i| &self.items[i].tab.value)
    }
    /// Changes the visible panel without moving focus or invoking `on_change`.
    pub fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) -> Result<(), StyleError> {
        let index = self
            .items
            .iter()
            .position(|i| i.tab.value == value && !i.tab.disabled)
            .ok_or_else(|| StyleError::new("tabs.selected", "value must name an enabled tab"))?;
        if self.selected != Some(index) {
            self.selected = Some(index);
            self.reveal = Some(index);
            self.invalidate(cx);
        }
        Ok(())
    }
    /// With manual activation, arrows move focus and Enter or Space selects.
    pub fn manual_activation(mut self, manual: bool) -> Self {
        self.manual_activation = manual;
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.transition_duration = duration;
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
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
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
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
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.items[index].tab.disabled {
            return;
        }
        self.items[index].focus.focus(window, cx);
        self.reveal = Some(index);
        if self.selected != Some(index) {
            self.selected = Some(index);
            self.invalidate(cx);
            if let Some(callback) = self.on_change.clone() {
                callback(self.items[index].tab.value.clone(), window, cx);
            }
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
            "left" => (1..=count)
                .map(|step| (index + count - step) % count)
                .find(|i| !self.items[*i].tab.disabled),
            "right" => (1..=count)
                .map(|step| (index + step) % count)
                .find(|i| !self.items[*i].tab.disabled),
            "home" => self.items.iter().position(|i| !i.tab.disabled),
            "end" => self.items.iter().rposition(|i| !i.tab.disabled),
            "enter" | "space" => {
                self.activate(index, window, cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        if let Some(next) = next {
            self.items[next].focus.focus(window, cx);
            self.reveal = Some(next);
            if !self.manual_activation {
                self.activate(next, window, cx);
            }
            cx.notify();
        }
        cx.stop_propagation();
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let focused = window
            .last_input_was_keyboard()
            .then(|| {
                self.items
                    .iter()
                    .position(|item| item.focus.is_focused(window))
            })
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
        self.style
            .validate(&context, &["list", "tab", "indicator", "icon", "panel"])?;
        let recipe = runtime.styles().recipe("tabs")?;
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
                let states = StateSelector(
                    [
                        (State::Disabled, item.tab.disabled),
                        (
                            State::Hovered,
                            !item.tab.disabled && self.hovered == Some(index),
                        ),
                        (
                            State::FocusVisible,
                            !item.tab.disabled && focused == Some(index),
                        ),
                    ]
                    .into_iter()
                    .filter_map(|(s, active)| active.then_some(s))
                    .collect(),
                );
                let mut selected = states.clone();
                selected.0.insert(State::Selected);
                Ok([
                    resolve(&states, Some("tab"))?,
                    resolve(&selected, Some("tab"))?,
                    resolve(&selected, Some("indicator"))?,
                    resolve(&states, Some("icon"))?,
                ])
            })
            .collect::<Result<_, StyleError>>()?;
        let empty = StateSelector::default();
        let styles = Styles {
            root: resolve(&empty, None)?,
            list: resolve(&empty, Some("list"))?,
            panel: resolve(&empty, Some("panel"))?,
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
impl Styled for Tabs {
    type Part = TabsPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Focusable for Tabs {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.selected
            .map(|i| self.items[i].focus.clone())
            .unwrap_or_else(|| self.fallback_focus.clone())
    }
}
impl Render for Tabs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let styles = match self.resolve(window, cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Tabs style error: {error}"))
                    .into_any_element();
            }
        };
        let focused = self
            .items
            .iter()
            .position(|item| item.focus.is_focused(window));
        let list = div()
            .id("tab-list")
            .role(Role::TabList)
            .aria_label(self.label.clone())
            .flex()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&self.scroll);
        let selected_styles = self.selected.and_then(|i| styles.items.get(i));
        let glass = selected_styles.is_some_and(|styles| styles[1].material.is_some());
        let mut decoration = div().size_full();
        let mut underline_height = px(2.);
        if let Some([_, on, indicator, _]) = selected_styles {
            if glass {
                let mut surface = on.clone();
                surface.border_width = crate::style::Resolved::Value(px(0.));
                decoration =
                    selection_surface(decoration, surface.clone(), surface, true, 1., 1., 0.)
                        .size_full();
            } else {
                decoration.style().refine(&indicator.to_gpui());
                decoration = decoration.size_full();
                underline_height = length(&indicator.height, 2.).max(px(0.));
            }
        }
        let mut group = gpui_effects::selection_indicator(
            "tab-selection",
            decoration.debug_selector(|| "fusion-tab-indicator".into()),
        )
        .duration(self.transition_duration)
        .enabled(self.animated && !window.prefers_reduced_motion())
        .flex()
        .min_w_full()
        .flex_shrink_0();
        group.style().refine(&styles.list.to_gpui());
        if !glass {
            group = group.underline(underline_height);
        }
        if let Some(index) = self.selected {
            group = group.selected(index);
        }
        let reveal = std::mem::take(&mut self.reveal);
        for (index, (item, [off, on, _, icon])) in
            self.items.iter_mut().zip(styles.items).enumerate()
        {
            let selected = self.selected == Some(index);
            let active = if selected { &on } else { &off };
            let mut tab = div()
                .id(index)
                .debug_selector(move || format!("fusion-tab-{index}"))
                .role(Role::Tab)
                .aria_label(item.tab.label.clone())
                .aria_selected(selected)
                .aria_disabled(item.tab.disabled)
                .track_focus(&item.focus)
                .tab_stop(!item.tab.disabled && focused.or(self.selected) == Some(index))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .flex_none();
            tab.style().refine(&off.to_gpui());
            tab = tab.text_color(color(&active.foreground));
            if selected {
                tab = tab.bg(gpui::transparent_black());
            }
            if let Some(path) = &item.tab.icon {
                let mut graphic = svg()
                    .path(path.clone())
                    .size(px(16.))
                    .text_color(color(&active.foreground));
                graphic.style().refine(&icon.to_gpui());
                tab = tab.child(graphic);
            }
            tab = tab.child(div().relative().child(item.tab.label.clone()));
            if reveal == Some(index) {
                let scroll = self.scroll.clone();
                tab = tab.child(
                    gpui::canvas(
                        move |bounds, window, cx| {
                            let viewport = scroll.bounds();
                            let offset = scroll.offset();
                            let delta = if bounds.left() < viewport.left()
                                || bounds.size.width > viewport.size.width
                            {
                                viewport.left() - bounds.left()
                            } else if bounds.right() > viewport.right() {
                                viewport.right() - bounds.right()
                            } else {
                                px(0.)
                            };
                            if delta.abs() > px(0.5) {
                                window.defer(cx, move |window, _| {
                                    scroll.set_offset(gpui::point(offset.x + delta, offset.y));
                                    window.refresh();
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                );
            }
            if !item.tab.disabled {
                let weak = cx.weak_entity();
                tab = tab
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered = hovered.then_some(index);
                        this.invalidate(cx);
                    }))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.activate(index, window, cx)),
                    )
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        this.navigate(index, event, window, cx)
                    }))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.activate(index, window, cx));
                    });
            }
            group = group.item(index, tab);
        }
        let mut root = div()
            .id("tabs")
            .debug_selector(|| "fusion-tabs".into())
            .flex()
            .flex_col()
            .min_w_0();
        root.style().refine(&styles.root.to_gpui());
        root = root.child(list.child(group));
        if let Some(item) = self.selected.and_then(|i| self.items.get(i)) {
            if let Some(panel) = &item.tab.panel {
                let mut content = div()
                    .id("tab-panel")
                    .debug_selector(|| "fusion-tab-panel".into())
                    .role(Role::TabPanel)
                    .aria_label(item.tab.label.clone())
                    .track_focus(&self.panel_focus)
                    .tab_stop(true)
                    .min_w_0()
                    .child(panel.clone());
                content.style().refine(&styles.panel.to_gpui());
                root = root.child(content);
            }
        }
        root.into_any_element()
    }
}
#[cfg(test)]
mod tests;
