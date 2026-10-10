mod input;
mod state;
pub use state::{SplitPaneEvent, SplitPaneState};

use gpui::{
    AccessibleAction, Along, AnyElement, App, Axis, CursorStyle, Entity, Focusable, HitboxBehavior,
    Hsla, IntoElement, Orientation, Pixels, Refineable, Role, SharedString, StyleRefinement,
    Window, canvas, container_query, div, prelude::*, px, rgb,
};
use state::{Limits, Pointer, interval};

#[derive(Clone, Copy)]
pub struct SplitPaneAppearance {
    pub divider: Hsla,
    pub active_divider: Hsla,
    pub focus_ring: Hsla,
}
impl Default for SplitPaneAppearance {
    fn default() -> Self {
        Self {
            divider: rgb(0xc8d2e1).into(),
            active_divider: rgb(0x547bd6).into(),
            focus_ring: rgb(0x547bd6).into(),
        }
    }
}

/// Two panes separated by a draggable, keyboard-focusable divider.
#[derive(IntoElement)]
pub struct SplitPane {
    state: Entity<SplitPaneState>,
    first: AnyElement,
    second: AnyElement,
    axis: Axis,
    limits: [Limits; 2],
    handle_size: Pixels,
    label: SharedString,
    appearance: SplitPaneAppearance,
    style: StyleRefinement,
}
impl SplitPane {
    pub fn new(
        state: &Entity<SplitPaneState>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        Self {
            state: state.clone(),
            first: first.into_any_element(),
            second: second.into_any_element(),
            axis: Axis::Horizontal,
            limits: [Limits::default(); 2],
            handle_size: px(8.),
            label: "Resize panes".into(),
            appearance: SplitPaneAppearance::default(),
            style: StyleRefinement::default().size_full().min_w_0().min_h_0(),
        }
    }
    /// Horizontal places panes side by side; vertical places the first pane above the second.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }
    pub fn first_min_size(mut self, size: Pixels) -> Self {
        self.limits[0].min = size.into();
        self
    }
    pub fn first_max_size(mut self, size: Pixels) -> Self {
        self.limits[0].max = size.into();
        self
    }
    pub fn second_min_size(mut self, size: Pixels) -> Self {
        self.limits[1].min = size.into();
        self
    }
    pub fn second_max_size(mut self, size: Pixels) -> Self {
        self.limits[1].max = size.into();
        self
    }
    /// Space reserved for the divider, including its pointer and touch target.
    pub fn handle_size(mut self, size: Pixels) -> Self {
        assert!(
            f32::from(size).is_finite() && size > px(0.),
            "handle size must be positive"
        );
        self.handle_size = size;
        self
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
    pub fn appearance(mut self, appearance: SplitPaneAppearance) -> Self {
        self.appearance = appearance;
        self
    }
}
impl Styled for SplitPane {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for SplitPane {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        for limits in self.limits {
            limits.validate();
        }
        let mut root = div()
            .id(("split-pane", self.state.entity_id()))
            .relative()
            .overflow_hidden();
        root.style().refine(&self.style);
        root.child(container_query(move |size, _, cx| {
            let total = f32::from(size.along(self.axis)).max(0.);
            let handle_size = f32::from(self.handle_size).min(total);
            let space = total - handle_size;
            let bounds = interval(space, self.limits[0], self.limits[1]);
            self.state
                .update(cx, |state, cx| state.measure(self.axis, space, bounds, cx));
            let state = self.state.read(cx);
            let first_size = state.first;
            let focus = state.focus_handle(cx);
            let dragging = state.is_dragging();
            let horizontal = self.axis == Axis::Horizontal;
            let appearance = self.appearance;
            let group: SharedString = format!("split-divider-{:?}", self.state.entity_id()).into();
            let keyboard = self.state.clone();
            let increment = self.state.clone();
            let decrement = self.state.clone();
            let measure = self.state.downgrade();
            let events = measure.clone();
            let listener = canvas(
                move |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    let _ = measure.update(cx, |state, _| {
                        if state.capture.is_some() && window.captured_hitbox() == state.capture {
                            if state.owns(Pointer::Mouse) {
                                window.capture_pointer(hitbox.id);
                            } else {
                                window.release_pointer();
                            }
                        }
                        state.capture = Some(hitbox.id);
                    });
                    hitbox
                },
                move |_, hitbox, window, _| input::register(events, hitbox, window),
            )
            .absolute()
            .inset_0();
            let handle = div()
                .id("divider")
                .group(group.clone())
                .relative()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(gpui::transparent_black())
                .track_focus(&focus)
                .tab_stop(true)
                .role(Role::Splitter)
                .aria_label(self.label)
                .aria_orientation(if horizontal {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                })
                .aria_numeric_value(first_size as f64)
                .aria_min_numeric_value(bounds.0 as f64)
                .aria_max_numeric_value(bounds.1 as f64)
                .aria_numeric_value_step(8.)
                .cursor(if horizontal {
                    CursorStyle::ResizeLeftRight
                } else {
                    CursorStyle::ResizeUpDown
                })
                .when(horizontal, |handle| handle.w(px(handle_size)).h_full())
                .when(!horizontal, |handle| handle.h(px(handle_size)).w_full())
                .focus_visible(move |style| style.border_color(appearance.focus_ring))
                .on_key_down(move |event, window, cx| {
                    let modifiers = event.keystroke.modifiers;
                    if modifiers.control
                        || modifiers.alt
                        || modifiers.platform
                        || modifiers.function
                    {
                        return;
                    }
                    let handled = keyboard.update(cx, |state, cx| {
                        let delta = if modifiers.shift { 32. } else { 8. };
                        match event.keystroke.key.as_str() {
                            "left" if horizontal => state.adjust(-delta, cx),
                            "right" if horizontal => state.adjust(delta, cx),
                            "up" if !horizontal => state.adjust(-delta, cx),
                            "down" if !horizontal => state.adjust(delta, cx),
                            "home" => state.edge(false, cx),
                            "end" => state.edge(true, cx),
                            "enter" => state.reset(cx),
                            "escape" if state.is_dragging() => {
                                state.cancel(cx);
                                window.release_pointer();
                            }
                            _ => return false,
                        }
                        true
                    });
                    if handled {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
                .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                    increment.update(cx, |state, cx| state.adjust(8., cx));
                })
                .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                    decrement.update(cx, |state, cx| state.adjust(-8., cx));
                })
                .child(
                    div()
                        .rounded_full()
                        .bg(if dragging {
                            appearance.active_divider
                        } else {
                            appearance.divider
                        })
                        .group_hover(group, move |style| style.bg(appearance.active_divider))
                        .when(horizontal, |line| line.w(px(2.)).h(px(24.)))
                        .when(!horizontal, |line| line.h(px(2.)).w(px(24.))),
                )
                .child(listener);
            let pane = |content, length| {
                div()
                    .min_w_0()
                    .min_h_0()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .when(horizontal, |pane| pane.w(px(length)).h_full())
                    .when(!horizontal, |pane| pane.h(px(length)).w_full())
                    .child(content)
            };
            div()
                .size_full()
                .flex()
                .when(!horizontal, |root| root.flex_col())
                .child(pane(self.first, first_size))
                .child(handle)
                .child(pane(self.second, space - first_size))
        }))
    }
}

#[cfg(test)]
mod tests;
