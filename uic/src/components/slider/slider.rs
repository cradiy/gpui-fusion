use std::rc::Rc;

use gpui::{
    AccessibleAction, AnyElement, App, Background, CursorStyle, Entity, IntoElement, KeyDownEvent,
    Orientation, Refineable as _, RenderOnce, Role, SharedString, StyleRefinement, Styled, Window,
    div, prelude::*, px, relative, transparent_black,
};

use super::{
    SliderAppearance, SliderState, SliderThumbVisibility, SliderTooltipPlacement,
    interaction::{InteractionPhase, SliderInteraction},
};

type ValueTooltipRenderer = Rc<dyn Fn(f64, &mut Window, &mut App) -> AnyElement>;

struct ValueTooltip {
    placement: SliderTooltipPlacement,
    render: ValueTooltipRenderer,
}

#[derive(IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    label: Option<SharedString>,
    secondary_value: Option<f64>,
    appearance: SliderAppearance,
    track_content: Option<AnyElement>,
    secondary_content: Option<AnyElement>,
    active_content: Option<AnyElement>,
    thumb_content: Option<AnyElement>,
    thumb_visibility: SliderThumbVisibility,
    hover_track_height: Option<gpui::Pixels>,
    value_tooltip: Option<ValueTooltip>,
    style: StyleRefinement,
}

