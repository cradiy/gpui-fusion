use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::{StaggerOrder, staggered_presence};
use gpui_platform::application;

struct Preview {
    visible: bool,
    slow: bool,
    reverse: bool,
    motion: bool,
    clicks: [usize; 4],
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
        let mut items = staggered_presence("collection", self.visible)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .animate_initial(true)
            .duration(Duration::from_millis(if self.slow { 700 } else { 280 }))
            .interval(Duration::from_millis(if self.slow { 180 } else { 65 }))
            .exit_order(if self.reverse {
                StaggerOrder::Reverse
            } else {
                StaggerOrder::Forward
            })
            .enabled(self.motion);
        for (index, (title, detail, color)) in [
            (
                "A fresh perspective",
                "Collect the ideas worth coming back to.",
                0xa2b8ff,
            ),
            (
                "Find your rhythm",
                "Small steps, a little room to breathe.",
                0x77dfc2,
            ),
            (
                "Make it your own",
                "Bring the details into focus.",
                0xffc38c,
            ),
            (
                "Ready for the next chapter",
                "Keep moving at your own pace.",
                0xd5a8eb,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let clicks = self.clicks[index];
            let on_click = cx.listener(move |this, _, _, cx| {
                this.clicks[index] += 1;
                cx.notify();
            });
            items = items.item(("card", index), move |frame| {
                div()
                    .w_full()
                    .relative()
                    .top(px(16. * (1. - frame.progress)))
                    .opacity(frame.progress)
                    .rounded(px(20.))
                    .bg(rgb(0x182230))
                    .border_1()
                    .border_color(rgb(0x2b3a4e))
                    .child(
                        div()
                            .id("open")
                            .m(px(6.))
                            .p(px(16.))
                            .rounded(px(14.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x202e40)))
                            .flex()
                            .items_center()
                            .gap(px(18.))
                            .child(
                                div()
                                    .size(px(38.))
                                    .flex_shrink_0()
                                    .rounded(px(12.))
                                    .bg(rgb(0x26344a))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .text_color(rgb(color))
                                    .child(format!("{:02}", index + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(div().text_size(px(19.)).child(title))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .line_height(px(20.))
                                            .text_color(rgb(0x93a4be))
                                            .child(detail),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(13.))
                                    .text_color(rgb(color))
                                    .child(if clicks == 0 {
                                        "→".to_owned()
                                    } else {
                                        format!("{clicks}  →")
                                    }),
                            )
                            .on_click(on_click),
                    )
            });
        }
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
                    .child("STAGGERED PRESENCE"),
            )
            .child(div().text_size(px(38.)).child("One thing at a time."))
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child("A collection that arrives in rhythm. Change your mind at any moment."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "visibility",
                            if self.visible {
                                "Hide collection"
                            } else {
                                "Show collection"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.visible = !this.visible;
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
                            "order",
                            if self.reverse {
                                "Exit: Reverse"
                            } else {
                                "Exit: Forward"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reverse = !this.reverse;
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
            .child(div().w_full().max_w(px(760.)).flex_shrink_0().child(items))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(0x8192ad))
                    .child("Your workspace stays here. Show the collection whenever you need it."),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(850.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Staggered presence");
                cx.new(|_| Preview {
                    visible: true,
                    slow: false,
                    reverse: true,
                    motion: true,
                    clicks: [0; 4],
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
