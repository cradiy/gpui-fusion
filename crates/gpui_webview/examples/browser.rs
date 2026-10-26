use gpui::{prelude::*, *};
use gpui_webview::{Url, WebView, WebViewBackend, WebViewEvent, WebViewOptions};

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
