use gpui::{prelude::*, *};
use gpui_webview::{Url, WebView, WebViewBackend, WebViewDialogKind, WebViewEvent, WebViewOptions};

struct BrowserWindow {
    view: Entity<WebView>,
    title: SharedString,
    home: Url,
    _events: Subscription,
}
impl BrowserWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home = Url::from_file_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/browser.html"
        ))
        .unwrap();
        let view = cx.new(|cx| {
            WebView::new(
                WebViewOptions::new(WebViewBackend::Servo, home.clone()),
                window,
                cx,
            )
        });
        let events = cx.subscribe(&view, |this, _, event, cx| {
            match event {
                WebViewEvent::TitleChanged(Some(title)) => this.title = title.clone().into(),
                WebViewEvent::Failed(error) => {
                    this.title = error.clone().into();
                    eprintln!("{error}");
                }
                WebViewEvent::DialogRequested(dialog) => {
                    if !dialog.is_pending() {
                        return;
                    }
                    if matches!(dialog.kind, WebViewDialogKind::Prompt { .. }) {
                        dialog.cancel();
                        return;
                    }
                    let dialog = dialog.clone();
                    let bounds = Bounds::centered(None, size(px(480.), px(240.)), cx);
                    let _ = cx.open_window(
                        WindowOptions {
                            titlebar: Some(TitlebarOptions {
                                title: Some("Page dialog".into()),
                                ..Default::default()
                            }),
                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                            ..Default::default()
                        },
                        |_, cx| cx.new(|_| PageDialog(dialog)),
                    );
                }
                _ => {}
            }
            cx.notify();
        });
        Self {
            view,
            title: "Browser".into(),
            home,
            _events: events,
        }
    }
}
struct PageDialog(gpui_webview::WebViewDialog);
impl Render for PageDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x131e29))
            .text_color(rgb(0xe2ebf4))
            .child("Message from web content")
            .child(self.0.message.clone())
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .id("accept")
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x386ca1))
                            .cursor_pointer()
                            .child("OK")
                            .on_click(cx.listener(|this, _, window, _| {
                                this.0.accept();
                                window.remove_window();
                            })),
                    )
                    .when(self.0.kind != WebViewDialogKind::Alert, |row| {
                        row.child(
                            div()
                                .id("cancel")
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x203244))
                                .cursor_pointer()
                                .child("Cancel")
                                .on_click(cx.listener(|this, _, window, _| {
                                    this.0.cancel();
                                    window.remove_window();
                                })),
                        )
                    }),
            )
    }
}
impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0c141e))
            .text_color(rgb(0xe2ebf4))
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .id("back")
                            .cursor_pointer()
                            .child("Back")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view.update(cx, |view, cx| view.go_back(cx))
                            })),
                    )
                    .child(
                        div()
                            .id("forward")
                            .cursor_pointer()
                            .child("Forward")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view.update(cx, |view, cx| view.go_forward(cx))
                            })),
                    )
                    .child(
                        div()
                            .id("reload")
                            .cursor_pointer()
                            .child("Reload")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view.update(cx, |view, cx| view.reload(cx))
                            })),
                    )
                    .child(
                        div()
                            .id("home")
                            .cursor_pointer()
                            .child("Home")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let home = this.home.clone();
                                this.view.update(cx, |view, cx| view.navigate(home, cx));
                            })),
                    )
                    .child(self.title.clone()),
            )
            .child(div().flex_1().min_h_0().child(self.view.clone()))
    }
}
fn main() {
    gpui_platform::application().run(|cx| {
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
                    title: Some("GPUI WebView".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| BrowserWindow::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