impl Slider {
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            state: state.clone(),
            label: None,
            secondary_value: None,
            appearance: SliderAppearance::default(),
            track_content: None,
            secondary_content: None,
            active_content: None,
            thumb_content: None,
            thumb_visibility: SliderThumbVisibility::Always,
            hover_track_height: None,
            value_tooltip: None,
            style: StyleRefinement::default()
                .relative()
                .w_full()
                .h(px(28.))
                .rounded(px(8.))
                .border_2()
                .border_color(transparent_black()),
        }
    }

    /// Adds a secondary layer, such as buffered media.
    pub fn secondary_value(mut self, value: f64) -> Self {
        self.secondary_value = Some(value);
        self
    }

    pub fn appearance(mut self, appearance: SliderAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Sets the accessible name of the slider.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds arbitrary content over the complete internal track.
    pub fn track_content(mut self, content: impl IntoElement) -> Self {
        self.track_content = Some(content.into_any_element());
        self
    }

    /// Adds arbitrary content clipped to the secondary track.
    pub fn secondary_content(mut self, content: impl IntoElement) -> Self {
        self.secondary_content = Some(content.into_any_element());
        self
    }

    /// Adds arbitrary content clipped to the active track.
    pub fn active_content(mut self, content: impl IntoElement) -> Self {
        self.active_content = Some(content.into_any_element());
        self
    }

    /// Adds arbitrary content inside the thumb surface.
    pub fn thumb_content(mut self, content: impl IntoElement) -> Self {
        self.thumb_content = Some(content.into_any_element());
        self
    }

    /// Controls whether the draggable thumb is rendered.
    pub fn thumb_visible(mut self, visible: bool) -> Self {
        self.thumb_visibility = if visible {
            SliderThumbVisibility::Always
        } else {
            SliderThumbVisibility::Never
        };
        self
    }

    /// Selects whether the thumb is always visible, hover-only, or absent.
    pub fn thumb_visibility(mut self, visibility: SliderThumbVisibility) -> Self {
        self.thumb_visibility = visibility;
        self
    }

    /// Sets the visible track height while hovered or dragged.
    pub fn hover_track_height(mut self, height: gpui::Pixels) -> Self {
        self.hover_track_height = Some(height.max(px(0.)));
        self
    }

    /// Renders the value under the pointer above or below the slider.
    pub fn value_tooltip<E>(
        mut self,
        placement: SliderTooltipPlacement,
        render: impl Fn(f64, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement,
    {
        self.value_tooltip = Some(ValueTooltip {
            placement,
            render: Rc::new(move |value, window, cx| render(value, window, cx).into_any_element()),
        });
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut appearance = self.appearance;
        let (
            value,
            ratio,
            secondary_ratio,
            min,
            max,
            step,
            disabled,
            dragging,
            hovered,
            hover_ratio,
            hover_value,
            focus,
            capture,
        ) = {
            let state = self.state.read(cx);
            (
                state.value(),
                state.ratio(),
                self.secondary_value.map(|value| state.ratio_for(value)),
                state.min(),
                state.max(),
                state.effective_step(),
                state.is_disabled(),
                state.is_dragging(),
                state.is_hovered(),
                state.hover_ratio(),
                state
                    .hover_ratio()
                    .map(|ratio| state.value_for_ratio(ratio)),
                state.focus(),
                state.capture(),
            )
        };
        let active_interaction = dragging || hovered;
        let thumb_visible = match self.thumb_visibility {
            SliderThumbVisibility::Always => true,
            SliderThumbVisibility::Hover => active_interaction,
            SliderThumbVisibility::Never => false,
        };
        let reserve_thumb_space = self.thumb_visibility != SliderThumbVisibility::Never;
        let half_thumb = if reserve_thumb_space {
            appearance.thumb_size / 2.
        } else {
            px(0.)
        };
        let mut track_style = appearance.style().clone();
        if active_interaction && let Some(height) = self.hover_track_height {
            track_style = track_style.h(height);
        }
        let cursor = self.style.mouse_cursor.unwrap_or(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::ResizeLeftRight
        });
        let secondary = secondary_ratio.map(|ratio| {
            slider_track_layer(
                ratio,
                appearance.secondary_track,
                self.secondary_content,
                &track_style,
            )
        });
        let active = slider_track_layer(
            ratio,
            appearance.active_track,
            self.active_content,
            &track_style,
        );

        let mut track_surface = div()
            .relative()
            .children(
                self.track_content
                    .map(|content| div().absolute().inset_0().child(content)),
            )
            .children(secondary)
            .child(active);
        track_surface.style().refine(&track_style);

        let thumb = thumb_visible.then(|| {
            div()
                .absolute()
                .debug_selector(|| "uic-slider-thumb".to_string())
                .left(relative(ratio))
                .top(relative(0.5))
                .ml(-half_thumb)
                .mt(-half_thumb)
                .size(appearance.thumb_size)
                .rounded_full()
                .border_2()
                .border_color(appearance.thumb_border)
                .bg(appearance.thumb)
                .flex()
                .items_center()
                .justify_center()
                .children(self.thumb_content)
        });
        let value_tooltip = match (self.value_tooltip, hover_ratio, hover_value) {
            (Some(tooltip), Some(ratio), Some(value)) if active_interaction && !disabled => {
                let content = (tooltip.render)(value, window, cx);
                let anchor = div()
                    .absolute()
                    .left(relative(ratio))
                    .w(px(0.))
                    .flex()
                    .justify_center()
                    .child(div().flex_none().child(content));
                Some(match tooltip.placement {
                    SliderTooltipPlacement::Top => anchor.bottom_full().mb_2(),
                    SliderTooltipPlacement::Bottom => anchor.top_full().mt_2(),
                })
            }
            _ => None,
        };
        let track = div()
            .absolute()
            .left(half_thumb)
            .right(half_thumb)
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(track_surface)
            .children(thumb)
            .children(value_tooltip);

        let pointer_state = self.state.clone();
        let hover_state = self.state.clone();
        let keyboard_state = self.state.clone();
        let increment_state = self.state.clone();
        let decrement_state = self.state.clone();
        let interaction = (!disabled).then(|| {
            SliderInteraction::new(capture, focus.clone(), move |ratio, phase, _, cx| {
                pointer_state.update(cx, |state, cx| match phase {
                    InteractionPhase::Hover => state.preview_hover_ratio(ratio, cx),
                    InteractionPhase::Start => state.start_drag(ratio, cx),
                    InteractionPhase::Preview => state.preview_ratio(ratio, cx),
                    InteractionPhase::Commit => state.commit_ratio(ratio, cx),
                });
            })
            .absolute()
            .inset_0()
            .cursor(cursor)
            .edge_inset(half_thumb)
        });

        let mut element = div()
            .id(("uic-slider", self.state.entity_id()))
            .debug_selector(|| "uic-slider".to_string())
            .track_focus(&focus)
            .tab_stop(!disabled)
            .role(Role::Slider)
            .aria_disabled(disabled)
            .when_some(self.label, |this, label| this.aria_label(label))
            .aria_numeric_value(value)
            .aria_numeric_value_step(step)
            .aria_min_numeric_value(min)
            .aria_max_numeric_value(max)
            .aria_orientation(Orientation::Horizontal)
            .cursor(cursor)
            .on_hover(move |hovered, _, cx| {
                hover_state.update(cx, |state, cx| state.set_hovered(*hovered, cx));
            })
            .on_key_down(move |event, _, cx| {
                adjust_from_key(&keyboard_state, event, cx);
            })
            .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                increment_state.update(cx, |state, cx| {
                    state.commit_delta(state.effective_step(), cx);
                });
            })
            .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                decrement_state.update(cx, |state, cx| {
                    state.commit_delta(-state.effective_step(), cx);
                });
            })
            .opacity(if disabled { 0.5 } else { 1.0 })
            .child(track)
            .children(interaction);
        element.style().refine(&self.style);
        element.focus_visible(move |style| style.border_color(appearance.focus_ring))
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn slider_track_layer(
    ratio: f32,
    background: Background,
    content: Option<AnyElement>,
    track_style: &StyleRefinement,
) -> gpui::Div {
    let mut layer = div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(relative(ratio))
        .overflow_hidden()
        .bg(background)
        .children(content);
    layer.style().corner_radii.refine(&track_style.corner_radii);
    layer
}

