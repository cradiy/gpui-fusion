mod state;
pub use state::{AutocompleteEvent, AutocompleteState};

use crate::components::{
    input::{Input, InputAppearance},
    overlay_anchor::TriggerAnchor,
    select::{SelectAppearance, SelectItemState, SelectMenu, SelectOption},
};
use gpui::{
    AccessibleAction, Anchor, AnyElement, App, Entity, Focusable, Hsla, KeyDownEvent,
    KeybindingKeystroke, Keystroke, MouseButton, Refineable, Role, SharedString, StyleRefinement,
    Window, anchored, deferred_overlay, div, point, prelude::*, px, rgb, transparent_black,
};
use std::rc::Rc;

#[derive(Clone, Copy)]
pub struct AutocompleteAppearance {
    pub input: InputAppearance,
    pub active_background: Hsla,
    pub muted: Hsla,
    pub disabled_opacity: f32,
}
impl Default for AutocompleteAppearance {
    fn default() -> Self {
        let select = SelectAppearance::default();
        Self {
            input: InputAppearance::default(),
            active_background: select.active_background,
            muted: select.muted,
            disabled_opacity: select.disabled_opacity,
        }
    }
}
type OptionRenderer =
    Rc<dyn Fn(&SelectOption, SelectItemState, &mut Window, &mut App) -> AnyElement>;

/// An operation bound to a single key combination on an autocomplete input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutocompleteAction {
    Previous,
    Next,
    Confirm,
    Dismiss,
}

