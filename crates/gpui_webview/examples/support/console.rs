use gpui::{prelude::*, *};
use gpui_webview::{WebViewConsoleLevel, WebViewConsoleMessage};
use std::collections::VecDeque;

#[derive(Default)]
pub struct PageConsole {
    entries: VecDeque<(WebViewConsoleLevel, SharedString)>,
}

impl PageConsole {
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        cx.notify();
    }

    pub fn push(&mut self, message: &WebViewConsoleMessage, cx: &mut Context<Self>) {
        if self.entries.len() == 500 {
            self.entries.pop_front();
        }
        let mut text: String = message.message.chars().take(16_384).collect();
        if text.len() < message.message.len() {
            text.push_str("… [truncated]");
        }
        self.entries.push_back((message.level, text.into()));
        cx.notify();
    }
}

impl Render for PageConsole {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0c141e))
            .text_color(rgb(0xe2ebf4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .p_3()
                    .child(format!(
                        "Page console · {} / 500 messages",
                        self.entries.len()
                    ))
                    .child(
                        div()
                            .id("clear-console")
                            .cursor_pointer()
                            .child("Clear")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("console-messages")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .children(self.entries.iter().rev().map(|(level, text)| {
                        let color = match level {
                            WebViewConsoleLevel::Error => 0xff8b8b,
                            WebViewConsoleLevel::Warn => 0xf3cd75,
                            _ => 0xe2ebf4,
                        };
                        div()
                            .mb_3()
                            .text_color(rgb(color))
                            .child(format!("[{level:?}] {text}"))
                    })),
            )
    }
}
