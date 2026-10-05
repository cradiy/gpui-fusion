use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::animated_collapse;
use gpui_platform::application;

struct Preview {
    expanded: [bool; 3],
    rows: usize,
    narrow: bool,
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
        let duration = Duration::from_millis(if self.slow { 900 } else { 260 });
        let panels = (0..3).map(|index| {
            let color = [0xa2b8ff, 0x77dfc2, 0xffc38c][index];
            let description = [
                "Keep your ideas together. Content can grow while this panel stays open, and everything below follows its height.",
                "A narrower workspace gives these words more lines. The panel measures its content again and moves to the new height.",
                "Close a panel and open it again before the animation finishes. It continues from its current height.",
            ][index];
            let mut content = div().w_full().px(px(22.)).pb(px(22.)).flex().flex_col().gap(px(12.))
                .child(div().text_size(px(14.)).line_height(px(22.)).text_color(rgb(0xa5b4c9)).child(description));
            if index == 0 {
                content = content.children((0..self.rows).map(|row| {
                    div().rounded(px(10.)).bg(rgb(0x253346)).px(px(14.)).py(px(12.))
                        .flex().gap(px(14.)).items_center()
                        .child(div().size(px(7.)).rounded_full().bg(rgb(color)).flex_shrink_0())
                        .child(div().text_size(px(13.)).child(format!("Study {:02} — A place for the next idea", row + 1)))
                }));
            }
            if index == 2 {
                content = content.child(button("explore", format!("Explore  →  {}", self.clicks))
                    .on_click(cx.listener(|this, _, _, cx| { this.clicks += 1; cx.notify(); })));
            }
            div().w_full().flex_shrink_0().rounded(px(20.)).overflow_hidden()
                .bg(rgb(0x182230)).border_1().border_color(rgb(0x2b3a4e))
                .flex().flex_col()
                .child(div().id(("header", index)).m(px(6.)).px(px(16.)).py(px(14.))
                    .rounded(px(14.)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0x202e40))).flex().items_center().gap(px(16.))
                    .child(div().text_size(px(11.)).text_color(rgb(color)).child(format!("0{}", index + 1)))
                    .child(div().flex_1().min_w_0().text_size(px(20.)).child(["Project notes", "Room to read", "Change your mind"][index]))
                    .child(div().text_color(rgb(0xa5b4c9)).text_size(px(20.)).child(if self.expanded[index] { "−" } else { "+" }))
                    .on_click(cx.listener(move |this, _, _, cx| { this.expanded[index] = !this.expanded[index]; cx.notify(); })))
                .child(animated_collapse(("details", index), self.expanded[index], move || content)
                    .duration(duration).enabled(self.motion))
        }).collect::<Vec<_>>();
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
                    .child("ANIMATED COLLAPSE"),
            )
            .child(div().text_size(px(38.)).child("Room for what matters."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Open a section. Add a note. Let the rest make room."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button("rows", format!("Notes: {}", self.rows)).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.rows = if this.rows == 5 { 1 } else { this.rows + 1 };
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        button(
                            "width",
                            if self.narrow {
                                "Width: Narrow"
                            } else {
                                "Width: Wide"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.narrow = !this.narrow;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "speed",
                            if self.slow {
                                "Duration: 900ms"
                            } else {
                                "Duration: 260ms"
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
                    .max_w(px(if self.narrow { 420. } else { 760. }))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .children(panels)
                    .child(
                        div()
                            .w_full()
                            .rounded(px(16.))
                            .border_1()
                            .border_color(rgb(0x2a384b))
                            .p(px(20.))
                            .text_size(px(13.))
                            .text_color(rgb(0x8192ad))
                            .child("Workspace activity · Everything is up to date"),
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
                    size(px(960.), px(900.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Animated collapse");
                cx.new(|_| Preview {
                    expanded: [true, false, true],
                    rows: 2,
                    narrow: false,
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
