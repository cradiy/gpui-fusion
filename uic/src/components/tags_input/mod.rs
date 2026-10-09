mod state;
pub use state::{TagsInputChanged, TagsInputError, TagsInputOptions, TagsInputState};

use crate::components::input::{Backspace, Delete, Input, InputAppearance, Left, Right};
use gpui::{
    AccessibleAction, App, Entity, Focusable, Hsla, KeyDownEvent, MouseButton, PathBuilder,
    Refineable, RenderOnce, Role, SharedString, StyleRefinement, Styled, Window, canvas, div,
    point, prelude::*, px, rgb, transparent_black,
};

#[derive(Clone, Copy)]
pub struct TagsInputAppearance {
    pub input: InputAppearance,
    pub tag_background: Hsla,
    pub tag_foreground: Hsla,
    pub selected_tag_background: Hsla,
    pub remove_hover: Hsla,
    pub error_border: Hsla,
    pub disabled_opacity: f32,
}

impl Default for TagsInputAppearance {
    fn default() -> Self {
        Self {
            input: InputAppearance::default(),
            tag_background: rgb(0xeff3fa).into(),
            tag_foreground: rgb(0x46618a).into(),
            selected_tag_background: rgb(0xdce7fc).into(),
            remove_hover: gpui::hsla(0.61, 0.3, 0.5, 0.12),
            error_border: rgb(0xc55764).into(),
            disabled_opacity: 0.45,
        }
    }
}

