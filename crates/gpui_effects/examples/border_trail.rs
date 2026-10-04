use std::time::Instant;

use gpui::{
    AnimationFramePolicy, App, BorderGradient, Bounds, Context, Render, Window, WindowBounds,
    WindowOptions, border_color_stop, border_gradient, div, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{BorderTrailMode, BorderTrailOptions, border_trail};
use gpui_platform::application;

struct Preview {
    phase: f32,
    last_frame: Instant,
    paused: bool,
    reverse: bool,
    dark: bool,
    gradient: bool,
    fixed_border: bool,
    palette: BorderGradient,
    clicks: [usize; 3],
}

impl Preview {
    fn palette(dark: bool) -> BorderGradient {
        border_gradient([
            border_color_stop(rgb(if dark { 0x77dfc2 } else { 0x197f68 }), 0.),
            border_color_stop(rgb(0x598bff), 0.18),
            border_color_stop(rgb(if dark { 0xad83ff } else { 0x8054ca }), 0.38),
            border_color_stop(rgb(if dark { 0xff7bad } else { 0xc5487a }), 0.6),
            border_color_stop(rgb(0xf5b967), 0.8),
            border_color_stop(rgb(if dark { 0x77dfc2 } else { 0x197f68 }), 1.),
        ])
    }

    fn trail(&self, offset: f32, color: u32) -> gpui::Div {
        let mut options = BorderTrailOptions {
            progress: self.phase + offset,
            length: px(if self.gradient { 320. } else { 220. }),
            width: px(2.5),
            color: rgb(color),
            reverse: self.reverse,
            mode: if self.fixed_border {
                BorderTrailMode::Border
            } else {
                BorderTrailMode::Trail
            },
            ..Default::default()
        };
        if self.gradient {
            options = options.gradient(self.palette.clone());
        }
        border_trail(options)
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if !self.paused {
            let direction = if self.reverse { -1. } else { 1. };
            self.phase = (self.phase
                + direction * now.duration_since(self.last_frame).as_secs_f32() / 4.)
                .rem_euclid(1.);
            window.request_animation_frame();
        }
        self.last_frame = now;
        let surface = if self.dark { 0x151c28 } else { 0xffffff };
        let text = if self.dark { 0xecf1fc } else { 0x202c42 };
        let muted = if self.dark { 0x8e9ab1 } else { 0x61718a };
        let line = if self.dark { 0xffffff16 } else { 0x18223822 };
        let control = |id, label| {
            div()
                .id(id)
                .px(px(14.))
                .py(px(10.))
                .rounded(px(10.))
                .bg(rgb(if self.dark { 0x202b3c } else { 0xe2e8f2 }))
                .text_size(px(12.))
                .cursor_pointer()
                .child(label)
        };
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(if self.dark { 0x0c1018 } else { 0xf4f6fa }))
            .text_color(rgb(text))
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(muted))
                    .child("BORDER / LIGHT"),
            )
            .child(div().text_size(px(36.)).child("Follow the edge."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(muted))
                    .child("A quiet signal that something is in motion."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        control("pause", if self.paused { "Resume" } else { "Pause" }).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.paused = !this.paused;
                                this.last_frame = Instant::now();
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        control(
                            "direction",
                            if self.reverse {
                                "Counterclockwise"
                            } else {
                                "Clockwise"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reverse = !this.reverse;
                            this.last_frame = Instant::now();
                            cx.notify();
                        })),
                    )
                    .child(
                        control(
                            "anchor",
                            if self.fixed_border {
                                "Fixed border"
                            } else {
                                "Moving colors"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.fixed_border = !this.fixed_border;
                            cx.notify();
                        })),
                    )
                    .child(
                        control("color", if self.gradient { "Gradient" } else { "Solid" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.gradient = !this.gradient;
                                cx.notify();
                            })),
                    )
                    .child(
                        control(
                            "theme",
                            if self.dark {
                                "Light surface"
                            } else {
                                "Dark surface"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dark = !this.dark;
                            this.palette = Self::palette(this.dark);
                            cx.notify();
                        })),
                    ),
            )
            .child(
                self.trail(0., if self.dark { 0xb1c6ff } else { 0x496bcc })
                    .w_full()
                    .min_h(px(76.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(rgb(surface))
                    .border_1()
                    .border_color(rgba(line))
                    .px(px(26.))
                    .py(px(18.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(rgb(0x8faaff))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(15.))
                            .child(if self.paused {
                                "Your next idea starts here."
                            } else {
                                "Making room for your next idea…"
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(muted))
                            .child(if self.paused { "PAUSED" } else { "WORKING" }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap(px(16.))
                    .children((0..3).map(|i| {
                        let (title, detail, color) = [
                            (
                                "Compose",
                                "A thought, taking shape.",
                                if self.dark { 0xa2b8ff } else { 0x496bcc },
                            ),
                            (
                                "Sync",
                                "Everything, up to date.",
                                if self.dark { 0x77dfc2 } else { 0x197f68 },
                            ),
                            (
                                "Publish",
                                "Ready for the next chapter.",
                                if self.dark { 0xffc38c } else { 0xb76a2b },
                            ),
                        ][i];
                        self.trail(i as f32 * 0.28, color)
                            .flex_1()
                            .min_w(px(230.))
                            .h(px(244.))
                            .p(px(22.))
                            .rounded(px(22.))
                            .border_1()
                            .border_color(rgba(line))
                            .bg(rgb(surface))
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(muted))
                                            .child(format!("0{}", i + 1)),
                                    )
                                    .child(div().size(px(7.)).rounded_full().bg(rgb(color))),
                            )
                            .child(div().mt(px(12.)).text_size(px(24.)).child(title))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(muted))
                                    .child(detail),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id(("open", i))
                                    .text_size(px(13.))
                                    .text_color(rgb(color))
                                    .cursor_pointer()
                                    .child(if self.clicks[i] == 0 {
                                        "Explore  →".into()
                                    } else {
                                        format!("Opened {} times", self.clicks[i])
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.clicks[i] += 1;
                                        cx.notify();
                                    })),
                            )
                    })),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                animation_frame_policy: AnimationFramePolicy::FollowDisplay,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(660.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Border trail");
                cx.new(|_| Preview {
                    phase: 0.2,
                    last_frame: Instant::now(),
                    paused: false,
                    reverse: false,
                    dark: true,
                    gradient: true,
                    fixed_border: true,
                    palette: Preview::palette(true),
                    clicks: [0; 3],
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