/// A free-text input with suggestions. Styled properties apply to the input surface.
#[derive(IntoElement)]
pub struct Autocomplete {
    state: Entity<AutocompleteState>,
    label: SharedString,
    placeholder: SharedString,
    empty_text: SharedString,
    loading_text: SharedString,
    menu: SelectMenu,
    appearance: AutocompleteAppearance,
    renderer: Option<OptionRenderer>,
    key_bindings: Vec<(KeybindingKeystroke, AutocompleteAction)>,
    style: StyleRefinement,
}
impl Autocomplete {
    pub fn new(state: &Entity<AutocompleteState>) -> Self {
        Self {
            state: state.clone(),
            label: "Suggestions".into(),
            placeholder: "Type to search".into(),
            empty_text: "No suggestions".into(),
            loading_text: "Loading…".into(),
            menu: SelectMenu::new().w_auto().h_auto().max_h(px(280.)),
            appearance: AutocompleteAppearance::default(),
            renderer: None,
            key_bindings: Vec::new(),
            style: StyleRefinement::default()
                .w_full()
                .h(px(44.))
                .px_3()
                .rounded_lg()
                .border_1()
                .border_color(rgb(0xd6deea))
                .bg(rgb(0xffffff))
                .text_color(rgb(0x25354b))
                .text_size(px(16.))
                .line_height(px(24.)),
        }
        .key_binding("up", AutocompleteAction::Previous)
        .key_binding("down", AutocompleteAction::Next)
        .key_binding("enter", AutocompleteAction::Confirm)
        .key_binding("escape", AutocompleteAction::Dismiss)
    }
    /// Add a shortcut for this input. The last matching binding takes precedence.
    /// Panics if the shortcut is not one valid GPUI key combination.
    pub fn key_binding(mut self, shortcut: &str, action: AutocompleteAction) -> Self {
        assert_eq!(
            shortcut.split_whitespace().count(),
            1,
            "expected one key combination"
        );
        let key = Keystroke::parse(shortcut).expect("invalid autocomplete shortcut");
        self.key_bindings
            .push((KeybindingKeystroke::from_keystroke(key), action));
        self
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }
    pub fn loading_text(mut self, text: impl Into<SharedString>) -> Self {
        self.loading_text = text.into();
        self
    }
    pub fn menu(mut self, menu: SelectMenu) -> Self {
        self.menu = menu;
        self
    }
    pub fn appearance(mut self, appearance: AutocompleteAppearance) -> Self {
        self.appearance = appearance;
        self
    }
    pub fn render_option<E: IntoElement>(
        mut self,
        render: impl Fn(&SelectOption, SelectItemState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.renderer = Some(Rc::new(move |option, flags, window, cx| {
            render(option, flags, window, cx).into_any_element()
        }));
        self
    }
}
impl Styled for Autocomplete {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for Autocomplete {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            if state.label != self.label {
                state.label = self.label.clone();
                state
                    .input
                    .update(cx, |input, cx| input.set_aria_label(self.label.clone(), cx));
            }
        });
        let state = self.state.read(cx);
        let input = state.input.clone();
        let focus = state.focus_handle(cx);
        let disabled = state.is_disabled(cx);
        let opened = state.opened && !input.read(cx).is_composing() && !disabled;
        input.update(cx, |input, _| {
            input.set_placeholder(self.placeholder.clone());
        });
        let anchor = TriggerAnchor::default();
        let popup_anchor = TriggerAnchor::default();
        let tracker = anchor.tracker();
        let outside_menu = popup_anchor.clone();
        let outside = self.state.clone();
        let click = self.state.clone();
        let keyboard = self.state.clone();
        let menu_state = self.state.clone();
        let appearance = self.appearance;
        let mut field = Input::new(&input)
            .blur_on_click_outside(false)
            .appearance(appearance.input)
            .w_full()
            .h_full()
            .min_w_0()
            .px_0()
            .border_0()
            .rounded_none()
            .bg(transparent_black());
        field.style().text = self.style.text.clone();
        let label = self.label.clone();
        let overlay = opened.then(|| {
            deferred_overlay(move |window, cx| {
                let state = menu_state.read(cx);
                if !state.opened || state.input.read(cx).is_composing() || state.is_disabled(cx) {
                    return None;
                }
                let bounds = anchor.bounds(window)?;
                let safe = window.insets().effective();
                let below = window.viewport_size().height - safe.bottom - bounds.bottom() - px(14.);
                let above = bounds.top() - safe.top - px(14.);
                let down = below >= px(180.) || below >= above;
                let position = point(
                    bounds.left(),
                    if down {
                        bounds.bottom() + px(6.)
                    } else {
                        bounds.top() - px(6.)
                    },
                );
                let scroll = state.scroll.clone();
                let mut list = div()
                    .id("suggestions")
                    .role(Role::ListBox)
                    .aria_label(label.clone())
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&scroll);
                let loading = state.loading;
                let rows: Vec<_> = state
                    .visible()
                    .into_iter()
                    .map(|i| {
                        let option = state.options[i].clone();
                        let flags = SelectItemState {
                            selected: false,
                            highlighted: state.active.as_ref() == Some(&option.id),
                            disabled: option.disabled,
                        };
                        (option, flags)
                    })
                    .collect();
                if loading || rows.is_empty() {
                    list = list.child(div().p_3().text_color(appearance.muted).child(if loading {
                        self.loading_text.clone()
                    } else {
                        self.empty_text.clone()
                    }));
                } else {
                    for (option, flags) in rows {
                        let content = self
                            .renderer
                            .as_ref()
                            .map(|render| render(&option, flags, window, cx))
                            .unwrap_or_else(|| {
                                div().child(option.label.clone()).into_any_element()
                            });
                        let choose = menu_state.clone();
                        let accessible = menu_state.clone();
                        let id = option.id.clone();
                        let accessible_id = id.clone();
                        list = list.child(
                            div()
                                .id(option.id.clone())
                                .role(Role::ListBoxOption)
                                .aria_label(option.label.clone())
                                .aria_disabled(flags.disabled)
                                .aria_selected(flags.highlighted)
                                .when(flags.highlighted, |row| row.aria_active_descendant())
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .when(flags.disabled, |row| {
                                    row.opacity(appearance.disabled_opacity)
                                })
                                .when(flags.highlighted, |row| {
                                    row.bg(appearance.active_background)
                                })
                                .when(!flags.disabled, |row| {
                                    row.cursor_pointer()
                                        .hover(move |style| style.bg(appearance.active_background))
                                })
                                .on_click(move |_, window, cx| {
                                    choose.update(cx, |state, cx| state.choose(&id, window, cx));
                                    cx.stop_propagation();
                                })
                                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                                    accessible.update(cx, |state, cx| {
                                        state.choose(&accessible_id, window, cx)
                                    });
                                })
                                .child(content),
                        );
                    }
                }
                let mut surface = div()
                    .id("autocomplete-popup")
                    .relative()
                    .occlude()
                    .w(bounds.size.width)
                    .flex()
                    .flex_col()
                    .overflow_hidden();
                surface.style().refine(&self.menu.style);
                // An auto menu width follows the input; callers may specify an explicit width.
                if matches!(self.menu.style.size.width, Some(gpui::Length::Auto)) {
                    surface = surface.w(bounds.size.width);
                }
                let available = if down { below } else { above }.max(px(0.));
                let height_limit = match self.menu.style.max_size.height {
                    Some(gpui::Length::Definite(length)) => length
                        .to_pixels(window.viewport_size().height.into(), window.rem_size())
                        .min(available),
                    _ => available,
                };
                surface = surface
                    .max_w(
                        (window.viewport_size().width - safe.left - safe.right - px(16.))
                            .max(px(0.)),
                    )
                    .max_h(height_limit)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(list)
                    .child(popup_anchor.tracker());
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
        let mut root = div()
            .id(("autocomplete", self.state.entity_id()))
            .relative()
            .flex()
            .items_center()
            .role(Role::ComboBox)
            .aria_label(self.label.clone())
            .aria_expanded(opened)
            .aria_disabled(disabled)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                click.update(cx, |state, cx| state.open(cx))
            })
            .on_mouse_down_out(move |event, window, cx| {
                if event.button == MouseButton::Left
                    && !outside_menu.contains(event.position, window)
                {
                    outside.update(cx, |state, cx| {
                        state.close(cx);
                        if state.focus_handle(cx).is_focused(window) {
                            window.blur();
                        }
                    });
                }
            })
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                let state = keyboard.read(cx);
                let modifiers = event.keystroke.modifiers;
                if state.is_disabled(cx)
                    || state.input.read(cx).is_composing()
                    || event.keystroke.is_ime_in_progress()
                {
                    return;
                }
                let action = self
                    .key_bindings
                    .iter()
                    .rev()
                    .find(|(key, _)| event.keystroke.should_match(key))
                    .map(|(_, action)| *action);
                let tab = event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                    && !modifiers.function;
                let handled = keyboard.update(cx, |state, cx| match action {
                    Some(AutocompleteAction::Previous) => {
                        state.navigate(false, cx);
                        true
                    }
                    Some(AutocompleteAction::Next) => {
                        state.navigate(true, cx);
                        true
                    }
                    Some(AutocompleteAction::Confirm) => {
                        state.submit(window, cx);
                        true
                    }
                    Some(AutocompleteAction::Dismiss) if state.opened => {
                        state.close(cx);
                        true
                    }
                    None if tab => {
                        state.close(cx);
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
        root.style().refine(&self.style);
        root.when(disabled, |root| root.opacity(appearance.disabled_opacity))
            .when(!disabled && focus.is_focused(window), |root| {
                root.border_color(appearance.input.focus_border)
            })
            .child(field)
            .children(overlay)
            .child(tracker)
    }
}

#[cfg(test)]
mod tests;
