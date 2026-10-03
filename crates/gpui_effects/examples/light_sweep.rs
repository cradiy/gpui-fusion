use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{LightSweepOptions, light_sweep};
use gpui_platform::application;

const DURATION: Duration = Duration::from_millis(1400);

struct Preview {
    started: [Option<Instant>; 3],
    clicks: [usize; 3],
    dark: bool,
}

impl Preview {
    fn new() -> Self {
        Self {
            started: [Some(Instant::now()); 3],
            clicks: [0; 3],
            dark: true,
        }
    }

    fn card(&self, index: usize, progress: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let (number, title, detail, color) = [
            ("01", "Canvas", "A fresh perspective.", 0xa2b8ff),
            ("02", "Library", "Everything in its place.", 0x77dfc2),
            ("03", "Publish", "Your next moment.", 0xffc38c),
        ][index];
        light_sweep(LightSweepOptions {
            progress,
            width: px(140.),
            color: rgb(if self.dark { color } else { 0xffffff }),
            opacity: if self.dark { 0.22 } else { 0.7 },
            ..Default::default()
        })
        .id(("sweep-card", index))
        .flex_1()
        .min_w(px(230.))
        .h(px(290.))
        .p(px(24.))
        .rounded(px(22.))
        .border_1()
        .border_color(rgba(if self.dark { 0xffffff16 } else { 0x18223816 }))
        .bg(rgb(if self.dark { 0x151a24 } else { 0xdce4f0 }))
        .flex()
        .flex_col()
        .gap(px(14.))
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.started[index] = Some(Instant::now());
                cx.notify();
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x7a889f))
                        .child(number),
                )
                .child(div().size(px(8.)).rounded_full().bg(rgb(color))),
        )
        .child(div().mt(px(26.)).text_size(px(26.)).child(title))
        .child(
            div()
                .text_size(px(13.))
                .text_color(rgb(if self.dark { 0x96a2b8 } else { 0x56647b }))
                .child(detail),
        )
        .child(div().flex_1())
        .child(
            div()
                .id(("open", index))
                .automation_id(format!("open-{index}"))
                .w_full()
                .px(px(14.))
                .py(px(11.))
                .rounded(px(10.))
                .border_1()
                .border_color(rgba(if self.dark { 0xffffff18 } else { 0x18223818 }))
                .bg(rgba(if self.dark { 0xffffff05 } else { 0xffffff30 }))
                .text_size(px(13.))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(0xffffff25)))
                .child(if self.clicks[index] == 0 {
                    "Explore  →".into()
                } else {
                    format!("Opened {} times", self.clicks[index])
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.clicks[index] += 1;
                    this.started[index] = Some(Instant::now());
                    cx.notify();
                })),
        )
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let progress = self.started.each_mut().map(|started| {
            let Some(time) = *started else {
                return 1.;
            };
            let elapsed = now.duration_since(time);
            if elapsed >= DURATION {
                *started = None;
                1.
            } else {
                window.request_animation_frame();
                elapsed.as_secs_f32() / DURATION.as_secs_f32()
            }
        });
        let control = |id, label| {
            div()
                .id(id)
                .px(px(14.))
                .py(px(10.))
                .rounded(px(10.))
                .bg(rgb(if self.dark { 0x202736 } else { 0xe0e6ef }))
                .text_size(px(12.))
                .cursor_pointer()
                .child(label)
        };
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(if self.dark { 0x0c1018 } else { 0xf4f6fa }))
            .text_color(rgb(if self.dark { 0xecf1fc } else { 0x202c42 }))
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("SURFACE LIGHT"),
            )
            .child(div().text_size(px(38.)).child("A passing glow."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Hover a card. A quiet highlight passes through."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(control("replay", "Replay all").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.started = [Some(Instant::now()); 3];
                            cx.notify();
                        },
                    )))
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
                            this.started = [Some(Instant::now()); 3];
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap(px(16.))
                    .children((0..3).map(|i| self.card(i, progress[i], cx).into_any_element())),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(940.), px(630.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Light sweep");
                cx.new(|_| Preview::new())
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, TestAppContext};

    #[gpui::test]
    fn sweeping_surface_preserves_child_clicks_and_finishes(cx: &mut TestAppContext) {
        let handle = cx.open_window(size(px(940.), px(630.)), |_, _| Preview::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.set_automation_enabled(true).unwrap();
            window.draw(cx).clear();
            let position = window
                .automation_snapshot()
                .unwrap()
                .nodes
                .iter()
                .find(|node| node.automation_id.as_deref() == Some("open-0"))
                .unwrap()
                .bounds
                .unwrap()
                .center();
            for event in [
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    ..Default::default()
                }),
                PlatformInput::MouseDown(MouseDownEvent {
                    position,
                    ..Default::default()
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    position,
                    ..Default::default()
                }),
            ] {
                window.dispatch_event(event, cx);
                window.draw(cx).clear();
            }
        })
        .unwrap();
        handle
            .update(cx, |view, _, cx| {
                assert_eq!(view.clicks[0], 1);
                assert!(view.started[0].is_some());
                view.started = [Some(Instant::now() - DURATION - Duration::from_secs(1)); 3];
                cx.notify();
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
        handle
            .update(cx, |view, _, _| {
                assert!(
                    view.started.iter().all(Option::is_none),
                    "completed passes must become idle"
                );
            })
            .unwrap();
    }
}
