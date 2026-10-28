use clap::Parser;
use gpui::{prelude::*, *};
use gpui_webview::{Url, WebView, WebViewBackend, WebViewDialogKind, WebViewEvent, WebViewOptions};
use uic::components::input::{Input, InputEvent, TextInput};

#[path = "support/console.rs"]
mod console;
use console::PageConsole;

#[derive(Parser)]
struct Arguments {
    /// Initial page URL. Opens the bundled page when omitted.
    url: Option<Url>,
}

struct BrowserWindow {
    view: Entity<WebView>,
    address: Entity<TextInput>,
    current_url: Url,
    address_error: Option<String>,
    home: Url,
    console: Entity<PageConsole>,
    console_window: Option<WindowHandle<PageConsole>>,
    _events: Subscription,
    _address_events: Subscription,
}
impl BrowserWindow {
    fn new(url: Option<Url>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home = url.unwrap_or_else(|| {
            Url::from_file_path(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/browser.html"
            ))
            .unwrap()
        });
        let view = cx.new(|cx| {
            WebView::new(
                WebViewOptions::new(WebViewBackend::Servo, home.clone()),
                window,
                cx,
            )
        });
        Self::with_view(home, view, window, cx)
    }

    fn with_view(
        home: Url,
        view: Entity<WebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let console = cx.new(|_| PageConsole::default());
        let address = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("Enter a URL")
                .initial_value(home.as_str().to_owned())
        });
        let address_events = cx.subscribe_in(&address, window, |this, _, event, window, cx| {
            if let InputEvent::Submit(_) = event {
                this.navigate_address(window, cx);
            }
        });
        let events = cx.subscribe_in(&view, window, |this, _, event, window, cx| {
            match event {
                WebViewEvent::NewWindowRequested(request) => {
                    let request = request.clone();
                    let options = WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1000.), px(760.)),
                            cx,
                        ))),
                        ..Default::default()
                    };
                    if let Err(error) = cx.open_window(options, move |window, cx| {
                        let view = cx.new(|cx| WebView::from_window_request(request, window, cx));
                        cx.new(|cx| {
                            Self::with_view(Url::parse("about:blank").unwrap(), view, window, cx)
                        })
                    }) {
                        this.address_error = Some(error.to_string());
                    }
                }
                WebViewEvent::Closed => window.remove_window(),
                WebViewEvent::TitleChanged(Some(title)) => window.set_window_title(title),
                WebViewEvent::LoadStarted => {
                    this.address_error = None;
                    this.console.update(cx, |console, cx| console.clear(cx));
                }
                WebViewEvent::UrlChanged(url) if *url != this.current_url => {
                    this.current_url = url.clone();
                    this.address.update(cx, |address, cx| {
                        address.set_value(url.as_str().to_owned(), cx);
                    });
                }
                WebViewEvent::ConsoleMessage(message) => {
                    eprintln!("[page {:?}] {}", message.level, message.message);
                    this.console
                        .update(cx, |console, cx| console.push(message, cx));
                    return;
                }
                WebViewEvent::Failed(error) => {
                    this.address_error = Some(error.clone());
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
            address,
            current_url: home.clone(),
            address_error: None,
            home,
            console,
            console_window: None,
            _events: events,
            _address_events: address_events,
        }
    }

    fn navigate_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.address.read(cx).value();
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let address = if value.contains("://")
            || value.starts_with("about:")
            || value.starts_with("data:")
            || value.starts_with("file:")
        {
            value.to_owned()
        } else {
            format!("https://{value}")
        };
        let Ok(url) = Url::parse(&address) else {
            self.address_error = Some("Enter a valid URL".into());
            cx.notify();
            return;
        };
        self.address_error = None;
        self.address.update(cx, |address, cx| {
            address.set_value(url.as_str().to_owned(), cx)
        });
        self.view.update(cx, |view, cx| view.navigate(url, cx));
        window.blur();
        cx.notify();
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
                    .h(px(56.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .gap_3()
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
                    .child(
                        div()
                            .id("console")
                            .cursor_pointer()
                            .child("Console")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(handle) = this.console_window {
                                    if handle
                                        .update(cx, |_, window, _| window.activate_window())
                                        .is_ok()
                                    {
                                        return;
                                    }
                                }
                                let console = this.console.clone();
                                this.console_window = cx
                                    .open_window(
                                        WindowOptions {
                                            titlebar: Some(TitlebarOptions {
                                                title: Some("Page console".into()),
                                                ..Default::default()
                                            }),
                                            ..Default::default()
                                        },
                                        |_, _| console,
                                    )
                                    .ok();
                            })),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.address)
                                .h(px(36.))
                                .bg(rgb(0x182533))
                                .text_color(rgb(0xe2ebf4))
                                .border_color(rgb(0x405366)),
                        ),
                    )
                    .child(div().id("go").cursor_pointer().child("Go").on_click(
                        cx.listener(|this, _, window, cx| this.navigate_address(window, cx)),
                    )),
            )
            .when_some(self.address_error.clone(), |root, error| {
                root.child(
                    div()
                        .px_4()
                        .pb_2()
                        .text_sm()
                        .text_color(rgb(0xffa0a0))
                        .child(error),
                )
            })
            .child(div().flex_1().min_h_0().child(self.view.clone()))
    }
}
fn main() {
    let arguments = Arguments::parse();
    gpui_platform::application().run(move |cx| {
        uic::init(cx);
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
            move |window, cx| cx.new(|cx| BrowserWindow::new(arguments.url, window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
