use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::animated_presence;
use gpui_platform::application;

struct Preview {
    visible: [bool; 3],
    clicks: [usize; 3],
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
        let duration = Duration::from_millis(if self.slow { 900 } else { 240 });
        let columns = (0..3)
            .map(|index| {
                let color = [0xa2b8ff, 0x77dfc2, 0xffc38c][index];
                let panel = div()
                    .id(("panel", index))
                    .w_full()
                    .p(px(24.))
                    .rounded(px(20.))
                    .bg(rgb(0x202c3c))
                    .border_1()
                    .border_color(rgb(0x34465b))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(color))
                            .child("YOUR WORKSPACE"),
                    )
                    .child(
                        div()
                            .text_size(px(25.))
                            .child(["Stay in focus", "Make some room", "Keep it close"][index]),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(0xa5b4c9))
                            .child("A small panel, ready when you need it."),
                    )
                    .child(
                        div()
                            .id("action")
                            .mt(px(12.))
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(px(9.))
                            .bg(rgb(0x304259))
                            .text_size(px(13.))
                            .cursor_pointer()
                            .child(format!("Explore  →  {}", self.clicks[index]))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.clicks[index] += 1;
                                cx.notify();
                            })),
                    );
                div()
                    .flex_1()
                    .min_w(px(230.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(0xa5b4c9))
                            .child(["01 / FADE", "02 / SLIDE UP", "03 / SLIDE IN"][index]),
                    )
                    .child(
                        div()
                            .id(("toggle", index))
                            .px(px(14.))
                            .py(px(10.))
                            .rounded(px(10.))
                            .bg(rgb(0x202736))
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x303a4c)))
                            .child(if self.visible[index] {
                                "Hide panel"
                            } else {
                                "Show panel"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.visible[index] = !this.visible[index];
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w_full()
                            .min_h(px(380.))
                            .p(px(16.))
                            .rounded(px(24.))
                            .bg(rgb(0x121b28))
                            .flex()
                            .flex_col()
                            .gap(px(16.))
                            .child(
                                animated_presence(
                                    ("presence", index),
                                    self.visible[index],
                                    move |frame| {
                                        let offset = px(18. * (1. - frame.progress));
                                        panel
                                            .opacity(frame.progress)
                                            .relative()
                                            .top(if index == 1 { offset } else { px(0.) })
                                            .left(if index == 2 { offset } else { px(0.) })
                                    },
                                )
                                .duration(duration)
                                .enabled(self.motion),
                            )
                            .child(
                                div()
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(rgb(0x2a384b))
                                    .p(px(16.))
                                    .text_size(px(12.))
                                    .text_color(rgb(0x8192ad))
                                    .child("Following content"),
                            ),
                    )
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
                    .child("ANIMATED PRESENCE"),
            )
            .child(div().text_size(px(38.)).child("Here when you need it."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Show, hide, or change your mind halfway through."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button("all", "Toggle all").on_click(cx.listener(|this, _, _, cx| {
                            let visible = !this.visible.iter().all(|v| *v);
                            this.visible.fill(visible);
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "speed",
                            if self.slow {
                                "Duration: 900ms"
                            } else {
                                "Duration: 240ms"
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
            .child(div().flex().flex_wrap().gap(px(24.)).children(columns))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(820.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Animated presence");
                cx.new(|_| Preview {
                    visible: [true; 3],
                    clicks: [0; 3],
                    slow: false,
                    motion: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
