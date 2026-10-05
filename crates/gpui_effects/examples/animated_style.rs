use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::animated_style;
use gpui_platform::application;

struct Preview {
    hovered: [bool; 3],
    card_hovered: [bool; 3],
    selected: usize,
    disabled: bool,
    slow: bool,
    motion: bool,
    clicks: usize,
}

fn button(id: &'static str, label: impl Into<gpui::SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(rgb(0x202736))
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x303a4c)))
        .child(label.into())
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let duration = Duration::from_millis(if self.slow { 900 } else { 180 });
        let actions = (0..3)
            .map(|index| {
                let hovered = self.hovered[index] && !self.disabled;
                let accent = [0xa2b8ff, 0x77dfc2, 0xffc38c][index];
                animated_style(("action", index))
                    .w(px(160.))
                    .h(px(52.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(if hovered {
                        [0x344568, 0x244e46, 0x574332][index]
                    } else {
                        0x202c3e
                    }))
                    .text_color(rgb(if hovered { accent } else { 0xb5c3d7 }))
                    .border_1()
                    .border_color(rgb(if hovered { accent } else { 0x34445c }))
                    .rounded(px(if hovered { 20. } else { 10. }))
                    .opacity(if self.disabled { 0.4 } else { 1. })
                    .duration(duration)
                    .enabled(self.motion)
                    .text_size(px(14.))
                    .when(!self.disabled, |item| item.cursor_pointer())
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if this.hovered[index] != *hovered {
                            this.hovered[index] = *hovered;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.disabled {
                            this.clicks += 1;
                            cx.notify();
                        }
                    }))
                    .child(["Create a note  +", "Save a draft  ✓", "Keep exploring  →"][index])
            })
            .collect::<Vec<_>>();
        let cards = (0..3)
            .map(|index| {
                let selected = self.selected == index;
                let hovered = self.card_hovered[index];
                let color = [0xa2b8ff, 0x77dfc2, 0xffc38c][index];
                animated_style(("card", index))
                    .w_full()
                    .p(px(22.))
                    .rounded(px(if selected { 24. } else { 14. }))
                    .bg(rgb(if selected {
                        [0x273550, 0x203b36, 0x40332b][index]
                    } else if hovered {
                        0x202d40
                    } else {
                        0x182230
                    }))
                    .border_1()
                    .border_color(rgb(if selected {
                        color
                    } else if hovered {
                        0x50627c
                    } else {
                        0x2b3a4e
                    }))
                    .text_color(rgb(if selected { color } else { 0xc7d3e6 }))
                    .duration(duration)
                    .enabled(self.motion)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if this.card_hovered[index] != *hovered {
                            this.card_hovered[index] = *hovered;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = index;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(12.))
                            .flex_shrink_0()
                            .child(format!("0{}", index + 1)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(div().text_size(px(22.)).child(
                                [
                                    "Make a little space",
                                    "Keep a steady rhythm",
                                    "Follow a new direction",
                                ][index],
                            ))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(20.))
                                    .text_color(rgb(0x93a4be))
                                    .child(
                                        [
                                            "A quiet place for the ideas that matter.",
                                            "Small changes, one thoughtful step at a time.",
                                            "There is always something else to discover.",
                                        ][index],
                                    ),
                            ),
                    )
                    .child(div().flex_shrink_0().text_size(px(20.)).child(if selected {
                        "●"
                    } else {
                        "○"
                    }))
            })
            .collect::<Vec<_>>();

        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .p(px(32.))
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("ANIMATED STYLES"),
            )
            .child(div().text_size(px(38.)).child("A softer change of state."))
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Hover an action. Choose a card. Move away before it settles."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "disable",
                            if self.disabled {
                                "Actions: Disabled"
                            } else {
                                "Actions: Enabled"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.disabled = !this.disabled;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "pace",
                            if self.slow {
                                "Pace: Slow"
                            } else {
                                "Pace: Natural"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.slow = !this.slow;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "motion",
                            if self.motion {
                                "Motion: On"
                            } else {
                                "Motion: Off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.motion = !this.motion;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(24.))
                    .child(
                        div()
                            .w_full()
                            .p(px(22.))
                            .rounded(px(24.))
                            .bg(rgb(0x151e2b))
                            .border_1()
                            .border_color(rgb(0x28364a))
                            .flex()
                            .flex_col()
                            .gap(px(18.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(0x8192ad))
                                    .child("SMALL ACTIONS"),
                            )
                            .child(div().flex().flex_wrap().gap(px(12.)).children(actions))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0x8192ad))
                                    .child(format!("{} actions taken", self.clicks)),
                            ),
                    )
                    .child(div().flex().flex_col().gap(px(14.)).children(cards)),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(950.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Animated styles");
                cx.new(|_| Preview {
                    hovered: [false; 3],
                    card_hovered: [false; 3],
                    selected: 0,
                    disabled: false,
                    slow: false,
                    motion: true,
                    clicks: 0,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
