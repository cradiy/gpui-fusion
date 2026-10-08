use super::*;
use futures::{StreamExt, future::Either, lock::Mutex};
use std::{collections::HashMap, sync::Arc};
use zbus::{Connection, Proxy, zvariant::Value};

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

struct LinuxNotifications {
    connection: Connection,
    options: NotificationOptions,
    capabilities: NotificationCapabilities,
    ids: Arc<Mutex<HashMap<String, u32>>>,
    stop: async_channel::Sender<()>,
}

pub async fn create(options: NotificationOptions) -> Result<NotificationCenter> {
    let connection = Connection::session().await?;
    let proxy = Proxy::new(&connection, SERVICE, PATH, SERVICE).await?;
    let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await?;
    let actions = proxy.receive_signal("ActionInvoked").await?;
    let closed = proxy.receive_signal("NotificationClosed").await?;
    let (events, receiver) = async_channel::unbounded();
    let (stop, stopped) = async_channel::bounded(1);
    let ids = Arc::new(Mutex::new(HashMap::<String, u32>::new()));
    let signal_ids = ids.clone();
    std::thread::Builder::new()
        .name("gpui-notifications".into())
        .spawn(move || {
            async_io::block_on(async move {
                let mut signals =
                    futures::stream::select(actions.map(|m| (true, m)), closed.map(|m| (false, m)));
                loop {
                    let next = signals.next();
                    futures::pin_mut!(next);
                    match futures::future::select(next, Box::pin(stopped.recv())).await {
                        Either::Left((Some((action, message)), _)) => {
                            let (native, response) = if action {
                                let Ok((id, action)) =
                                    message.body().deserialize::<(u32, String)>()
                                else {
                                    continue;
                                };
                                (id, Some(action))
                            } else {
                                let Ok((id, _reason)) = message.body().deserialize::<(u32, u32)>()
                                else {
                                    continue;
                                };
                                (id, None)
                            };
                            let mut ids = signal_ids.lock().await;
                            if let Some(id) = ids
                                .iter()
                                .find_map(|(key, value)| (*value == native).then(|| key.clone()))
                            {
                                let event = match response {
                                    Some(action) => NotificationEvent::Activated {
                                        id,
                                        action: (action != "default").then_some(action),
                                        reply: None,
                                    },
                                    None => {
                                        ids.remove(&id);
                                        NotificationEvent::Dismissed { id }
                                    }
                                };
                                let _ = events.try_send(event);
                            }
                        }
                        _ => break,
                    }
                }
            });
        })?;
    Ok(NotificationCenter::from_backend(
        Rc::new(LinuxNotifications {
            connection,
            options,
            capabilities: NotificationCapabilities {
                max_actions: if capabilities.iter().any(|c| c == "actions") {
                    8
                } else {
                    0
                },
                dismissal_events: true,
                progress: true,
                inline_reply: false,
                resource_icons: true,
                image_icons: true,
            },
            ids,
            stop,
        }),
        receiver,
    ))
}

impl NotificationBackend for LinuxNotifications {
    fn capabilities(&self) -> NotificationCapabilities {
        self.capabilities
    }
    fn permission(&self, _: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        Box::pin(async { Ok(NotificationPermission::Granted) })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let proxy = Proxy::new(&self.connection, SERVICE, PATH, SERVICE).await?;
            let mut ids = self.ids.lock().await;
            let replaces = ids.get(&notification.id).copied().unwrap_or(0);
            let mut actions = Vec::new();
            if self.capabilities.max_actions > 0 {
                actions.extend(["default".to_owned(), "Open".to_owned()]);
                for action in notification.actions {
                    actions.extend([action.id, action.label]);
                }
            }
            let mut hints = HashMap::new();
            hints.insert("desktop-entry", Value::from(self.options.app_id.as_str()));
            hints.insert("suppress-sound", Value::from(notification.silent));
            if let Some(progress) = notification.progress {
                hints.insert("value", Value::from(i32::from(progress)));
            }
            let body = notification
                .body
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let icon = match notification.icon {
                Some(NotificationIcon::Resource(value) | NotificationIcon::ImageUri(value)) => {
                    value
                }
                None => self.options.app_id.clone(),
            };
            let native: u32 = proxy
                .call(
                    "Notify",
                    &(
                        self.options.app_name.as_str(),
                        replaces,
                        icon,
                        notification.title.as_str(),
                        body,
                        actions,
                        hints,
                        -1i32,
                    ),
                )
                .await?;
            ids.insert(notification.id, native);
            Ok(())
        })
    }
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let proxy = Proxy::new(&self.connection, SERVICE, PATH, SERVICE).await?;
            let mut ids = self.ids.lock().await;
            if let Some(native) = ids.get(&id).copied() {
                proxy
                    .call::<_, _, ()>("CloseNotification", &(native,))
                    .await?;
                ids.remove(&id);
            }
            Ok(())
        })
    }
}

impl Drop for LinuxNotifications {
    fn drop(&mut self) {
        self.stop.close();
    }
}
