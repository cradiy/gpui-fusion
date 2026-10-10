mod multi_select;
mod state;

use crate::components::{input::Input, overlay_anchor::TriggerAnchor};
use gpui::{
    AccessibleAction, Anchor, AnyElement, App, ElementId, Entity, Focusable, Hsla, IntoElement,
    KeyDownEvent, MouseButton, PathBuilder, Refineable, RenderOnce, Role, SharedString,
    StyleRefinement, Styled, Window, anchored, canvas, deferred_overlay, div, point, prelude::*,
    px, rgb,
};
pub use multi_select::MultiSelect;
pub use state::{MultiSelectChanged, SelectChanged, SelectOption, SelectState};
use std::rc::Rc;

/// Styles the dropdown option surface.
#[derive(Clone)]
pub struct SelectMenu {
    pub(super) style: StyleRefinement,
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
    clearable: bool,
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
    disabled: bool,
    multiple: bool,
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
            disabled: false,
            multiple: false,
            config: Config {
                clearable: false,
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
        self.config.clearable = clearable;
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
            assert_eq!(
                state.multiple, self.multiple,
                "MultiSelect requires SelectState::multiple; Select requires SelectState::new"
            );
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
        let focus = state.trigger_focus.clone().tab_stop(!self.disabled);
        let scope = state.scope_focus.clone();
        let anchor = TriggerAnchor::default();
        let toggle = self.state.clone();
        let root_id = (self.id.clone(), "select");
        let clearable = self.config.clearable;
        let multiple = self.multiple;
        let key_state = self.state.clone();
        let clear = self.state.clone();
        let accessible_clear = self.state.clone();
        let trigger = div()
            .id(self.id)
            .track_focus(&focus)
            .role(Role::ComboBox)
            .aria_label(self.config.title.clone())
            .aria_expanded(is_open)
            .aria_disabled(self.disabled)
            .relative()
            .w_full()
            .when(!self.multiple, |trigger| trigger.h_full())
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
                } else if multiple
                    && !key_state.read(cx).disabled
                    && !key_state.read(cx).is_open()
                    && event.keystroke.key == "backspace"
                {
                    key_state.update(cx, |state, cx| {
                        if let Some(id) = state.selected_ids().last().cloned() {
                            state.deselect(&id, cx);
                        }
                    });
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
            .child(if self.multiple {
                selected_tags(&self.state, &self.placeholder, &self.config.appearance, cx)
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(selected.is_none(), |value| {
                        value.text_color(self.config.appearance.muted)
                    })
                    .child(selected.clone().unwrap_or(self.placeholder))
                    .into_any_element()
            })
            .when(
                self.config.clearable && !self.multiple && selected.is_some(),
                |trigger| {
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
                            })
                            .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                                accessible_clear.update(cx, |state, cx| {
                                    if !state.disabled {
                                        state.clear(cx);
                                    }
                                });
                            }),
                    )
                },
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(self.config.appearance.muted)
                    .when(self.multiple, |arrow| arrow.self_start().mt(px(6.)))
                    .child(select_icon(SelectIcon::Chevron)),
            );
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
    let multiple = data.multiple;
    let selected_count = data.selected_ids().len();
    let limit = data.selection_limit();
    let focus = data.panel_focus.clone();
    let scroll = data.scroll.clone();
    let rows: Vec<_> = data
        .visible()
        .into_iter()
        .map(|i| {
            let option = data.options[i].clone();
            let flags = SelectItemState {
                selected: data.is_selected(&option.id),
                highlighted: data.active.as_ref() == Some(&option.id),
                disabled: data.unavailable(&option),
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
                "space" if !state.searchable => {
                    state.accept(window, cx);
                    true
                }
                "escape" => {
                    state.close(window, cx);
                    true
                }
                "tab" => {
                    state.close(window, cx);
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    true
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
                .blur_on_click_outside(false)
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
        .role(if multiple { Role::Group } else { Role::ListBox })
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
        let accessible_choose = state.clone();
        let accessible_id = id.clone();
        let hover = state.clone();
        let hover_id = option.id.clone();
        let content = config
            .render_option
            .as_ref()
            .map(|render| render(&option, flags, window, cx))
            .unwrap_or_else(|| div().child(option.label.clone()).into_any_element());
        let row = div()
            .id(option.id.clone())
            .role(if multiple {
                Role::CheckBox
            } else {
                Role::ListBoxOption
            })
            .when(multiple, |row| row.aria_toggled(flags.selected.into()))
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
                    state.choose(&id, window, cx);
                });
                cx.stop_propagation();
            })
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                accessible_choose.update(cx, |state, cx| state.choose(&accessible_id, window, cx));
            })
            .when(multiple, |row| {
                row.child(
                    div()
                        .size(px(16.))
                        .flex_shrink_0()
                        .rounded(px(5.))
                        .border_1()
                        .border_color(if flags.selected {
                            config.appearance.selected_foreground
                        } else {
                            config.appearance.muted.opacity(0.45)
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(flags.selected, |mark| {
                            mark.bg(config.appearance.selected_foreground)
                                .text_color(rgb(0xffffff))
                                .child(select_icon(SelectIcon::Check))
                        }),
                )
            })
            .child(div().flex_1().min_w_0().child(content))
            .when(!multiple, |row| {
                row.child(
                    div()
                        .w(px(18.))
                        .text_color(config.appearance.selected_foreground)
                        .child(if flags.selected { "✓" } else { "" }),
                )
            });
        list = list.child(row);
    }
    root.child(list)
        .when(multiple, |root| {
            let clear = state.clone();
            let accessible_clear = state.clone();
            let accent = config.appearance.selected_foreground;
            root.child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .border_t_1()
                    .border_color(config.appearance.muted.opacity(0.15))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(config.appearance.muted)
                    .child(div().child(match limit {
                        Some(limit) => format!("{selected_count} / {limit} selected"),
                        None => format!("{selected_count} selected"),
                    }))
                    .when(config.clearable && selected_count > 0, |footer| {
                        footer.child(
                            div()
                                .id("clear-all")
                                .role(Role::Button)
                                .aria_label("Clear selection")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_color(accent)
                                .child("Clear")
                                .hover(move |style| style.bg(accent.opacity(0.08)))
                                .on_click(move |_, _, cx| {
                                    clear.update(cx, |state, cx| {
                                        if !state.disabled {
                                            state.clear(cx);
                                        }
                                    });
                                    cx.stop_propagation();
                                })
                                .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                                    accessible_clear.update(cx, |state, cx| {
                                        if !state.disabled {
                                            state.clear(cx);
                                        }
                                    });
                                }),
                        )
                    }),
            )
        })
        .into_any_element()
}

