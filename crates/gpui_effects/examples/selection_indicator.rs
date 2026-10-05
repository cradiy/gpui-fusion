use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, rgba, size,
};
use gpui_effects::selection_indicator;
use gpui_platform::application;

struct Preview {
    selected: [usize; 2],
    long_label: bool,
    reverse: bool,
    narrow: bool,
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
        let duration = Duration::from_millis(if self.slow { 900 } else { 260 });
        let mut segments = selection_indicator(
            "files",
            div().size_full().rounded(px(14.)).bg(rgb(0x344762)),
        )
        .selected(("file", self.selected[0]))
        .duration(duration)
        .enabled(self.motion)
        .flex()
        .flex_wrap()
        .gap(px(4.))
        .p(px(6.))
        .rounded(px(20.))
        .bg(rgb(0x121b28))
        .border_1()
        .border_color(rgb(0x28364a));
        let order = if self.reverse { [2, 1, 0] } else { [0, 1, 2] };
        for index in order {
            let selected = index == self.selected[0];
            let title = [
                "All files",
                if self.long_label {
                    "Recently opened"
                } else {
                    "Recent"
                },
                "Shared with me",
            ][index];
            segments = segments.item(
                ("file", index),
                div()
                    .id("option")
                    .px(px(18.))
                    .py(px(12.))
                    .flex_shrink_0()
                    .rounded(px(14.))
                    .text_size(px(15.))
                    .text_color(rgb(if selected { 0xf2f5ff } else { 0x94a5bf }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xffffff0a)))
                    .child(title)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected[0] = index;
                        cx.notify();
                    })),
            );
        }
        let mut tabs = selection_indicator(
            "workspace",
            div().size_full().rounded(px(2.)).bg(rgb(0x77dfc2)),
        )
        .selected(("tab", self.selected[1]))
        .underline(px(3.))
        .duration(duration)
        .enabled(self.motion)
        .flex()
        .flex_wrap()
        .gap(px(14.));
        for (index, label) in ["Overview", "Activity", "Versions", "Details"]
            .into_iter()
            .enumerate()
        {
            let selected = index == self.selected[1];
            tabs = tabs.item(
                ("tab", index),
                div()
                    .id("option")
                    .px(px(8.))
                    .pt(px(12.))
                    .pb(px(16.))
                    .flex_shrink_0()
                    .text_size(px(15.))
                    .text_color(rgb(if selected { 0x92f2d7 } else { 0x94a5bf }))
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(0xf2f5ff)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected[1] = index;
                        cx.notify();
                    })),
            );
        }
        let (title, description) = [
            (
                "A place for your next idea.",
                "Bring your sketches, notes and small discoveries together.",
            ),
            (
                "A little progress, every day.",
                "See the recent changes that keep your workspace moving.",
            ),
            (
                "Every version tells a story.",
                "Look back at the details that brought you here.",
            ),
            (
                "The details make it yours.",
                "Keep the context close, and the next step clear.",
            ),
        ][self.selected[1]];

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
                    .child("SELECTION INDICATOR"),
            )
            .child(div().text_size(px(38.)).child("Follow your focus."))
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Choose a view. The highlight finds its place."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "label",
                            if self.long_label {
                                "Label: Long"
                            } else {
                                "Label: Short"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.long_label = !this.long_label;
                            cx.notify();
                        })),
                    )
                    .child(button("order", "Reverse order").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.reverse = !this.reverse;
                            cx.notify();
                        },
                    )))
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
                    .max_w(px(if self.narrow { 400. } else { 760. }))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(28.))
                    .child(
                        div()
                            .w_full()
                            .p(px(22.))
                            .rounded(px(24.))
                            .bg(rgb(0x182230))
                            .border_1()
                            .border_color(rgb(0x2b3a4e))
                            .flex()
                            .flex_col()
                            .gap(px(18.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(0x8192ad))
                                    .child("YOUR COLLECTION"),
                            )
                            .child(segments)
                            .child(
                                div()
                                    .pt(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(div().size(px(8.)).rounded_full().bg(rgb(0xa2b8ff)))
                                    .child(
                                        div().text_size(px(14.)).text_color(rgb(0xb7c7df)).child(
                                            [
                                                "Everything, together in one place.",
                                                "Pick up where you left off.",
                                                "Good ideas are better together.",
                                            ][self.selected[0]],
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .p(px(22.))
                            .rounded(px(24.))
                            .bg(rgb(0x182230))
                            .border_1()
                            .border_color(rgb(0x2b3a4e))
                            .flex()
                            .flex_col()
                            .gap(px(22.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(0x8192ad))
                                    .child("WORKSPACE"),
                            )
                            .child(tabs)
                            .child(
                                div()
                                    .py(px(14.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(14.))
                                    .child(
                                        div().text_size(px(26.)).line_height(px(34.)).child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .line_height(px(22.))
                                            .text_color(rgb(0x93a4be))
                                            .child(description),
                                    )
                                    .child(
                                        div()
                                            .mt(px(12.))
                                            .h(px(4.))
                                            .w(px(64.))
                                            .rounded_full()
                                            .bg(rgb(0x77dfc2)),
                                    ),
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
                    size(px(960.), px(900.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Selection indicator");
                cx.new(|_| Preview {
                    selected: [0, 0],
                    long_label: false,
                    reverse: false,
                    narrow: false,
                    slow: false,
                    motion: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
