use gpui::{gpui_notifications::*, prelude::*, *};

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|cx| {
                let mut demo = Demo {
                    center: None,
                    status: "Connect to system notifications.".into(),
                    count: 0,
                    connecting: false,
                };
                demo.connect(cx);
                demo
            })
        })
        .unwrap();
    });
}
struct Demo {
    center: Option<NotificationCenter>,
    status: String,
    count: u32,
    connecting: bool,
}
impl Demo {
    fn connect(&mut self, cx: &mut Context<Self>) {
        if self.center.is_some() || self.connecting {
            return;
        }
        self.connecting = true;
        let options = NotificationOptions::new("dev.gpui.notifications", "GPUI Notifications");
        #[cfg(target_os = "windows")]
        let options = NotificationOptions {
            windows_register_application: true,
            ..options
        };
        let request = cx.notifications(options);
        cx.spawn(async move |this, cx| match request.await {
            Ok(center) => {
                let events = center.take_events().unwrap();
                if this
                    .update(cx, |this, cx| {
                        this.connecting = false;
                        this.status = format!("Connected: {:?}", center.capabilities());
                        this.center = Some(center);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                while let Ok(event) = events.recv().await {
                    if this
                        .update(cx, |this, cx| {
                            this.status = format!("{event:?}");
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    this.connecting = false;
                    this.status = error.to_string();
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn action(&mut self, action: u8, cx: &mut Context<Self>) {
        let Some(center) = self.center.clone() else {
            self.connect(cx);
            return;
        };
        self.count += 1;
        let count = self.count;
        let permission = (action == 0).then(|| center.request_permission());
        cx.spawn(async move |this, cx| {
            let result = async {
                match action {
                    0 => Ok(format!("Permission: {:?}", permission.unwrap().await?)),
                    3 => {
                        center.remove("demo").await?;
                        Ok("Notification removed".into())
                    }
                    _ => {
                        let mut notification = Notification::new(
                            "demo",
                            "GPUI Notifications",
                            format!("Message {count} · Hello / 你好"),
                        );
                        let capabilities = center.capabilities();
                        if capabilities.max_actions > 0 {
                            notification
                                .actions
                                .push(NotificationAction::new("open", "Open"));
                        }
                        if capabilities.inline_reply {
                            notification.actions.push(
                                NotificationAction::new("reply", "Reply").reply("Your message"),
                            );
                        }
                        if action == 2 && capabilities.progress {
                            notification.progress = Some(((count * 10) % 101) as u8);
                            notification.silent = true;
                        }
                        center.show(notification).await?;
                        Ok("Notification sent; inspect the system notification surface.".into())
                    }
                }
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.status = result.unwrap_or_else(|error: anyhow::Error| error.to_string());
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x101821))
            .text_color(rgb(0xe7edf5))
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("System notifications"))
            .child(
                div()
                    .id("connect")
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(0x27394b))
                    .child("Connect")
                    .on_click(cx.listener(|this, _, _, cx| this.connect(cx))),
            )
            .children(
                [
                    "Request permission",
                    "Send notification",
                    "Update progress",
                    "Remove",
                ]
                .into_iter()
                .enumerate()
                .map(|(action, label)| {
                    div()
                        .id(("action", action))
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x27394b))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.action(action as u8, cx)))
                }),
            )
            .child(div().whitespace_normal().child(self.status.clone()))
    }
}
