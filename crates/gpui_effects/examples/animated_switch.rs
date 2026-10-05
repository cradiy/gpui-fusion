use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::{TransitionKind, animated_switch};
use gpui_platform::application;

struct Preview {
    page: usize,
    status: usize,
    blur: bool,
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
        let duration = Duration::from_millis(if self.slow { 1200 } else { 320 });
        let kind = if self.blur {
            TransitionKind::BlurFade
        } else {
            TransitionKind::CrossFade
        };
        let mut choices = div().flex().flex_wrap().gap(px(8.));
        for (index, (id, label)) in [
            ("design", "Design"),
            ("prototype", "Prototype"),
            ("inspect", "Inspect"),
        ]
        .into_iter()
        .enumerate()
        {
            choices = choices.child(
                button(id, label)
                    .when(index == self.page, |item| item.bg(rgb(0x344762)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = index;
                        cx.notify();
                    })),
            );
        }
        div().id("preview").size_full().overflow_y_scroll().p(px(32.))
            .bg(rgb(0x0c1018)).text_color(rgb(0xecf1fc)).flex().flex_col().gap(px(22.))
            .child(div().text_size(px(12.)).text_color(rgb(0x929fbc)).child("ANIMATED SWITCH"))
            .child(div().text_size(px(38.)).child("Make room for what's next."))
            .child(div().text_size(px(14.)).line_height(px(22.)).text_color(rgb(0x8e9ab1))
                .child("Change a view or a status. Give the moment a softer landing."))
            .child(div().flex().flex_wrap().gap(px(10.))
                .child(button("kind", if self.blur { "Effect: Blur fade" } else { "Effect: Crossfade" })
                    .on_click(cx.listener(|this, _, _, cx| { this.blur = !this.blur; cx.notify(); })))
                .child(button("pace", if self.slow { "Pace: Slow" } else { "Pace: Natural" })
                    .on_click(cx.listener(|this, _, _, cx| { this.slow = !this.slow; cx.notify(); })))
                .child(button("motion", if self.motion { "Motion: On" } else { "Motion: Off" })
                    .on_click(cx.listener(|this, _, _, cx| { this.motion = !this.motion; cx.notify(); }))))
            .child(div().w_full().max_w(px(760.)).flex_shrink_0().flex().flex_col().gap(px(22.))
                .child(div().w_full().p(px(22.)).rounded(px(24.)).bg(rgb(0x182230))
                    .border_1().border_color(rgb(0x2b3a4e)).flex().flex_col().gap(px(18.))
                    .child(div().text_size(px(12.)).text_color(rgb(0x8192ad)).child("WORKSPACE"))
                    .child(choices)
                    .child(animated_switch("page", self.page, |page| {
                        let (eyebrow, title, detail, color, background) = [
                            ("01 / DESIGN", "Start with a little possibility.", "Shape an idea, find its color, and make it feel like yours.", 0xa2b8ff, 0x26344b),
                            ("02 / PROTOTYPE", "Give your idea a little motion.", "Connect the moments and explore how everything fits together.", 0x77dfc2, 0x203c39),
                            ("03 / INSPECT", "Good work lives in the details.", "Look a little closer. Keep the intent clear from start to finish.", 0xffc38c, 0x40332c),
                        ][*page];
                        div().size_full().p(px(24.)).rounded(px(18.)).bg(rgb(background))
                            .flex().flex_col().gap(px(16.))
                            .child(div().text_size(px(11.)).text_color(rgb(color)).child(eyebrow))
                            .child(div().text_size(px(28.)).line_height(px(36.)).child(title))
                            .child(div().text_size(px(14.)).line_height(px(22.)).text_color(rgb(0xb5c3d7)).child(detail))
                            .child(div().flex_1())
                            .child(div().w(px(54.)).h(px(4.)).rounded_full().bg(rgb(color)))
                    }).w_full().h(px(280.)).duration(duration).kind(kind).enabled(self.motion)))
                .child(div().w_full().p(px(22.)).rounded(px(24.)).bg(rgb(0x182230))
                    .border_1().border_color(rgb(0x2b3a4e)).flex().flex_col().gap(px(18.))
                    .child(div().text_size(px(12.)).text_color(rgb(0x8192ad)).child("SMALL MOMENTS"))
                    .child(animated_switch("status", self.status, |status| {
                        let (symbol, label, detail, color) = [
                            ("○", "Ready when you are", "Your next idea has a place here.", 0xa2b8ff),
                            ("↗", "Saving your changes", "Keeping the good things close.", 0xffc38c),
                            ("✓", "Everything is saved", "A little peace of mind.", 0x77dfc2),
                        ][*status];
                        div().size_full().flex().items_center().gap(px(16.))
                            .child(div().size(px(42.)).flex_shrink_0().rounded(px(13.)).bg(rgb(0x29374a))
                                .flex().items_center().justify_center().text_size(px(24.)).text_color(rgb(color)).child(symbol))
                            .child(div().min_w_0().flex_1().flex().flex_col().gap(px(5.))
                                .child(div().text_size(px(18.)).text_color(rgb(color)).child(label))
                                .child(div().text_size(px(13.)).line_height(px(20.)).text_color(rgb(0x93a4be)).child(detail)))
                    }).w_full().h(px(86.)).duration(duration).kind(kind).enabled(self.motion))
                    .child(div().flex().flex_wrap().gap(px(10.))
                        .child(button("status-next", "Next status").on_click(cx.listener(|this, _, _, cx| {
                            this.status = (this.status + 1) % 3; cx.notify();
                        })))
                        .child(button("status-reset", "Back to ready").on_click(cx.listener(|this, _, _, cx| {
                            this.status = 0; cx.notify();
                        }))))))
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
                window.set_window_title("Animated switch");
                cx.new(|_| Preview {
                    page: 0,
                    status: 0,
                    blur: false,
                    slow: false,
                    motion: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
