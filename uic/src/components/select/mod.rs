mod state;

use crate::components::{input::Input, overlay_anchor::TriggerAnchor};
use gpui::{
    Anchor, AnyElement, App, ElementId, Entity, Focusable, Hsla, IntoElement, KeyDownEvent,
    MouseButton, Refineable, RenderOnce, Role, SharedString, StyleRefinement, Styled, Window,
    anchored, deferred_overlay, div, point, prelude::*, px, rgb,
};
pub use state::{SelectChanged, SelectOption, SelectState};
use std::rc::Rc;

/// Styles the dropdown option surface.
#[derive(Clone)]
pub struct SelectMenu {
    style: StyleRefinement,
}
impl Default for SelectMenu {
    fn default() -> Self {
        Self {
            style: StyleRefinement::default()
                .w(px(320.))
                .h(px(360.))
                .p_2()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xdfe5ee))
                .rounded_xl()
                .shadow_lg(),
        }
    }
}
impl SelectMenu {
    pub fn new() -> Self {
        Self::default()
    }
}
impl Styled for SelectMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Semantic colors used inside the trigger and option surface.
#[derive(Clone)]
pub struct SelectAppearance {
    pub muted: Hsla,
    pub active_background: Hsla,
    pub selected_foreground: Hsla,
    pub focus_border: Hsla,
    pub disabled_opacity: f32,
}
impl Default for SelectAppearance {
    fn default() -> Self {
        Self {
            muted: rgb(0x8290a3).into(),
            active_background: rgb(0xeaf0fc).into(),
            selected_foreground: rgb(0x3159bd).into(),
            focus_border: rgb(0x5b82e8).into(),
            disabled_opacity: 0.45,
        }
    }
}
#[derive(Clone, Copy)]
pub struct SelectItemState {
    pub selected: bool,
    pub highlighted: bool,
    pub disabled: bool,
}
type OptionRenderer =
    Rc<dyn Fn(&SelectOption, SelectItemState, &mut Window, &mut App) -> AnyElement>;

#[derive(Clone)]
struct Config {
    menu: SelectMenu,
    appearance: SelectAppearance,
    title: SharedString,
    empty_text: SharedString,
    render_option: Option<OptionRenderer>,
}