/// A wrapping tag editor. Enter adds the draft; empty-draft Backspace selects before deleting.
#[derive(IntoElement)]
pub struct TagsInput {
    state: Entity<TagsInputState>,
    label: SharedString,
    placeholder: SharedString,
    appearance: TagsInputAppearance,
    style: StyleRefinement,
}
impl TagsInput {
    pub fn new(state: &Entity<TagsInputState>) -> Self {
        Self {
            state: state.clone(),
            label: "Tags".into(),
            placeholder: "Add a tag".into(),
            appearance: TagsInputAppearance::default(),
            style: StyleRefinement::default()
                .w_full()
                .min_h(px(48.))
                .p_2()
                .gap_2()
                .rounded_lg()
                .border_1()
                .border_color(rgb(0xd6deea))
                .bg(rgb(0xffffff))
                .text_color(rgb(0x25354b))
                .text_size(px(15.))
                .line_height(px(22.)),
        }
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    pub fn appearance(mut self, appearance: TagsInputAppearance) -> Self {
        self.appearance = appearance;
        self
    }
}
impl Styled for TagsInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for TagsInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            state.input.update(cx, |input, _| {
                input.set_placeholder(self.placeholder.clone())
            });
            if state.label != self.label {
                state.label = self.label.clone();
                state
                    .input
                    .update(cx, |input, cx| input.set_aria_label(self.label.clone(), cx));
            }
        });
        let data = self.state.read(cx);
        let tags = data.tags().to_vec();
        let selected = data.selected.clone();
        let disabled = data.is_disabled(cx);
        let error = data.error().map(ToString::to_string);
        let focus = data.focus_handle(cx);
        let click_focus = focus.clone();
        let outside_focus = focus.clone();
        let mut editor = Input::new(&data.input)
            .blur_on_click_outside(false)
            .appearance(self.appearance.input)
            .w_full()
            .h(px(30.))
            .px_1()
            .border_0()
            .rounded_none()
            .bg(transparent_black());
        editor.style().text = self.style.text.clone();
        let keyboard = self.state.clone();
        let backspace = self.state.clone();
        let delete = self.state.clone();
        let left = self.state.clone();
        let right = self.state.clone();
        let edit = self.state.clone();
        let mut root = div()
            .id(("tags-input", self.state.entity_id()))
            .role(Role::Group)
            .aria_label(self.label.clone())
            .aria_disabled(disabled)
            .flex()
            .flex_wrap()
            .items_center()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !disabled {
                    click_focus.focus(window, cx);
                }
            })
            .on_mouse_down_out(move |event, window, _| {
                if event.button == MouseButton::Left && outside_focus.is_focused(window) {
                    window.blur();
                }
            })
            .capture_action(move |_: &Backspace, _, cx| {
                handle_empty_action(&backspace, "backspace", cx)
            })
            .capture_action(move |_: &Delete, _, cx| handle_empty_action(&delete, "delete", cx))
            .capture_action(move |_: &Left, _, cx| handle_empty_action(&left, "left", cx))
            .capture_action(move |_: &Right, _, cx| handle_empty_action(&right, "right", cx))
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                let data = keyboard.read(cx);
                if data.is_disabled(cx)
                    || data.input.read(cx).is_composing()
                    || event.keystroke.modifiers.control
                    || event.keystroke.modifiers.platform
                    || event.keystroke.modifiers.alt
                    || event.keystroke.modifiers.shift
                {
                    return;
                }
                let handled = keyboard.update(cx, |state, cx| match event.keystroke.key.as_str() {
                    "enter" => {
                        state.submit(cx);
                        true
                    }
                    "escape" if state.selected.is_some() || state.error().is_some() => {
                        state.cancel_selection(cx);
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
        if disabled {
            root = root.opacity(self.appearance.disabled_opacity);
        } else if error.is_some() {
            root = root.border_color(self.appearance.error_border);
        } else if focus.is_focused(window) {
            root = root.border_color(self.appearance.input.focus_border);
        }
        for tag in tags {
            let choose = self.state.clone();
            let chosen = tag.clone();
            let remove = self.state.clone();
            let removed = tag.clone();
            let accessible = self.state.clone();
            let accessible_tag = tag.clone();
            let active = selected.as_ref() == Some(&tag);
            let chip = div()
                .id(tag.clone())
                .max_w_full()
                .min_w_0()
                .h(px(30.))
                .pl_3()
                .pr_1()
                .gap_2()
                .rounded(px(6.))
                .flex()
                .items_center()
                .bg(if active {
                    self.appearance.selected_tag_background
                } else {
                    self.appearance.tag_background
                })
                .text_color(self.appearance.tag_foreground)
                .on_click(move |_, window, cx| {
                    choose.update(cx, |state, cx| {
                        if !state.is_disabled(cx) {
                            state.selected = Some(chosen.clone());
                            state.focus_handle(cx).focus(window, cx);
                            cx.notify();
                        }
                    });
                    cx.stop_propagation();
                })
                .child(div().min_w_0().truncate().child(tag.clone()))
                .child(
                    div()
                        .id("remove")
                        .role(Role::Button)
                        .aria_label(format!("Remove {tag}"))
                        .aria_disabled(disabled)
                        .size(px(22.))
                        .flex_shrink_0()
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!disabled, |button| {
                            button
                                .cursor_pointer()
                                .hover(move |style| style.bg(self.appearance.remove_hover))
                        })
                        .child(
                            canvas(
                                |_, _, _| (),
                                |bounds, _, window, _| {
                                    let center = bounds.center();
                                    let mut path = PathBuilder::stroke(px(1.25));
                                    path.move_to(center + point(px(-2.5), px(-2.5)));
                                    path.line_to(center + point(px(2.5), px(2.5)));
                                    path.move_to(center + point(px(2.5), px(-2.5)));
                                    path.line_to(center + point(px(-2.5), px(2.5)));
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, window.text_style().color);
                                    }
                                },
                            )
                            .size(px(12.)),
                        )
                        .on_click(move |_, _, cx| {
                            remove.update(cx, |state, cx| {
                                if !state.is_disabled(cx) {
                                    state.remove(&removed, cx);
                                }
                            });
                            cx.stop_propagation();
                        })
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            accessible.update(cx, |state, cx| {
                                if !state.is_disabled(cx) {
                                    state.remove(&accessible_tag, cx);
                                }
                            });
                        }),
                );
            root = root.child(chip);
        }
        root.child(
            div()
                .min_w(px(100.))
                .max_w_full()
                .flex_1()
                .h(px(30.))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    edit.update(cx, |state, cx| state.cancel_selection(cx));
                })
                .child(editor),
        )
    }
}

fn handle_empty_action(state: &Entity<TagsInputState>, action: &str, cx: &mut App) {
    let data = state.read(cx);
    if data.is_disabled(cx) || data.input.read(cx).is_composing() || !data.draft(cx).is_empty() {
        return;
    }
    let handled = state.update(cx, |state, cx| match action {
        "backspace" => {
            state.backspace(cx);
            true
        }
        "delete" => {
            if let Some(tag) = state.selected.clone() {
                state.remove(&tag, cx);
                true
            } else {
                false
            }
        }
        "left" => {
            state.navigate(false, cx);
            true
        }
        "right" => {
            state.navigate(true, cx);
            true
        }
        _ => false,
    });
    if handled {
        cx.stop_propagation();
    }
}

#[cfg(test)]
mod tests;
