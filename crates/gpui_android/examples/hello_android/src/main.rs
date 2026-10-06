use gpui::{prelude::*, *};
use uic::components::input::{Input, InputEvent, InputMode, TextInput};

const KEYBOARDS: [(&str, TextInputPurpose); 6] = [
    ("Email", TextInputPurpose::Email),
    ("URL", TextInputPurpose::Url),
    ("Phone", TextInputPurpose::Phone),
    (
        "Digits",
        TextInputPurpose::Number {
            decimal: false,
            signed: false,
        },
    ),
    (
        "Signed decimal",
        TextInputPurpose::Number {
            decimal: true,
            signed: true,
        },
    ),
    ("Text", TextInputPurpose::Text),
];

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        uic::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| Counter {
                details: false,
                count: 0,
                scroll: ScrollHandle::new(),
                clipboard_status: "Copy the counter or paste text from another app.".into(),
                title: cx.new(|cx| TextInput::new(cx).placeholder("Name")),
                text: cx.new(|cx| TextInput::new(cx).multiline().placeholder("Message")),
                password: cx.new(|cx| TextInput::new(cx).password().placeholder("Password")),
                password_visible: false,
                submissions: 0,
                keyboard: 0,
                keyboard_input: cx.new(|cx| {
                    TextInput::new(cx)
                        .input_purpose(KEYBOARDS[0].1)
                        .placeholder("Try a keyboard layout")
                }),
            });
            view.update(cx, |this, cx| {
                cx.subscribe(&this.title, |this, _, event, cx| {
                    if matches!(event, InputEvent::Submit(_)) {
                        this.submissions += 1;
                        cx.notify();
                    }
                })
                .detach();
            });
            window.on_system_back(
                cx,
                window.handler_for(&view, |this, window, cx| {
                    this.details = false;
                    window.set_back_enabled(false);
                    cx.notify();
                }),
            );
            view
        })
        .expect("failed to open the GPUI window");
    });
}

struct Counter {
    details: bool,
    count: usize,
    scroll: ScrollHandle,
    clipboard_status: String,
    text: Entity<TextInput>,
    title: Entity<TextInput>,
    password: Entity<TextInput>,
    password_visible: bool,
    submissions: usize,
    keyboard: usize,
    keyboard_input: Entity<TextInput>,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_back_enabled(self.details);
        if self.details {
            return div()
                .id("details-page")
                .size_full()
                .overflow_y_scroll()
                .bg(rgb(0x101923))
                .text_color(rgb(0xe7edf7))
                .font_family("IBM Plex Sans")
                .p_6()
                .flex()
                .flex_col()
                .gap_5()
                .child(div().text_3xl().child("Details"))
                .child("System Back returns to the main page. With the keyboard open, Back hides it first.")
                .child(Input::new(&self.title).text_color(rgb(0x172033)))
                .child(div().text_sm().child(format!("Keyboard: {}", KEYBOARDS[self.keyboard].0)))
                .child(Input::new(&self.keyboard_input).text_color(rgb(0x172033)))
                .child(button("keyboard-purpose", "Change keyboard").on_click(cx.listener(|this, _, window, cx| {
                    this.keyboard = (this.keyboard + 1) % KEYBOARDS.len();
                    this.keyboard_input.update(cx, |input, cx| {
                        input.set_input_purpose(KEYBOARDS[this.keyboard].1, cx);
                    });
                    window.focus(&this.keyboard_input.focus_handle(cx), cx);
                    window.show_soft_keyboard();
                    cx.notify();
                })))
                .child(div().flex().flex_wrap().gap_3()
                    .child(button("edit-name", "Edit name").on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.title.focus_handle(cx), cx);
                        window.show_soft_keyboard();
                    })))
                    .child(button("hide-keyboard", "Hide keyboard").on_click(|_, window, _| {
                        window.hide_soft_keyboard();
                    })))
                .child(button("back", "Back to main page").on_click(cx.listener(|this, _, window, cx| {
                    this.details = false;
                    window.set_back_enabled(false);
                    cx.notify();
                })))
                .into_any_element();
        }
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
                button("details", "Open details").on_click(cx.listener(|this, _, _, cx| {
                    this.details = true;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .text_color(rgb(0xa0b1c6))
                    .child("A Rust interface inside an Android View."),
            )
            .child(Input::new(&self.title).text_color(rgb(0x172033)))
            .child(
                div()
                    .text_sm()
                    .child(format!("Name submissions: {}", self.submissions)),
            )
            .child(Input::new(&self.text).rows(2).text_color(rgb(0x172033)))
            .child(Input::new(&self.password).text_color(rgb(0x172033)))
            .child(
                button("password-mode", "Show / hide password").on_click(cx.listener(
                    |this, _, window, cx| {
                        this.password_visible = !this.password_visible;
                        this.password.update(cx, |input, cx| {
                            input.set_mode(if this.password_visible {
                                InputMode::Text
                            } else {
                                InputMode::Password
                            });
                            cx.notify();
                        });
                        window.focus(&this.password.focus_handle(cx), cx);
                        window.show_soft_keyboard();
                    },
                )),
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
            .into_any_element()
    }
}
