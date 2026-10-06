use gpui::{prelude::*, *};

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| Counter {
                count: 0,
                scroll: ScrollHandle::new(),
                clipboard_status: "Copy the counter or paste text from another app.".into(),
            })
        })
        .expect("failed to open the GPUI window");
    });
}

struct Counter {
    count: usize,
    scroll: ScrollHandle,
    clipboard_status: String,
}

fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .p_4()
        .rounded_lg()
        .bg(rgb(0x375c91))
        .child(label)
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
                    .child(button("increment", "Tap to count").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.count += 1;
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa0b1c6))
                    .child("Rotate or switch apps. Your count stays here."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_5()
                    .rounded_xl()
                    .bg(rgb(0x1e2d40))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_xl().child("Clipboard & links"))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(button("copy", "Copy count").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let task =
                                        cx.write_to_clipboard_async(ClipboardItem::new_string(
                                            format!("GPUI: {} taps", this.count),
                                        ));
                                    cx.spawn(async move |this, cx| {
                                        let result = task.await;
                                        let _ = this.update(cx, |this, cx| {
                                            this.clipboard_status = result
                                                .map(|_| "Counter copied.".into())
                                                .unwrap_or_else(|error| error.to_string());
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                },
                            )))
                            .child(button("paste", "Paste text").on_click(cx.listener(
                                |_, _, _, cx| {
                                    let task = cx.read_from_clipboard_async();
                                    cx.spawn(async move |this, cx| {
                                        let result = task.await;
                                        let _ = this.update(cx, |this, cx| {
                                            this.clipboard_status = match result {
                                                Ok(Some(item)) => format!(
                                                    "Pasted: {}",
                                                    item.text().unwrap_or_default()
                                                ),
                                                Ok(None) => "No text available to paste.".into(),
                                                Err(error) => error.to_string(),
                                            };
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                },
                            ))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .whitespace_normal()
                            .text_color(rgb(0xa0b1c6))
                            .child(self.clipboard_status.clone()),
                    )
                    .child(button("open-link", "Open website").on_click(|_, _, cx| {
                        cx.open_url("https://www.rust-lang.org/");
                    })),
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
