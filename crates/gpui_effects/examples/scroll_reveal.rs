use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, ScrollHandle, Stateful, Window, WindowBounds, WindowOptions,
    div, point, prelude::*, px, rgb, size,
};
use gpui_effects::scroll_reveal;
use gpui_platform::application;

struct Preview {
    scroll: ScrollHandle,
    once: bool,
    motion: bool,
    slow: bool,
    generation: usize,
    saved: [bool; 9],
}

fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(rgb(0x202736))
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x303a4c)))
        .child(label)
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let titles = [
            "A little room to think",
            "Collect the small things",
            "Find your own rhythm",
            "Leave a little space",
            "Try another direction",
            "Let the idea breathe",
            "Keep what matters",
            "Make something yours",
            "There is more ahead",
        ];
        let descriptions = [
            "Good ideas do not always arrive all at once. Start with a quiet corner.",
            "A passing thought, a useful detail, a color you want to remember.",
            "One small step can be enough to give the next one a shape.",
            "The pauses are part of the composition, too.",
            "Look at a familiar thought from somewhere you have not stood before.",
            "There is no need to finish every thought the moment it arrives.",
            "A few thoughtful choices can say more than a page full of noise.",
            "Follow the details that feel right, and see where they lead.",
            "Scroll back whenever you want to take a second look.",
        ];
        let cards = (0..9)
            .map(|index| {
                let accent = [0xa2b8ff, 0x77dfc2, 0xffc38c][index % 3];
                scroll_reveal(("card", index))
                    .w_full()
                    .flex_shrink_0()
                    .p(px(24.))
                    .rounded(px(22.))
                    .bg(rgb([0x1c293e, 0x1a302d, 0x322a25][index % 3]))
                    .border_1()
                    .border_color(rgb([0x344965, 0x34544b, 0x55463a][index % 3]))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .threshold(0.3)
                    .once(self.once)
                    .enabled(self.motion)
                    .duration(Duration::from_millis(if self.slow { 1000 } else { 420 }))
                    .delay(Duration::from_millis((index % 3) as u64 * 45))
                    .offset(point(px(0.), px(24.)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .text_size(px(12.))
                            .text_color(rgb(accent))
                            .child(format!("NOTE {:02}", index + 1))
                            .child("✦"),
                    )
                    .child(div().text_size(px(26.)).child(titles[index]))
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(23.))
                            .text_color(rgb(0xa4b2c8))
                            .child(descriptions[index]),
                    )
                    .child(
                        div().mt(px(6.)).flex().child(
                            div()
                                .id("save")
                                .px(px(12.))
                                .py(px(8.))
                                .rounded(px(10.))
                                .text_size(px(13.))
                                .text_color(rgb(accent))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui::rgba(0xffffff0c)))
                                .child(if self.saved[index] {
                                    "Saved  ✓"
                                } else {
                                    "Keep this thought  +"
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.saved[index] = !this.saved[index];
                                    cx.notify();
                                })),
                        ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .size_full()
            .p(px(32.))
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("SCROLL REVEAL"),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(38.))
                    .child("A thought at a time."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child(
                        "Scroll through the notes. Each one arrives when there is room to see it.",
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "once",
                            if self.once {
                                "Play: Once"
                            } else {
                                "Play: Repeat"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.once = !this.once;
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
                    )
                    .child(button("restart", "Start again  ↻").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.generation += 1;
                            this.scroll.set_offset(point(px(0.), px(0.)));
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .id("notes")
                    .w_full()
                    .max_w(px(760.))
                    .flex_1()
                    .min_h_0()
                    .rounded(px(26.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .bg(rgb(0x111925))
                    .border_1()
                    .border_color(rgb(0x253145))
                    .child(
                        div()
                            .id(("entries", self.generation))
                            .p(px(20.))
                            .flex()
                            .flex_col()
                            .gap(px(18.))
                            .children(cards)
                            .child(
                                div()
                                    .py(px(24.))
                                    .text_size(px(13.))
                                    .text_color(rgb(0x8192ad))
                                    .child("A good place to pause. Or begin again."),
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
                window.set_window_title("Scroll reveal");
                cx.new(|_| Preview {
                    scroll: ScrollHandle::default(),
                    once: true,
                    motion: true,
                    slow: false,
                    generation: 0,
                    saved: [false; 9],
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