fn adjust_from_key(state: &Entity<SliderState>, event: &KeyDownEvent, cx: &mut App) {
    let key = event.keystroke.key.as_str();
    let recognized = matches!(
        key,
        "left" | "down" | "right" | "up" | "home" | "end" | "pageup" | "pagedown"
    );
    if !recognized || state.read(cx).is_disabled() {
        return;
    }

    state.update(cx, |state, cx| {
        let step = state.effective_step();
        match key {
            "left" | "down" => state.commit_delta(-step, cx),
            "right" | "up" => state.commit_delta(step, cx),
            "home" => state.commit_value(state.min(), cx),
            "end" => state.commit_value(state.max(), cx),
            "pageup" => state.commit_delta(step * 10.0, cx),
            "pagedown" => state.commit_delta(-step * 10.0, cx),
            _ => {}
        }
    });
    cx.stop_propagation();
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{
        Context, Entity, Modifiers, MouseButton, Render, Subscription, TestAppContext,
        VisualTestContext, point, size,
    };

    use super::*;
    use crate::components::slider::SliderEvent;

    struct TestSlider {
        state: Entity<SliderState>,
        events: Rc<RefCell<Vec<SliderEvent>>>,
        thumb_visibility: SliderThumbVisibility,
        tooltip_placement: Option<SliderTooltipPlacement>,
        tooltip_value: Rc<RefCell<Option<f64>>>,
        _subscription: Subscription,
    }

    impl Render for TestSlider {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut slider = Slider::new(&self.state)
                .thumb_visibility(self.thumb_visibility)
                .hover_track_height(px(10.))
                .track_content(div().size_full())
                .active_content(div().size_full())
                .thumb_content(div().size(px(6.)).rounded_full());
            if let Some(placement) = self.tooltip_placement {
                let tooltip_value = self.tooltip_value.clone();
                slider = slider.value_tooltip(placement, move |value, _, _| {
                    *tooltip_value.borrow_mut() = Some(value);
                    div()
                        .debug_selector(|| "uic-slider-value-tooltip".to_string())
                        .w(px(48.))
                        .h(px(20.))
                });
            }
            div().w(px(320.)).mt(px(60.)).child(slider)
        }
    }

    fn open_test_slider(cx: &mut TestAppContext) -> gpui::WindowHandle<TestSlider> {
        cx.open_window(size(px(500.), px(180.)), |_, cx| {
            let state = cx.new(|cx| SliderState::new(25., 0.0..=100.0, cx).step(1.));
            let events = Rc::new(RefCell::new(Vec::new()));
            let events_for_subscription = events.clone();
            let subscription = cx.subscribe(&state, move |_, _, event, _| {
                events_for_subscription.borrow_mut().push(*event);
            });
            TestSlider {
                state,
                events,
                thumb_visibility: SliderThumbVisibility::Always,
                tooltip_placement: None,
                tooltip_value: Rc::new(RefCell::new(None)),
                _subscription: subscription,
            }
        })
    }

    fn draw(window: &gpui::WindowHandle<TestSlider>, cx: &mut TestAppContext) -> VisualTestContext {
        let mut visual = VisualTestContext::from_window((*window).into(), cx);
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        visual
    }

    #[gpui::test]
    fn dragging_clamps_and_separates_changing_from_changed(cx: &mut TestAppContext) {
        let window = open_test_slider(cx);
        let mut visual = draw(&window, cx);
        let bounds = visual
            .debug_bounds("uic-slider")
            .expect("slider should be rendered");
        let start = point(bounds.left() + bounds.size.width * 0.5, bounds.center().y);
        let outside = point(bounds.right() + px(50.), bounds.center().y);

        visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        window
            .update(&mut visual.cx, |view, _, cx| {
                assert!(view.state.read(cx).is_dragging());
            })
            .unwrap();
        visual.simulate_mouse_move(outside, MouseButton::Left, Modifiers::default());
        visual.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });

        window
            .update(&mut visual.cx, |view, _, cx| {
                assert_eq!(view.state.read(cx).value(), 100.0);
                assert!(!view.state.read(cx).is_dragging());
                assert!(matches!(
                    view.events.borrow().last(),
                    Some(SliderEvent::Changed(100.0))
                ));
                assert!(
                    view.events
                        .borrow()
                        .iter()
                        .any(|event| matches!(event, SliderEvent::Changing(_)))
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn keyboard_and_accessibility_steps_commit_values(cx: &mut TestAppContext) {
        let window = open_test_slider(cx);
        let mut visual = draw(&window, cx);
        let bounds = visual.debug_bounds("uic-slider").unwrap();
        let quarter = point(bounds.left() + bounds.size.width * 0.25, bounds.center().y);
        visual.simulate_mouse_down(quarter, MouseButton::Left, Modifiers::default());
        visual.simulate_mouse_up(quarter, MouseButton::Left, Modifiers::default());
        window
            .update(&mut visual.cx, |view, _, cx| {
                view.state.update(cx, |state, cx| state.set_value(25.0, cx));
                view.events.borrow_mut().clear()
            })
            .unwrap();

        visual.simulate_keystrokes("right pageup");
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });

        window
            .update(&mut visual.cx, |view, _, cx| {
                assert_eq!(view.state.read(cx).value(), 36.0);
                assert_eq!(
                    view.events.borrow().as_slice(),
                    &[SliderEvent::Changed(26.0), SliderEvent::Changed(36.0)]
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn thumb_can_be_removed_without_replacing_the_slider(cx: &mut TestAppContext) {
        let window = open_test_slider(cx);
        let mut visual = draw(&window, cx);
        assert!(visual.debug_bounds("uic-slider-thumb").is_some());

        window
            .update(&mut visual.cx, |view, _, cx| {
                view.thumb_visibility = SliderThumbVisibility::Never;
                cx.notify();
            })
            .unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });

        assert!(visual.debug_bounds("uic-slider-thumb").is_none());
    }

    #[gpui::test]
    fn hover_thumb_appears_without_changing_the_outer_layout(cx: &mut TestAppContext) {
        let window = open_test_slider(cx);
        window
            .update(cx, |view, _, cx| {
                view.thumb_visibility = SliderThumbVisibility::Hover;
                cx.notify();
            })
            .unwrap();
        let mut visual = draw(&window, cx);
        let bounds = visual.debug_bounds("uic-slider").unwrap();
        assert!(visual.debug_bounds("uic-slider-thumb").is_none());

        visual.simulate_mouse_move(bounds.center(), None::<MouseButton>, Modifiers::default());
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });

        assert_eq!(visual.debug_bounds("uic-slider").unwrap(), bounds);
        assert!(visual.debug_bounds("uic-slider-thumb").is_some());
        window
            .update(&mut visual.cx, |view, _, cx| {
                assert!(view.state.read(cx).is_hovered());
            })
            .unwrap();
    }

    #[gpui::test]
    fn value_tooltip_tracks_hover_value_and_placement(cx: &mut TestAppContext) {
        let window = open_test_slider(cx);
        window
            .update(cx, |view, _, cx| {
                view.tooltip_placement = Some(SliderTooltipPlacement::Top);
                cx.notify();
            })
            .unwrap();
        let mut visual = draw(&window, cx);
        let slider_bounds = visual.debug_bounds("uic-slider").unwrap();
        assert!(visual.debug_bounds("uic-slider-value-tooltip").is_none());

        let half_thumb = px(9.);
        let numeric_width = slider_bounds.size.width - half_thumb * 2.;
        let pointer = point(
            slider_bounds.left() + half_thumb + numeric_width * 0.75,
            slider_bounds.center().y,
        );
        visual.simulate_mouse_move(pointer, None::<MouseButton>, Modifiers::default());
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });

        let top_bounds = visual
            .debug_bounds("uic-slider-value-tooltip")
            .expect("tooltip should render above the hovered value");
        assert!(top_bounds.bottom() <= slider_bounds.top());
        window
            .update(&mut visual.cx, |view, _, cx| {
                let value = view.tooltip_value.borrow().unwrap();
                assert!((value - 75.0).abs() <= 1.0);
                assert_eq!(view.state.read(cx).value(), 25.0);
            })
            .unwrap();

        window
            .update(&mut visual.cx, |view, _, cx| {
                view.tooltip_placement = Some(SliderTooltipPlacement::Bottom);
                cx.notify();
            })
            .unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let bottom_bounds = visual.debug_bounds("uic-slider-value-tooltip").unwrap();
        assert!(bottom_bounds.top() >= slider_bounds.bottom());

        visual.simulate_mouse_move(
            point(slider_bounds.right() + px(40.), slider_bounds.center().y),
            None::<MouseButton>,
            Modifiers::default(),
        );
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert!(visual.debug_bounds("uic-slider-value-tooltip").is_none());
    }
}
