use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use uic::components::text_view::TextView;

struct Preview;

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_8().bg(rgb(0xf5f7fb)).text_color(rgb(0x243247))
            .child(div().w_full().max_w(px(620.)).mx_auto().p_6().rounded_xl().bg(rgb(0xffffff))
                .child(div().text_2xl().mb_4().child("Reading notes"))
                .child(TextView::new("notes", "The observatory opens before sunrise. Visitors can follow the marked trail to the eastern terrace.\n\n观测记录：云层逐渐散去，山谷仍笼罩在薄雾中。🌄\n\nSelect a passage to copy it into another application.")
                    .text_size(px(19.)).line_height(px(32.)))
                .child(div().mt_6().text_sm().text_color(rgb(0x69778a)).child("Drag or double-click to select. On touch screens, long-press a word.")))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.), px(520.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Preview),
        )
        .unwrap();
        cx.activate(true);
    });
}
