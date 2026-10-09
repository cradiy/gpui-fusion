mod state;
pub use state::{NumberInputChanged, NumberInputOptions, NumberInputState};

use crate::components::input::{Input, InputAppearance};
use gpui::{
    AccessibleAction, AnyElement, App, Entity, Focusable, Hsla, KeyDownEvent, MouseButton,
    PathBuilder, Refineable, RenderOnce, Role, SharedString, StyleRefinement, Styled, Window,
    canvas, div, point, prelude::*, px, rgb, transparent_black,
};

#[derive(Clone, Copy)]
pub struct NumberInputAppearance {
    pub input: InputAppearance,
    pub button_hover: Hsla,
    pub disabled_opacity: f32,
}
impl Default for NumberInputAppearance {
    fn default() -> Self {
        Self {
            input: InputAppearance::default(),
            button_hover: gpui::hsla(0.61, 0.35, 0.5, 0.08),
            disabled_opacity: 0.45,
        }
    }
}

/// Decimal input with explicit commit, cancellation and step controls.
#[derive(IntoElement)]
pub struct NumberInput {
    state: Entity<NumberInputState>,
    label: SharedString,
    controls: bool,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    appearance: NumberInputAppearance,
    style: StyleRefinement,
}
impl NumberInput {
    pub fn new(state: &Entity<NumberInputState>) -> Self {
        Self {
            state: state.clone(),
            label: "Value".into(),
            controls: true,
            prefix: None,
            suffix: None,
            appearance: NumberInputAppearance::default(),
            style: StyleRefinement::default()
                .w_full()
                .h(px(46.))
                .pl_3()
                .pr(px(6.))
                .gap_2()
                .rounded_lg()
                .border_1()
                .border_color(rgb(0xd6deea))
                .bg(rgb(0xffffff))
                .text_color(rgb(0x25354b))
                .text_size(px(16.))
                .line_height(px(24.)),
        }
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
    pub fn controls(mut self, controls: bool) -> Self {
        self.controls = controls;
        self
    }
    pub fn prefix(mut self, content: impl IntoElement) -> Self {
        self.prefix = Some(content.into_any_element());
        self
    }
    pub fn suffix(mut self, content: impl IntoElement) -> Self {
        self.suffix = Some(content.into_any_element());
        self
    }
    pub fn appearance(mut self, appearance: NumberInputAppearance) -> Self {
        self.appearance = appearance;
        self
    }
}
impl Styled for NumberInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for NumberInput {
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
        let disabled = state.is_disabled(cx);
        let focus = state.focus_handle(cx);
        let options = state.options();
        let value = state.value();
        let down_enabled = state.can_adjust(false, cx);
        let up_enabled = state.can_adjust(true, cx);
        let mut input = Input::new(&state.input)
            .appearance(self.appearance.input)
            .flex_1()
            .min_w_0()
            .w_auto()
            .h_full()
            .px_0()
            .border_0()
            .rounded_none()
            .bg(transparent_black());
        let has_trailing = self.controls || self.suffix.is_some();
        let trailing = div()
            .h_full()
            .flex()
            .items_center()
            .gap_2()
            .children(self.suffix)
            .when(self.controls, |trailing| {
                trailing.child(
                    div()
                        .w(px(32.))
                        .h_full()
                        .flex_shrink_0()
                        .py(px(4.))
                        .pl(px(5.))
                        .flex()
                        .flex_col()
                        .child(step_button(
                            &self.state,
                            true,
                            up_enabled,
                            self.appearance.button_hover,
                        ))
                        .child(step_button(
                            &self.state,
                            false,
                            down_enabled,
                            self.appearance.button_hover,
                        )),
                )
            });
        input = input
            .gap_2()
            .when_some(self.prefix, |input, prefix| input.prefix(prefix))
            .when(has_trailing, |input| input.suffix(trailing));
        input.style().text = self.style.text.clone();
        let keyboard = self.state.clone();
        let increment = self.state.clone();
        let decrement = self.state.clone();
        let mut root = div()
            .id(("number-input", self.state.entity_id()))
            .role(Role::SpinButton)
            .aria_label(self.label.clone())
            .aria_disabled(disabled)
            .aria_numeric_value(value)
            .aria_numeric_value_step(options.step)
            .when_some(options.min, |root, min| root.aria_min_numeric_value(min))
            .when_some(options.max, |root, max| root.aria_max_numeric_value(max))
            .flex()
            .items_center()
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                if keyboard.read(cx).is_disabled(cx)
                    || keyboard.read(cx).input.read(cx).is_composing()
                {
                    return;
                }
                let handled = keyboard.update(cx, |state, cx| match event.keystroke.key.as_str() {
                    "up" => {
                        state.increment(cx);
                        true
                    }
                    "down" => {
                        state.decrement(cx);
                        true
                    }
                    "enter" => {
                        state.commit(cx);
                        true
                    }
                    "escape" => {
                        state.cancel(cx);
                        true
                    }
                    _ => false,
                });
                if handled {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                increment.update(cx, |state, cx| state.increment(cx))
            })
            .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                decrement.update(cx, |state, cx| state.decrement(cx))
            });
        root.style().refine(&self.style);
        root.when(disabled, |root| {
            root.opacity(self.appearance.disabled_opacity)
        })
        .when(!disabled && focus.is_focused(window), |root| {
            root.border_color(self.appearance.input.focus_border)
        })
        .child(input)
    }
}
fn step_button(
    state: &Entity<NumberInputState>,
    forward: bool,
    enabled: bool,
    hover: Hsla,
) -> impl IntoElement {
    let click = state.clone();
    let accessible = state.clone();
    div()
        .id(if forward { "increment" } else { "decrement" })
        .role(Role::Button)
        .aria_label(if forward { "Increase" } else { "Decrease" })
        .aria_disabled(!enabled)
        .w_full()
        .flex_1()
        .min_h_0()
        .rounded(px(3.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let center = bounds.center();
                    let direction = if forward { -1. } else { 1. };
                    let mut path = PathBuilder::stroke(px(1.5));
                    path.move_to(center + point(px(-3.5), px(-1.75 * direction)));
                    path.line_to(center + point(px(0.), px(1.75 * direction)));
                    path.line_to(center + point(px(3.5), px(-1.75 * direction)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, window.text_style().color);
                    }
                },
            )
            .size(px(12.)),
        )
        .when(!enabled, |button| button.opacity(0.3))
        .when(enabled, |button| {
            button.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            if enabled {
                click.update(cx, |state, cx| {
                    if forward {
                        state.increment(cx)
                    } else {
                        state.decrement(cx)
                    }
                });
            }
            cx.stop_propagation();
        })
        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
            if enabled {
                accessible.update(cx, |state, cx| {
                    if forward {
                        state.increment(cx)
                    } else {
                        state.decrement(cx)
                    }
                });
            }
        })
}

#[cfg(test)]
mod tests;