fn selected_tags(
    state: &Entity<SelectState>,
    placeholder: &SharedString,
    appearance: &SelectAppearance,
    cx: &App,
) -> gpui::Div {
    let data = state.read(cx);
    let mut tags = div().flex_1().min_w_0().flex().flex_wrap().gap(px(6.));
    if data.selected_ids().is_empty() {
        return tags.text_color(appearance.muted).child(placeholder.clone());
    }
    for id in data.selected_ids() {
        let Some(option) = data.options.iter().find(|option| &option.id == id) else {
            continue;
        };
        let remove = state.clone();
        let removed = id.clone();
        let accessible = state.clone();
        let accessible_id = id.clone();
        let disabled = data.disabled;
        tags = tags.child(
            div()
                .id(id.clone())
                .max_w_full()
                .min_w_0()
                .h(px(26.))
                .pl_2()
                .pr_1()
                .gap_1()
                .rounded(px(5.))
                .bg(appearance.active_background)
                .text_color(appearance.selected_foreground)
                .flex()
                .items_center()
                .child(div().min_w_0().truncate().child(option.label.clone()))
                .child(
                    div()
                        .id("remove")
                        .role(Role::Button)
                        .aria_label(format!("Remove {}", option.label))
                        .aria_disabled(disabled)
                        .size(px(20.))
                        .flex_shrink_0()
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(appearance.selected_foreground.opacity(0.65))
                        .when(!disabled, |button| {
                            let accent = appearance.selected_foreground;
                            button.cursor_pointer().hover(move |style| {
                                style.bg(accent.opacity(0.1)).text_color(accent)
                            })
                        })
                        .child(select_icon(SelectIcon::Close))
                        .on_click(move |_, _, cx| {
                            remove.update(cx, |state, cx| {
                                if !state.disabled {
                                    state.deselect(&removed, cx);
                                }
                            });
                            cx.stop_propagation();
                        })
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            accessible.update(cx, |state, cx| {
                                if !state.disabled {
                                    state.deselect(&accessible_id, cx);
                                }
                            });
                        }),
                ),
        );
    }
    tags
}

#[derive(Clone, Copy)]
enum SelectIcon {
    Close,
    Chevron,
    Check,
}

fn select_icon(icon: SelectIcon) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            let mut path = PathBuilder::stroke(px(1.4));
            match icon {
                SelectIcon::Close => {
                    path.move_to(center + point(px(-2.5), px(-2.5)));
                    path.line_to(center + point(px(2.5), px(2.5)));
                    path.move_to(center + point(px(-2.5), px(2.5)));
                    path.line_to(center + point(px(2.5), px(-2.5)));
                }
                SelectIcon::Chevron => {
                    path.move_to(center + point(px(-3.), px(-1.5)));
                    path.line_to(center + point(px(0.), px(1.5)));
                    path.line_to(center + point(px(3.), px(-1.5)));
                }
                SelectIcon::Check => {
                    path.move_to(center + point(px(-3.), px(0.)));
                    path.line_to(center + point(px(-0.8), px(2.2)));
                    path.line_to(center + point(px(3.4), px(-2.4)));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, window.text_style().color);
            }
        },
    )
    .size(px(12.))
}

#[cfg(test)]
mod multi_tests;
#[cfg(test)]
mod tests;