/// A single-selection trigger with searchable options and keyboard navigation.
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    state: Entity<SelectState>,
    placeholder: SharedString,
    search_placeholder: SharedString,
    searchable: bool,
    clearable: bool,
    disabled: bool,
    config: Config,
    style: StyleRefinement,
}
impl Select {
    pub fn new(id: impl Into<ElementId>, state: &Entity<SelectState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            placeholder: "Select an option".into(),
            search_placeholder: "Search options".into(),
            searchable: true,
            clearable: false,
            disabled: false,
            config: Config {
                menu: SelectMenu::default(),
                appearance: SelectAppearance::default(),
                title: "Choose an option".into(),
                empty_text: "No matching options".into(),
                render_option: None,
            },
            style: StyleRefinement::default()
                .w_full()
                .min_w(px(120.))
                .h(px(44.))
                .px_3()
                .bg(rgb(0xffffff))
                .text_color(rgb(0x25354b))
                .text_sm()
                .rounded_lg()
                .border_1()
                .border_color(rgb(0xd6deea)),
        }
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }
    /// Accessible trigger name.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.config.title = label.into();
        self
    }
    pub fn search_placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.search_placeholder = text.into();
        self
    }
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn menu(mut self, menu: SelectMenu) -> Self {
        self.config.menu = menu;
        self
    }
    pub fn appearance(mut self, appearance: SelectAppearance) -> Self {
        self.config.appearance = appearance;
        self
    }
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.config.empty_text = text.into();
        self
    }
    /// Supplies option content; the enclosing row retains selection and accessibility behavior.
    pub fn render_option<E: IntoElement>(
        mut self,
        render: impl Fn(&SelectOption, SelectItemState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.config.render_option = Some(Rc::new(move |option, state, window, cx| {
            render(option, state, window, cx).into_any_element()
        }));
        self
    }
}
impl Styled for Select {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for Select {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            state.search.update(cx, |input, _| {
                input.set_placeholder(self.search_placeholder.clone())
            });
            state.searchable = self.searchable;
            state.disabled = self.disabled;
            if self.disabled {
                state.close(window, cx);
            }
        });
        let mut inherited_text = self.style.text.clone();
        inherited_text.refine(&self.config.menu.style.text);
        self.config.menu.style.text = inherited_text;
        let state = self.state.read(cx);
        let selected = state.selected_option().map(|option| option.label.clone());
        let is_open = state.is_open();
        let focus = state.trigger_focus.clone();
        let scope = state.scope_focus.clone();
        let anchor = TriggerAnchor::default();
        let toggle = self.state.clone();
        let root_id = (self.id.clone(), "select");
        let clearable = self.clearable;
        let key_state = self.state.clone();
        let clear = self.state.clone();
        let trigger = div()
            .id(self.id)
            .track_focus(&focus)
            .tab_stop(!self.disabled)
            .role(Role::ComboBox)
            .aria_label(self.config.title.clone())
            .aria_expanded(is_open)
            .aria_disabled(self.disabled)
            .relative()
            .size_full()
            .flex()
            .items_center()
            .gap_2();
        let trigger = trigger
            .when(!self.disabled, |trigger| trigger.cursor_pointer())
            .on_click(move |_, window, cx| {
                if toggle.read(cx).is_open() {
                    toggle.update(cx, |state, cx| state.close(window, cx));
                } else {
                    open(&toggle, window, cx);
                }
                cx.stop_propagation();
            })
            .on_key_down(move |event, window, cx| {
                if !key_state.read(cx).is_open()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space" | "down")
                    && !key_state.read(cx).disabled
                {
                    open(&key_state, window, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                } else if clearable
                    && !key_state.read(cx).disabled
                    && matches!(event.keystroke.key.as_str(), "backspace" | "delete")
                {
                    key_state.update(cx, |state, cx| state.clear(cx));
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(selected.is_none(), |value| {
                        value.text_color(self.config.appearance.muted)
                    })
                    .child(selected.clone().unwrap_or(self.placeholder)),
            )
            .when(self.clearable && selected.is_some(), |trigger| {
                trigger.child(
                    div()
                        .id("clear")
                        .role(Role::Button)
                        .aria_label("Clear selection")
                        .aria_disabled(self.disabled)
                        .px_2()
                        .cursor_pointer()
                        .child("×")
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |state, cx| {
                                if !state.disabled {
                                    state.clear(cx);
                                }
                            });
                            cx.stop_propagation();
                        }),
                )
            })
            .child(div().text_color(self.config.appearance.muted).child("⌄"));
        let tracker = anchor.tracker();
        let menu_state = self.state.clone();
        let menu_config = self.config.clone();
        let menu = is_open.then(|| {
            deferred_overlay(move |window, cx| {
                if !menu_state.read(cx).is_open() {
                    return None;
                }
                let bounds = anchor.bounds(window)?;
                let outside = menu_state.clone();
                let outside_anchor = anchor.clone();
                let safe = window.insets().effective();
                let below = window.viewport_size().height - safe.bottom - bounds.bottom() - px(14.);
                let above = bounds.top() - safe.top - px(14.);
                let down = below >= px(240.) || below >= above;
                let position = point(
                    bounds.left(),
                    if down {
                        bounds.bottom() + px(6.)
                    } else {
                        bounds.top() - px(6.)
                    },
                );
                let mut surface = div()
                    .id("select-popup")
                    .occlude()
                    .flex()
                    .flex_col()
                    .overflow_hidden();
                surface.style().refine(&menu_config.menu.style);
                let surface = surface
                    .max_w(
                        (window.viewport_size().width - safe.left - safe.right - px(16.))
                            .max(px(0.)),
                    )
                    .max_h(if down { below } else { above }.max(px(0.)))
                    .on_mouse_down_out(move |event, window, cx| {
                        if !outside_anchor.contains(event.position, window) {
                            outside.update(cx, |state, cx| state.close(window, cx));
                        }
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(panel(&menu_state, &menu_config, window, cx));
                Some(
                    anchored()
                        .position(position)
                        .anchor(if down {
                            Anchor::TopLeft
                        } else {
                            Anchor::BottomLeft
                        })
                        .child(surface)
                        .into_any_element(),
                )
            })
            .with_priority(100)
        });
        let mut root = div().id(root_id).relative().track_focus(&scope);
        root.style().refine(&self.style);
        root.when(self.disabled, |root| {
            root.opacity(self.config.appearance.disabled_opacity)
        })
        .when(focus.is_focused(window) || is_open, |root| {
            root.border_color(self.config.appearance.focus_border)
        })
        .child(trigger)
        .children(menu)
        .child(tracker)
    }
}

