use gpui::{
    AnyElement, App, CursorStyle, Entity, IntoElement, MouseButton, Refineable as _, RenderOnce,
    StyleRefinement, Styled, Window, div, prelude::*, px,
};

use super::{InputAppearance, TextInput};
use crate::components::scrollbar::Scrollbar;

#[derive(IntoElement)]
pub struct Input {
    state: Entity<TextInput>,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    appearance: InputAppearance,
    configure_scrollbar: Option<Box<dyn FnOnce(Scrollbar) -> Scrollbar>>,
    rows: Option<usize>,
    blur_on_click_outside: bool,
    style: StyleRefinement,
}

input_appearance!(Input);

impl Input {
    pub fn new(state: &Entity<TextInput>) -> Self {
        Self {
            state: state.clone(),
            prefix: None,
            suffix: None,
            appearance: InputAppearance::default(),
            configure_scrollbar: None,
            rows: None,
            blur_on_click_outside: true,
            style: StyleRefinement::default(),
        }
    }

    /// Clears focus on a primary click outside the complete input surface.
    /// Disable this when a containing composite manages its own focus boundary.
    pub fn blur_on_click_outside(mut self, enabled: bool) -> Self {
        self.blur_on_click_outside = enabled;
        self
    }

    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    pub fn appearance(mut self, appearance: InputAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// Configures the multi-line input's scrollbar after its defaults are applied.
    ///
    /// Use Styled methods for the cursor, track size, position, and background;
    /// use `appearance` for thumb states and `auto_hide` for visibility behavior.
    /// Increase the input's right padding when widening the overlaid scrollbar.
    /// Repeated calls apply in order.
    pub fn scrollbar(mut self, configure: impl FnOnce(Scrollbar) -> Scrollbar + 'static) -> Self {
        let previous = self.configure_scrollbar.take();
        self.configure_scrollbar = Some(Box::new(move |scrollbar| {
            configure(match previous {
                Some(previous) => previous(scrollbar),
                None => scrollbar,
            })
        }));
        self
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let appearance = self.appearance;
        self.state
            .update(cx, |state, _cx| state.appearance = appearance);

        let (focused, focus_handle, disabled, multiline, scroll_handle, scrollbar_state) = {
            let state = self.state.read(cx);
            (
                state.focus_handle.is_focused(window),
                state.focus_handle.clone(),
                state.disabled,
                state.mode == super::InputMode::Multiline,
                state.scroll_handle.clone(),
                state.scrollbar_state.clone(),
            )
        };
        let outside_focus = focus_handle.clone();
        let scrollbar_id = ("uic-input-scrollbar", self.state.entity_id());

        let row_height = self
            .rows
            .map(|rows| super::row_height(&self.style, rows, window.rem_size()));

        let mut element = div()
            .relative()
            .flex()
            .when(multiline, |this| this.items_start())
            .when(!multiline, |this| this.items_center())
            .w_full()
            .h(px(44.))
            .when_some(row_height.filter(|_| multiline), |this, height| {
                this.h(height)
            })
            .px(px(14.))
            .when(multiline, |this| this.py(px(10.)))
            .gap(px(10.))
            .text_size(px(16.))
            .line_height(px(24.))
            .text_color(gpui::hsla(0., 0., 0.08, 1.))
            .rounded(px(10.))
            .border(px(1.))
            .border_color(if focused && !disabled {
                appearance.focus_border
            } else {
                gpui::hsla(0., 0., 0.75, 1.)
            })
            .bg(gpui::hsla(0., 0., 1., 1.))
            .opacity(if disabled { 0.6 } else { 1.0 })
            .cursor(if disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .when(self.blur_on_click_outside, |input| {
                input.on_mouse_down_out(move |event, window, _| {
                    if event.button == MouseButton::Left && outside_focus.is_focused(window) {
                        window.blur();
                    }
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !disabled {
                    window.focus(&focus_handle, cx);
                }
            })
            .children(self.prefix)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .when(multiline, |this| this.h_full())
                    .child(self.state),
            )
            .children(self.suffix)
            .when(multiline, |this| {
                this.child(
                    Scrollbar::vertical(scrollbar_id, &scrollbar_state, &scroll_handle)
                        .auto_hide(false)
                        .absolute()
                        .right(px(2.))
                        .top(px(4.))
                        .bottom(px(4.))
                        .h_auto()
                        .when_some(self.configure_scrollbar, |scrollbar, configure| {
                            configure(scrollbar)
                        }),
                )
            });
        element.style().refine(&self.style);
        if focused && !disabled {
            element = element.border_color(appearance.focus_border);
        }
        element
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Focusable, Render, TestAppContext, VisualTestContext, point, size};

    struct Example {
        first: Entity<TextInput>,
        second: Entity<TextInput>,
        clicks: usize,
    }
    impl Render for Example {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    Input::new(&self.first)
                        .w(px(280.))
                        .suffix(div().w(px(32.)).child("px")),
                )
                .child(Input::new(&self.second).w(px(280.)))
                .child(
                    div()
                        .id("action")
                        .w(px(280.))
                        .h(px(44.))
                        .child("Action")
                        .on_click(cx.listener(|this, _, _, _| this.clicks += 1)),
                )
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear());
    }

    #[gpui::test]
    fn outside_click_blurs_without_consuming_target_or_blurring_suffix(cx: &mut TestAppContext) {
        let view = cx.open_window(size(px(400.), px(300.)), |_, cx| Example {
            first: cx.new(|cx| TextInput::new(cx).initial_value("First")),
            second: cx.new(TextInput::new),
            clicks: 0,
        });
        let mut visual = VisualTestContext::from_window(view.into(), cx);
        visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        draw(&mut visual);
        visual.simulate_click(point(px(80.), px(38.)), Default::default());
        draw(&mut visual);
        visual.simulate_click(point(px(267.), px(38.)), Default::default());
        draw(&mut visual);
        view.update(&mut visual.cx, |this, window, cx| {
            assert!(this.first.focus_handle(cx).is_focused(window))
        })
        .unwrap();
        visual.simulate_click(point(px(350.), px(250.)), Default::default());
        draw(&mut visual);
        visual.update(|window, cx| assert!(window.focused(cx).is_none()));
        visual.simulate_click(point(px(80.), px(38.)), Default::default());
        draw(&mut visual);
        visual.simulate_click(point(px(80.), px(98.)), Default::default());
        draw(&mut visual);
        view.update(&mut visual.cx, |this, window, cx| {
            assert!(this.second.focus_handle(cx).is_focused(window))
        })
        .unwrap();
        visual.simulate_click(point(px(80.), px(158.)), Default::default());
        draw(&mut visual);
        view.update(&mut visual.cx, |this, window, cx| {
            assert!(window.focused(cx).is_none());
            assert_eq!(this.clicks, 1);
            assert_eq!(this.first.read(cx).value().as_ref(), "First");
        })
        .unwrap();
    }
}
