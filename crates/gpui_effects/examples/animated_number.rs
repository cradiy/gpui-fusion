use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::animated_number;
use gpui_platform::application;

struct Preview {
    count: f64,
    zoom: f64,
    progress: f64,
    slow: bool,
    motion: bool,
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
        let duration = Duration::from_millis(if self.slow { 1200 } else { 300 });
        let motion = self.motion;
        let metric = |id: &'static str,
                      title: &'static str,
                      detail: &'static str,
                      target: f64,
                      color: u32,
                      format: fn(f64) -> String| {
            div()
                .w_full()
                .p(px(24.))
                .rounded(px(20.))
                .bg(rgb(0x182230))
                .border_1()
                .border_color(rgb(0x2b3a4e))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .items_center()
                        .child(div().size(px(7.)).rounded_full().bg(rgb(color)))
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(rgb(0x9aacc5))
                                .child(title),
                        ),
                )
                .child(
                    animated_number(id, target, move |value| {
                        div()
                            .text_size(px(46.))
                            .line_height(px(56.))
                            .text_color(rgb(color))
                            .child(format(value))
                    })
                    .duration(duration)
                    .enabled(motion),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(rgb(0x8e9ab1))
                        .child(detail),
                )
        };
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
                    .child("ANIMATED NUMBERS"),
            )
            .child(div().text_size(px(38.)).child("Every change counts."))
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child(
                        "Small updates, smooth arrivals. Change a value again before it settles.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button("add", "+125 items").on_click(cx.listener(|this, _, _, cx| {
                            this.count = (this.count + 125.).min(99999.);
                            cx.notify();
                        })),
                    )
                    .child(button("remove", "−125 items").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.count = (this.count - 125.).max(-99999.);
                            cx.notify();
                        },
                    )))
                    .child(
                        button("zoom", "Change zoom").on_click(cx.listener(|this, _, _, cx| {
                            this.zoom = if this.zoom < 150. { 175.5 } else { 75. };
                            cx.notify();
                        })),
                    )
                    .child(button("progress", "Next step").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.progress = if this.progress >= 1. {
                                0.
                            } else {
                                (this.progress + 0.2).min(1.)
                            };
                            cx.notify();
                        },
                    )))
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
                    .gap(px(16.))
                    .child(metric(
                        "count",
                        "COLLECTION",
                        "A little more room for your ideas.",
                        self.count,
                        0xa2b8ff,
                        |v| format!("{v:.0} items"),
                    ))
                    .child(metric(
                        "zoom-value",
                        "CANVAS ZOOM",
                        "Move between the big picture and the details.",
                        self.zoom,
                        0x77dfc2,
                        |v| format!("{v:.1}%"),
                    ))
                    .child(
                        div()
                            .w_full()
                            .p(px(24.))
                            .rounded(px(20.))
                            .bg(rgb(0x182230))
                            .border_1()
                            .border_color(rgb(0x2b3a4e))
                            .flex()
                            .flex_col()
                            .gap(px(16.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0x9aacc5))
                                    .child("WORK IN PROGRESS"),
                            )
                            .child(
                                animated_number("progress-value", self.progress, |value| {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(18.))
                                        .child(
                                            div()
                                                .text_size(px(46.))
                                                .line_height(px(56.))
                                                .text_color(rgb(0xffc38c))
                                                .child(format!("{:.0}%", value * 100.)),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(6.))
                                                .rounded_full()
                                                .bg(rgb(0x2a384b))
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .w(gpui::relative(value as f32))
                                                        .rounded_full()
                                                        .bg(rgb(0xffc38c)),
                                                ),
                                        )
                                })
                                .duration(duration)
                                .enabled(motion),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0x8e9ab1))
                                    .child("One step closer to something good."),
                            ),
                    ),
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
                window.set_window_title("Animated numbers");
                cx.new(|_| Preview {
                    count: 1240.,
                    zoom: 100.,
                    progress: 0.4,
                    slow: false,
                    motion: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
