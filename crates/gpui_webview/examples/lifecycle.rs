use clap::Parser;
use gpui::{prelude::*, *};
use gpui_webview::{Url, WebView, WebViewBackend, WebViewOptions};

#[derive(Parser)]
struct Arguments {
    #[arg(default_value = "about:blank")]
    url: Url,
}

struct LifecycleWindow {
    url: Url,
    views: Vec<Entity<WebView>>,
}

impl Render for LifecycleWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x15202b))
            .text_color(rgb(0xe2ebf4))
            .child(
                div()
                    .h(px(56.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(div().id("add").cursor_pointer().child("Add view").on_click(
                        cx.listener(|this, _, window, cx| {
                            let options =
                                WebViewOptions::new(WebViewBackend::Servo, this.url.clone());
                            this.views
                                .push(cx.new(|cx| WebView::new(options, window, cx)));
                            cx.notify();
                        }),
                    ))
                    .child(
                        div()
                            .id("remove")
                            .cursor_pointer()
                            .child("Remove view")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(view) = this.views.pop() {
                                    view.update(cx, |view, cx| view.close(cx));
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("clear")
                            .cursor_pointer()
                            .child("Close all")
                            .on_click(cx.listener(|this, _, _, cx| {
                                for view in this.views.drain(..) {
                                    view.update(cx, |view, cx| view.close(cx));
                                }
                                cx.notify();
                            })),
                    )
                    .child(format!(
                        "{} views · PID {}",
                        self.views.len(),
                        std::process::id()
                    )),
            )
            .child(
                div().flex_1().min_h_0().flex().children(
                    self.views
                        .iter()
                        .map(|view| div().flex_1().min_w_0().h_full().child(view.clone())),
                ),
            )
    }
}

fn main() {
    let arguments = Arguments::parse();
    gpui_platform::application().run(move |cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.), px(760.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WebView lifecycle".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| LifecycleWindow {
                    url: arguments.url,
                    views: Vec::new(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
