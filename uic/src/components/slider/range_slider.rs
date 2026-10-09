use gpui::{
    AccessibleAction, App, Entity, KeyDownEvent, Orientation, Refineable, RenderOnce, Role,
    SharedString, StyleRefinement, Styled, Window, div, prelude::*, px, relative,
};

use super::{RangeSliderState, RangeSliderThumb, SliderAppearance, interaction::SliderInteraction};

/// A horizontal interval selector with two independently focusable endpoints.
#[derive(IntoElement)]
pub struct RangeSlider {
    state: Entity<RangeSliderState>,
    label: SharedString,
    lower_label: SharedString,
    upper_label: SharedString,
    appearance: SliderAppearance,
    style: StyleRefinement,
}

impl RangeSlider {
    pub fn new(state: &Entity<RangeSliderState>) -> Self {
        Self {
            state: state.clone(),
            label: "Range".into(),
            lower_label: "Minimum".into(),
            upper_label: "Maximum".into(),
            appearance: SliderAppearance::default(),
            style: StyleRefinement::default().relative().w_full().h(px(36.)),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn thumb_labels(
        mut self,
        lower: impl Into<SharedString>,
        upper: impl Into<SharedString>,
    ) -> Self {
        self.lower_label = lower.into();
        self.upper_label = upper.into();
        self
    }

    pub fn appearance(mut self, appearance: SliderAppearance) -> Self {
        self.appearance = appearance;
        self
    }
}

impl Styled for RangeSlider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RangeSlider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = self.state.read(cx);
        let lower = data.ratio(RangeSliderThumb::Lower);
        let upper = data.ratio(RangeSliderThumb::Upper);
        let disabled = data.is_disabled();
        let mut appearance = self.appearance;
        let half_thumb = appearance.thumb_size / 2.;
        let track_style = appearance.style().clone();
        let mut fill = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(lower))
            .w(relative(upper - lower))
            .bg(appearance.active_track.clone());
        fill.style().corner_radii.refine(&track_style.corner_radii);
        let mut surface = div().relative().child(fill);
        surface.style().refine(&track_style);
        let mut track = div()
            .absolute()
            .left(half_thumb)
            .right(half_thumb)
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(surface);

        for (thumb, label, ratio) in [
            (RangeSliderThumb::Lower, self.lower_label, lower),
            (RangeSliderThumb::Upper, self.upper_label, upper),
        ] {
            let focus = data.thumb_focus_handle(thumb).tab_stop(!disabled);
            let limits = data.limits(thumb);
            let keyboard = self.state.clone();
            let increment = self.state.clone();
            let decrement = self.state.clone();
            let node = div()
                .id(match thumb {
                    RangeSliderThumb::Lower => "lower",
                    RangeSliderThumb::Upper => "upper",
                })
                .debug_selector(move || match thumb {
                    RangeSliderThumb::Lower => "uic-range-slider-lower".into(),
                    RangeSliderThumb::Upper => "uic-range-slider-upper".into(),
                })
                .absolute()
                .left(relative(ratio))
                .top(relative(0.5))
                .ml(-half_thumb)
                .mt(-half_thumb)
                .size(appearance.thumb_size)
                .rounded_full()
                .border_2()
                .border_color(appearance.thumb_border)
                .bg(appearance.thumb.clone())
                .track_focus(&focus)
                .role(Role::Slider)
                .aria_label(format!("{}: {label}", self.label))
                .aria_disabled(disabled)
                .aria_orientation(Orientation::Horizontal)
                .aria_numeric_value(data.value(thumb))
                .aria_numeric_value_step(data.effective_step())
                .aria_min_numeric_value(*limits.start())
                .aria_max_numeric_value(*limits.end())
                .on_key_down(move |event, window, cx| {
                    adjust_from_key(&keyboard, thumb, event, window, cx)
                })
                .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                    increment.update(cx, |state, cx| {
                        state.commit(thumb, state.value(thumb) + state.effective_step(), cx)
                    });
                })
                .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                    decrement.update(cx, |state, cx| {
                        state.commit(thumb, state.value(thumb) - state.effective_step(), cx)
                    });
                })
                .focus_visible(move |style| {
                    style
                        .border_color(appearance.focus_ring)
                        .bg(appearance.focus_ring)
                });
            track = track.child(node);
        }
        let pointer = self.state.clone();
        let interaction = (!disabled).then(|| {
            SliderInteraction::new(
                data.capture.clone(),
                data.thumb_focus_handle(RangeSliderThumb::Lower),
                move |ratio, phase, window, cx| {
                    if let Some(focus) =
                        pointer.update(cx, |state, cx| state.pointer(ratio, phase, cx))
                    {
                        focus.focus(window, cx);
                    }
                },
            )
            .absolute()
            .inset_0()
            .edge_inset(half_thumb)
            .cursor_ew_resize()
        });
        let mut root = div()
            .id(("uic-range-slider", self.state.entity_id()))
            .debug_selector(|| "uic-range-slider".into())
            .role(Role::Group)
            .aria_label(self.label)
            .aria_disabled(disabled)
            .child(track)
            .children(interaction);
        root.style().refine(&self.style);
        if disabled {
            root = root.opacity(0.5);
        }
        root
    }
}

fn adjust_from_key(
    state: &Entity<RangeSliderState>,
    thumb: RangeSliderThumb,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if state.read(cx).is_disabled() {
        return;
    }
    let key = event.keystroke.key.as_str();
    let modifiers = event.keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.alt {
        return;
    }
    if key == "tab" {
        if modifiers.shift {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
        window.prevent_default();
        cx.stop_propagation();
        return;
    }
    if !matches!(
        key,
        "left" | "right" | "up" | "down" | "home" | "end" | "pageup" | "pagedown"
    ) {
        return;
    }
    state.update(cx, |state, cx| {
        let value = state.value(thumb);
        let step = state.effective_step();
        let next = match key {
            "left" | "down" => value - step,
            "right" | "up" => value + step,
            "pageup" => value + step * 10.,
            "pagedown" => value - step * 10.,
            "home" => *state.limits(thumb).start(),
            "end" => *state.limits(thumb).end(),
            _ => value,
        };
        state.commit(thumb, next, cx);
    });
    window.prevent_default();
    cx.stop_propagation();
}