fn open(state: &Entity<SelectState>, window: &mut Window, cx: &mut App) {
    if state.read(cx).disabled || state.read(cx).is_open() {
        return;
    }
    state.read(cx).trigger_focus.clone().focus(window, cx);
    state.update(cx, |state, cx| state.begin(cx));
    let focus = if state.read(cx).searchable {
        state.read(cx).search.focus_handle(cx)
    } else {
        state.read(cx).panel_focus.clone()
    };
    focus.focus(window, cx);
    window.refresh();
}

fn panel(
    state: &Entity<SelectState>,
    config: &Config,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let data = state.read(cx);
    let search = data.search.clone();
    let searchable = data.searchable;
    let focus = data.panel_focus.clone();
    let scroll = data.scroll.clone();
    let rows: Vec<_> = data
        .visible()
        .into_iter()
        .map(|i| {
            let option = data.options[i].clone();
            let flags = SelectItemState {
                selected: data.selected_id() == Some(&option.id),
                highlighted: data.active.as_ref() == Some(&option.id),
                disabled: option.disabled,
            };
            (option, flags)
        })
        .collect();
    let keyboard = state.clone();
    let mut root = div()
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_2()
        .track_focus(&focus)
        .capture_key_down(move |event: &KeyDownEvent, window, cx| {
            if keyboard.read(cx).search.read(cx).is_composing() {
                return;
            }
            let handled = keyboard.update(cx, |state, cx| match event.keystroke.key.as_str() {
                "up" => {
                    state.navigate(false, cx);
                    true
                }
                "down" => {
                    state.navigate(true, cx);
                    true
                }
                "enter" => {
                    state.accept(window, cx);
                    true
                }
                "escape" => {
                    state.close(window, cx);
                    true
                }
                "tab" => {
                    state.close(window, cx);
                    false
                }
                _ => false,
            });
            if handled {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
    if searchable {
        root = root.child(
            Input::new(&search)
                .flex_shrink_0()
                .w_full()
                .h(px(40.))
                .px_3()
                .py_2()
                .rounded_lg()
                .text_sm(),
        );
    }
    let mut list = div()
        .id("options")
        .role(Role::ListBox)
        .aria_label(config.title.clone())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&scroll);
    if rows.is_empty() {
        list = list.child(
            div()
                .p_4()
                .text_color(config.appearance.muted)
                .child(config.empty_text.clone()),
        );
    }
    for (option, flags) in rows {
        let choose = state.clone();
        let id = option.id.clone();
        let hover = state.clone();
        let hover_id = option.id.clone();
        let content = config
            .render_option
            .as_ref()
            .map(|render| render(&option, flags, window, cx))
            .unwrap_or_else(|| div().child(option.label.clone()).into_any_element());
        let row = div()
            .id(option.id.clone())
            .role(Role::ListBoxOption)
            .aria_label(option.label.clone())
            .aria_selected(flags.selected)
            .aria_disabled(flags.disabled)
            .when(flags.highlighted, |row| row.aria_active_descendant())
            .w_full()
            .min_h(px(42.))
            .px_3()
            .py_2()
            .rounded_lg()
            .flex()
            .items_center()
            .gap_2()
            .when(flags.highlighted, |row| {
                row.bg(config.appearance.active_background)
            })
            .when(flags.disabled, |row| {
                row.opacity(config.appearance.disabled_opacity)
            })
            .when(!flags.disabled, |row| {
                row.cursor_pointer().on_hover(move |hovered, _, cx| {
                    if *hovered {
                        hover.update(cx, |state, cx| {
                            state.active = Some(hover_id.clone());
                            cx.notify();
                        });
                    }
                })
            })
            .on_click(move |_, window, cx| {
                choose.update(cx, |state, cx| {
                    if !state.disabled && state.select(&id, cx) {
                        state.close(window, cx);
                    }
                });
                cx.stop_propagation();
            })
            .child(div().flex_1().min_w_0().child(content))
            .child(
                div()
                    .w(px(18.))
                    .text_color(config.appearance.selected_foreground)
                    .child(if flags.selected { "✓" } else { "" }),
            );
        list = list.child(row);
    }
    root.child(list).into_any_element()
}

#[cfg(test)]
mod tests;
