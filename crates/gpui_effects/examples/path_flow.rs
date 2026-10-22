use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, point, prelude::*, px,
    rgb, size,
};
use gpui_effects::{FlowPath, PathFlowOptions, path_flow};

struct Preview {
    last: Instant,
    elapsed: Duration,
    paused: bool,
    route: FlowPath,
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let dt = now.duration_since(self.last);
        self.last = now;
        let moving = !self.paused && !window.prefers_reduced_motion() && window.is_window_active();
        if moving {
            if dt < Duration::from_millis(200) {
                self.elapsed += dt;
            }
            window.request_animation_frame();
        }
        let progress = (self.elapsed.as_secs_f32() / 6.).fract();
        div()
            .size_full()
            .bg(rgb(0x101724))
            .text_color(rgb(0xe1eaf7))
            .p_8()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_2xl().child("Path flow"))
                    .child(
                        div()
                            .id("pause")
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x27374d))
                            .cursor_pointer()
                            .child(if self.paused { "Resume" } else { "Pause" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.paused = !this.paused;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9baec7))
                    .child("Motion respects the system preference."),
            )
            .child(div().text_lg().child("Distance-based route"))
            .child(
                path_flow(self.route.clone(), progress, PathFlowOptions::default())
                    .w_full()
                    .flex_1()
                    .min_h(px(160.))
                    .rounded_xl()
                    .bg(rgb(0x162233)),
            )
            .child(div().text_sm().text_color(rgb(0x9baec7)).child(format!(
                "Route progress: {:.0}% · resize the window to change its aspect ratio",
                progress * 100.
            )))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(860.), px(760.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Preview {
                    last: Instant::now(),
                    elapsed: Duration::ZERO,
                    paused: false,
                    route: FlowPath::new([
                        point(0.08, 0.65),
                        point(0.28, 0.22),
                        point(0.53, 0.78),
                        point(0.72, 0.25),
                        point(0.92, 0.45),
                    ])
                    .unwrap(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
