mod state;
pub use state::{CommandItem, CommandPaletteEvent, CommandPaletteState};

use crate::components::{
    input::{Input, InputAppearance},
    modal::{self, Modal, ModalPlacement},
};
use gpui::{
    AccessibleAction, AnyElement, App, Entity, Focusable, Hsla, KeyDownEvent, KeybindingKeystroke,
    Keystroke, Length, MouseButton, Pixels, Refineable, Role, SharedString, StyleRefinement,
    Window, div, prelude::*, px, rgb, svg, transparent_black,
};
use state::Row;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub struct CommandPaletteAppearance {
    pub input: InputAppearance,
    pub active_background: Hsla,
    pub muted: Hsla,
    pub divider: Hsla,
    pub disabled_opacity: f32,
}
impl Default for CommandPaletteAppearance {
    fn default() -> Self {
        Self {
            input: InputAppearance::default(),
            active_background: rgb(0xebf0fc).into(),
            muted: rgb(0x8190a5).into(),
            divider: rgb(0xe8ecf3).into(),
            disabled_opacity: 0.4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CommandItemState {
    pub highlighted: bool,
    pub disabled: bool,
}
type ItemRenderer = Rc<dyn Fn(&CommandItem, CommandItemState, &mut Window, &mut App) -> AnyElement>;
type SurfaceRenderer =
    Rc<dyn Fn(AnyElement, &StyleRefinement, &mut Window, &mut App) -> AnyElement>;

/// An operation bound to a single key combination in a command palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandPaletteAction {
    Previous,
    Next,
    Confirm,
    Dismiss,
}

/// Searchable commands, embedded directly or presented with [`Self::show`].
/// Styled properties apply to the entire palette surface.
#[derive(Clone, IntoElement)]
pub struct CommandPalette {
    state: Entity<CommandPaletteState>,
    label: SharedString,
    placeholder: SharedString,
    empty_text: SharedString,
    loading_text: SharedString,
    appearance: CommandPaletteAppearance,
    renderer: Option<ItemRenderer>,
    surface: Option<SurfaceRenderer>,
    key_bindings: Vec<(KeybindingKeystroke, CommandPaletteAction)>,
    placement: Option<ModalPlacement>,
    in_modal: bool,
    style: StyleRefinement,
}
impl CommandPalette {
    pub fn new(state: &Entity<CommandPaletteState>) -> Self {
        Self {
            state: state.clone(),
            label: "Commands".into(),
            placeholder: "Search commands…".into(),
            empty_text: "No matching commands".into(),
            loading_text: "Loading commands…".into(),
            appearance: CommandPaletteAppearance::default(),
            renderer: None,
            surface: None,
            key_bindings: Vec::new(),
            placement: None,
            in_modal: false,
            style: StyleRefinement::default()
                .w(px(560.))
                .h(px(420.))
                .rounded_xl()
                .border_1()
                .border_color(rgb(0xdce3ee))
                .text_color(rgb(0x26364d))
                .text_size(px(15.))
                .line_height(px(22.)),
        }
        .key_binding("up", CommandPaletteAction::Previous)
        .key_binding("down", CommandPaletteAction::Next)
        .key_binding("enter", CommandPaletteAction::Confirm)
        .key_binding("escape", CommandPaletteAction::Dismiss)
    }
    /// Add a shortcut for this palette. The last matching binding takes precedence.
    /// Panics if the shortcut is not one valid GPUI key combination.
    pub fn key_binding(mut self, shortcut: &str, action: CommandPaletteAction) -> Self {
        assert_eq!(
            shortcut.split_whitespace().count(),
            1,
            "expected one key combination"
        );
        let key = Keystroke::parse(shortcut).expect("invalid command palette shortcut");
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
    pub fn appearance(mut self, appearance: CommandPaletteAppearance) -> Self {
        self.appearance = appearance;
        self
    }
    /// Set the placement used by [`Self::show`]. Embedded layout is unaffected.
    /// The default is horizontally centered, 24px below the top safe area.
    pub fn placement(mut self, placement: ModalPlacement) -> Self {
        self.placement = Some(placement);
        self
    }
    /// Replace the palette shell with a styled container holding `content` once.
    /// Palette styles and viewport limits apply to the returned surface. Custom
    /// surfaces have no default white background; an explicit `.bg(...)` still applies.
    pub fn surface<E: IntoElement + Styled>(
        mut self,
        render: impl Fn(AnyElement, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.surface = Some(Rc::new(move |content, style, window, cx| {
            let mut surface = render(content, window, cx)
                .flex()
                .flex_col()
                .min_w_0()
                .min_h_0()
                .overflow_hidden();
            surface.style().refine(style);
            surface.into_any_element()
        }));
        self
    }
    /// Customize row content; the palette retains navigation and activation behavior.
    pub fn render_item<E: IntoElement>(
        mut self,
        render: impl Fn(&CommandItem, CommandItemState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.renderer = Some(Rc::new(move |item, flags, window, cx| {
            render(item, flags, window, cx).into_any_element()
        }));
        self
    }
    /// Show through the mounted modal layer, retaining the current query.
    /// Confirmation closes the modal before emitting `Invoked` and restores prior focus.
    pub fn show(mut self, window: &mut Window, cx: &mut App) {
        let lifetime = Rc::new(());
        self.state.update(cx, |state, _| {
            state.modal_lifetime = Rc::downgrade(&lifetime)
        });
        let focus = self.state.focus_handle(cx);
        let placement = self.placement.unwrap_or(ModalPlacement::Top {
            offset: window.insets().effective().top + px(24.),
        });
        self.placement = Some(placement);
        self.in_modal = true;
        modal::show(
            Modal::new(move |_, _| {
                // Only the mounted modal owns this token; replacement expires it.
                let _keep_alive = &lifetime;
                self.clone()
            })
            .unstyled()
            .hide_footer()
            .ok_on_enter(false)
            .close_on_escape(false)
            .placement(placement),
            window,
            cx,
        );
        window.focus(&focus, cx);
    }
}
impl Styled for CommandPalette {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            if state.label != self.label {
                state.label = self.label.clone();
                state
                    .input
                    .update(cx, |input, cx| input.set_aria_label(self.label.clone(), cx));
            }
            state.input.update(cx, |input, _| {
                input.set_placeholder(self.placeholder.clone())
            });
        });
        let state = self.state.read(cx);
        let appearance = self.appearance;
        let mut input = Input::new(&state.input)
            .blur_on_click_outside(false)
            .appearance(appearance.input)
            .flex_1()
            .min_w_0()
            .h(px(52.))
            .px_4()
            .border_0()
            .rounded_none()
            .bg(transparent_black());
        input.style().text = self.style.text.clone();
        let dismiss = self.state.clone();
        let accessible_dismiss = self.state.clone();
        let header = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .border_b_1()
            .border_color(appearance.divider)
            .child(input)
            .when(self.in_modal, |header| {
                header.child(
                    div()
                        .id("dismiss")
                        .role(Role::Button)
                        .aria_label("Close commands")
                        .mr_3()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .text_color(appearance.muted)
                        .cursor_pointer()
                        .hover(move |s| s.bg(appearance.active_background))
                        .on_click(move |_, window, cx| {
                            dismiss.update(cx, |state, cx| state.dismiss(window, cx));
                        })
                        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                            accessible_dismiss.update(cx, |state, cx| state.dismiss(window, cx));
                        })
                        .child("Esc"),
                )
            });
        let mut list = div()
            .id("commands")
            .role(Role::ListBox)
            .aria_label(self.label.clone())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&state.scroll)
            .p_2();
        let rows = state.rows();
        let items = state.items.clone();
        let active = state.active.clone();
        if state.loading || rows.is_empty() {
            list = list.child(div().px_3().py_6().text_color(appearance.muted).child(
                if state.loading {
                    self.loading_text
                } else {
                    self.empty_text
                },
            ));
        } else {
            for (row_index, row) in rows.into_iter().enumerate() {
                let index = match row {
                    Row::Group(label) => {
                        list = list.child(
                            div()
                                .id(("group", row_index))
                                .px_3()
                                .pt_3()
                                .pb_2()
                                .text_xs()
                                .text_color(appearance.muted)
                                .child(label),
                        );
                        continue;
                    }
                    Row::Item(index) => index,
                };
                let item = &items[index];
                let flags = CommandItemState {
                    highlighted: active.as_ref() == Some(&item.id),
                    disabled: item.disabled,
                };
                let content = if let Some(render) = &self.renderer {
                    render(item, flags, window, cx)
                } else {
                    default_item(item, appearance)
                };
                let choose = self.state.clone();
                let accessible = self.state.clone();
                let id = item.id.clone();
                let accessible_id = id.clone();
                list = list.child(
                    div()
                        .id(("command", index))
                        .role(Role::ListBoxOption)
                        .aria_label(item.label.clone())
                        .aria_selected(flags.highlighted)
                        .aria_disabled(flags.disabled)
                        .when(flags.highlighted, |row| {
                            row.aria_active_descendant()
                                .bg(appearance.active_background)
                        })
                        .when(flags.disabled, |row| {
                            row.opacity(appearance.disabled_opacity)
                        })
                        .when(!flags.disabled, |row| {
                            row.cursor_pointer()
                                .hover(move |s| s.bg(appearance.active_background))
                        })
                        .px_3()
                        .py_3()
                        .rounded_lg()
                        .on_click(move |_, window, cx| {
                            choose.update(cx, |state, cx| state.invoke(&id, window, cx));
                            cx.stop_propagation();
                        })
                        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                            accessible
                                .update(cx, |state, cx| state.invoke(&accessible_id, window, cx));
                        })
                        .child(content),
                );
            }
        }
        let keyboard = self.state.clone();
        let outside = self.state.clone();
        let root = div()
            .id(("command-palette", self.state.entity_id()))
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .when(!self.in_modal, |root| {
                root.on_mouse_down_out(move |event, window, cx| {
                    if event.button == MouseButton::Left
                        && outside.focus_handle(cx).is_focused(window)
                    {
                        window.blur();
                    }
                })
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                let state = keyboard.read(cx);
                if state.input.read(cx).is_composing()
                    || state.input.read(cx).is_disabled()
                    || event.keystroke.is_ime_in_progress()
                {
                    return;
                }
                let modifiers = event.keystroke.modifiers;
                let tab = event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                    && !modifiers.function
                    && state.modal_lifetime.upgrade().is_some();
                let action = self
                    .key_bindings
                    .iter()
                    .rev()
                    .find(|(key, _)| event.keystroke.should_match(key))
                    .map(|(_, action)| *action);
                let handled = keyboard.update(cx, |state, cx| {
                    match action {
                        Some(CommandPaletteAction::Previous) => state.navigate(false, cx),
                        Some(CommandPaletteAction::Next) => state.navigate(true, cx),
                        Some(CommandPaletteAction::Confirm) => state.accept(window, cx),
                        Some(CommandPaletteAction::Dismiss) => state.dismiss(window, cx),
                        None if tab => window.focus(&state.focus_handle(cx), cx),
                        _ => return false,
                    }
                    true
                });
                if handled {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            });
        let mut style = self.style;
        if self.in_modal {
            let insets = window.insets();
            let safe = insets.effective();
            let (top, bottom, handle) = match self.placement.unwrap() {
                ModalPlacement::Top { offset } => (offset, safe.bottom + px(16.), px(0.)),
                ModalPlacement::Center => (safe.top + px(16.), safe.bottom + px(16.), px(0.)),
                ModalPlacement::Bottom {
                    avoid_safe_area,
                    drag_to_dismiss,
                } => (
                    if avoid_safe_area { safe.top } else { px(0.) },
                    if avoid_safe_area {
                        safe.bottom
                    } else {
                        (insets.ime.bottom - insets.consumed.bottom).max(px(0.))
                    },
                    if drag_to_dismiss {
                        modal::SHEET_HANDLE_HEIGHT
                    } else {
                        px(0.)
                    },
                ),
            };
            let available_height =
                (window.viewport_size().height - top - bottom - handle).max(px(0.));
            let available_width =
                (window.viewport_size().width - safe.left - safe.right - px(32.)).max(px(0.));
            let limit = |value: Option<Length>, available: Pixels, axis: Pixels| match value {
                Some(Length::Definite(value)) => value
                    .to_pixels(axis.into(), window.rem_size())
                    .min(available),
                _ => available,
            };
            let height = limit(
                style.max_size.height,
                available_height,
                window.viewport_size().height,
            );
            let width = limit(
                style.max_size.width,
                available_width,
                window.viewport_size().width,
            );
            style = style.max_h(height).max_w(width);
        }
        let mut root = root.child(header).child(list);
        if let Some(surface) = self.surface {
            surface(
                root.flex_1().w_full().into_any_element(),
                &style,
                window,
                cx,
            )
        } else {
            root = root.bg(rgb(0xffffff));
            root.style().refine(&style);
            root.into_any_element()
        }
    }
}
fn default_item(item: &CommandItem, appearance: CommandPaletteAppearance) -> AnyElement {
    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().truncate().child(item.label.clone()))
        .when(!item.description.is_empty(), |text| {
            text.child(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .text_color(appearance.muted)
                    .child(item.description.clone()),
            )
        });
    div()
        .flex()
        .items_center()
        .gap_3()
        .when_some(item.icon.clone(), |row, path| {
            row.child(
                svg()
                    .path(path)
                    .size(px(18.))
                    .flex_shrink_0()
                    .text_color(appearance.muted),
            )
        })
        .child(text)
        .when_some(item.shortcut.clone(), |row, shortcut| {
            row.child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(appearance.muted)
                    .child(shortcut),
            )
        })
        .into_any_element()
}

#[cfg(test)]
mod tests;
