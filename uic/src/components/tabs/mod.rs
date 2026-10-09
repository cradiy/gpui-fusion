mod state;

use std::rc::Rc;

use gpui::{
    AnyElement, App, Background, ElementId, Hsla, IntoElement, MouseButton, Refineable, RenderOnce,
    Role, SharedString, StyleRefinement, Styled, Window, canvas, div, prelude::*, px, rgb,
    transparent_black,
};
use gpui_effects::selection_indicator;
use state::State;

/// Decoration used to mark the selected tab.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TabVariant {
    #[default]
    Underline,
    Pill,
}

/// Colors of the tab bar's interaction states and selection marker.
#[derive(Clone)]
pub struct TabsAppearance {
    pub indicator: Background,
    pub selected_text: Hsla,
    pub hover_background: Background,
    pub focus_ring: Hsla,
    pub disabled_opacity: f32,
}

impl Default for TabsAppearance {
    fn default() -> Self {
        Self {
            indicator: rgb(0xdfe8ff).into(),
            selected_text: rgb(0x234cb0).into(),
            hover_background: gpui::rgba(0x8196bd18).into(),
            focus_ring: rgb(0x6589e6).into(),
            disabled_opacity: 0.4,
        }
    }
}

type ChangeCallback<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

struct Tab<T> {
    value: T,
    content: AnyElement,
    disabled: bool,
}

/// A controlled horizontal tab bar with automatic activation and overflow scrolling.
/// Keep a stable ID and update the selected value in `on_change`.
#[derive(IntoElement)]
pub struct Tabs<T: Clone + PartialEq + 'static> {
    id: ElementId,
    selected: T,
    tabs: Vec<Tab<T>>,
    variant: TabVariant,
    disabled: bool,
    label: Option<SharedString>,
    on_change: Option<ChangeCallback<T>>,
    appearance: TabsAppearance,
    style: StyleRefinement,
}

impl<T: Clone + PartialEq + 'static> Tabs<T> {
    pub fn new(id: impl Into<ElementId>, selected: T) -> Self {
        Self {
            id: id.into(),
            selected,
            tabs: Vec::new(),
            variant: TabVariant::default(),
            disabled: false,
            label: None,
            on_change: None,
            appearance: TabsAppearance::default(),
            style: StyleRefinement::default()
                .w_full()
                .min_w_0()
                .text_sm()
                .text_color(rgb(0x758299))
                .rounded_lg()
                .border_1()
                .border_color(transparent_black()),
        }
    }

    /// Appends a content-sized tab. Values must be unique within the bar.
    #[track_caller]
    pub fn tab(mut self, value: T, content: impl IntoElement) -> Self {
        assert!(
            !self.tabs.iter().any(|tab| tab.value == value),
            "tab values must be unique"
        );
        self.tabs.push(Tab {
            value,
            content: content.into_any_element(),
            disabled: false,
        });
        self
    }

    pub fn disabled_tab(self, value: T, content: impl IntoElement) -> Self {
        let mut this = self.tab(value, content);
        this.tabs.last_mut().unwrap().disabled = true;
        this
    }

    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn appearance(mut self, appearance: TabsAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn on_change(mut self, callback: impl Fn(T, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
}

impl<T: Clone + PartialEq + 'static> Styled for Tabs<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + PartialEq + 'static> RenderOnce for Tabs<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| State::new(cx));
        let index = self.tabs.iter().position(|tab| tab.value == self.selected);
        state.update(cx, |state, _| {
            if state.selected != index {
                state.selected = index;
                state.geometry = None;
            }
        });
        let scroll = state.read(cx).scroll.clone();
        let focus = state.read(cx).focus.clone();
        let values: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| !tab.disabled)
            .map(|tab| tab.value.clone())
            .collect();
        let enabled = !self.disabled && !values.is_empty();
        let key_callback = self.on_change.clone();
        let selected = self.selected.clone();
        let appearance = self.appearance;
        let underline = self.variant == TabVariant::Underline;
        let mut items = selection_indicator(
            "selection",
            div()
                .size_full()
                .rounded_lg()
                .bg(appearance.indicator.clone()),
        )
        .enabled(!window.prefers_reduced_motion())
        .flex()
        .gap_1()
        .min_w_full()
        .flex_shrink_0()
        .when_some(index, |items, index| items.selected(index.to_string()));
        if underline {
            items = items.underline(px(3.));
        }
        for (index, tab) in self.tabs.into_iter().enumerate() {
            let selected = self.selected == tab.value;
            let disabled = self.disabled || tab.disabled;
            let callback = self.on_change.clone();
            let focus = focus.clone();
            let measure = state.downgrade();
            let item = div()
                .id(("tab", index))
                .debug_selector(move || format!("uic-tab-{index}"))
                .role(Role::Tab)
                .aria_selected(selected)
                .aria_disabled(disabled)
                .when(selected, |item| item.aria_active_descendant())
                .relative()
                .whitespace_nowrap()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .px_4()
                .py_3()
                .rounded_lg()
                .when(selected, |item| item.text_color(appearance.selected_text))
                .opacity(if disabled {
                    appearance.disabled_opacity
                } else {
                    1.
                })
                .when(!disabled, |item| {
                    item.cursor_pointer()
                        .hover(|style| style.bg(appearance.hover_background.clone()))
                        .on_click(move |_, window, cx| {
                            focus.focus(window, cx);
                            if !selected && let Some(callback) = &callback {
                                callback(tab.value.clone(), window, cx);
                            }
                        })
                })
                .child(tab.content)
                .when(selected, |item| {
                    item.child(
                        canvas(
                            move |bounds, window, cx| {
                                let _ = measure
                                    .update(cx, |state, cx| state.reveal(bounds, window, cx));
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                });
            items = items.item(index.to_string(), item);
        }
        let mut root = div()
            .id(self.id)
            .debug_selector(|| "uic-tabs".into())
            .track_focus(&focus)
            .tab_stop(enabled)
            .role(Role::TabList)
            .aria_disabled(self.disabled)
            .when_some(self.label, |root, label| root.aria_label(label))
            .on_key_down(move |event, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if !enabled
                    || modifiers.control
                    || modifiers.platform
                    || modifiers.alt
                    || modifiers.shift
                {
                    return;
                }
                let current = values.iter().position(|value| *value == selected);
                let index = match event.keystroke.key.as_str() {
                    "left" => current
                        .map(|index| (index + values.len() - 1) % values.len())
                        .unwrap_or(values.len() - 1),
                    "right" => current.map(|index| (index + 1) % values.len()).unwrap_or(0),
                    "home" => 0,
                    "end" => values.len() - 1,
                    _ => return,
                };
                if values[index] != selected
                    && let Some(callback) = &key_callback
                {
                    callback(values[index].clone(), window, cx);
                }
                window.prevent_default();
                cx.stop_propagation();
            })
            .when(enabled, |root| {
                root.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    focus.focus(window, cx)
                })
            })
            .child(
                div()
                    .id("viewport")
                    .flex()
                    .items_start()
                    .w_full()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .child(items),
            );
        root.style().refine(&self.style);
        root.focus_visible(move |style| style.border_color(appearance.focus_ring))
    }
}

#[cfg(test)]
mod tests;
