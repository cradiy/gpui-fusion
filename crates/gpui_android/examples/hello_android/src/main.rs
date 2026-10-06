use gpui::{prelude::*, *};

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| Counter {
                count: 0,
                scroll: ScrollHandle::new(),
            })
        })
        .expect("failed to open the GPUI window");
    });
}

struct Counter {
    count: usize,
    scroll: ScrollHandle,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(rgb(0x101923))
            .text_color(rgb(0xe7edf7))
            .font_family("IBM Plex Sans")
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_3xl().child("GPUI on Android"))
            .child(
                div()
                    .text_color(rgb(0xa0b1c6))
                    .child("A Rust interface inside an Android View."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(0x1e2d40))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_2xl().child(format!("{} taps", self.count)))
                    .child(
                        div()
                            .id("increment")
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(0x375c91))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.count += 1;
                                cx.notify();
                            }))
                            .child("Tap to count"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa0b1c6))
                    .child("Rotate or switch apps. Your count stays here."),
            )
            .child(div().text_xl().child("Swipe to explore"))
            .children((1usize..=20).map(|index| {
                div()
                    .id(("row", index))
                    .flex_shrink_0()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(0x1e2d40))
                    .child(format!("Item {index:02}"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
            }))
    }
}
