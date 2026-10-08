use super::Notification;
use super::*;
#[path = "windows_activation.rs"]
mod activation;
use ::windows::{
    Data::Xml::Dom::XmlDocument, Foundation::TypedEventHandler, UI::Notifications::*, core::HSTRING,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Entry {
    toast: ToastNotification,
    dismissed: i64,
    failed: i64,
    finished: Arc<AtomicBool>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        let _ = self.toast.RemoveDismissed(self.dismissed);
        let _ = self.toast.RemoveFailed(self.failed);
    }
}
struct WindowsNotifications {
    _activation: activation::Registration,
    notifier: ToastNotifier,
    app_id: HSTRING,
    events: async_channel::Sender<NotificationEvent>,
    entries: RefCell<HashMap<String, Entry>>,
}

pub async fn create(options: NotificationOptions) -> Result<NotificationCenter> {
    let (events, receiver) = async_channel::unbounded();
    let activation = activation::Registration::new(&options, events.clone())?;
    let app_id = HSTRING::from(options.app_id);
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)?;
    Ok(NotificationCenter::from_backend(
        Rc::new(WindowsNotifications {
            _activation: activation,
            notifier,
            app_id,
            events,
            entries: RefCell::default(),
        }),
        receiver,
    ))
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn identity(id: &str) -> (HSTRING, HSTRING) {
    let value = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, id.as_bytes())
        .simple()
        .to_string();
    (HSTRING::from(&value[..16]), HSTRING::from(&value[16..]))
}

impl NotificationBackend for WindowsNotifications {
    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            max_actions: 5,
            inline_reply: true,
            dismissal_events: true,
            progress: true,
            resource_icons: false,
            image_icons: true,
        }
    }
    fn permission(&self, _: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        let setting = self.notifier.Setting();
        Box::pin(async move {
            Ok(if setting? == NotificationSetting::Enabled {
                NotificationPermission::Granted
            } else {
                NotificationPermission::Denied
            })
        })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            ensure!(
                self.permission(false).await? == NotificationPermission::Granted,
                "notifications are disabled in Windows settings"
            );
            let mut source = format!(
                "<toast launch=\"{}:default\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text>",
                notification.id,
                xml(&notification.title),
                xml(&notification.body)
            );
            if let Some(NotificationIcon::ImageUri(uri)) = &notification.icon {
                source.push_str(&format!(
                    "<image placement=\"appLogoOverride\" src=\"{}\"/>",
                    xml(uri)
                ));
            }
            if let Some(value) = notification.progress {
                source.push_str(&format!(
                    "<progress value=\"{}\" valueStringOverride=\"{}%\" status=\"Progress\"/>",
                    f64::from(value) / 100.,
                    value
                ));
            }
            source.push_str("</binding></visual>");
            if !notification.actions.is_empty() {
                source.push_str("<actions>");
                for action in &notification.actions {
                    if let Some(placeholder) = &action.reply_placeholder {
                        source.push_str(&format!(
                            "<input id=\"{}\" type=\"text\" placeHolderContent=\"{}\"/>",
                            action.id,
                            xml(placeholder)
                        ));
                    }
                }
                for action in &notification.actions {
                    let input = if action.reply_placeholder.is_some() {
                        format!(" hint-inputId=\"{}\"", action.id)
                    } else {
                        String::new()
                    };
                    source.push_str(&format!("<action content=\"{}\" arguments=\"{}:{}\" activationType=\"foreground\"{input}/>", xml(&action.label), notification.id, action.id));
                }
                source.push_str("</actions>");
            }
            if notification.silent {
                source.push_str("<audio silent=\"true\"/>");
            }
            source.push_str("</toast>");
            let document = XmlDocument::new()?;
            document.LoadXml(&HSTRING::from(source))?;
            let toast = ToastNotification::CreateToastNotification(&document)?;
            let (tag, group) = identity(&notification.id);
            toast.SetTag(&tag)?;
            toast.SetGroup(&group)?;
            let finished = Arc::new(AtomicBool::new(false));
            let mut entry = Entry {
                toast,
                dismissed: 0,
                failed: 0,
                finished: finished.clone(),
            };
            let events = self.events.clone();
            let id = notification.id.clone();
            let done = finished.clone();
            entry.dismissed = entry.toast.Dismissed(&TypedEventHandler::new(move |_, _| {
                done.store(true, Ordering::Relaxed);
                let _ = events.try_send(NotificationEvent::Dismissed { id: id.clone() });
                Ok(())
            }))?;
            let events = self.events.clone();
            let id = notification.id.clone();
            entry.failed = entry.toast.Failed(&TypedEventHandler::new(
                move |_, args: ::windows::core::Ref<'_, ToastFailedEventArgs>| {
                    finished.store(true, Ordering::Relaxed);
                    let message = args
                        .as_ref()
                        .and_then(|args| args.ErrorCode().ok())
                        .map(|code| format!("{code:?}"))
                        .unwrap_or_else(|| "Windows rejected the notification".into());
                    let _ = events.try_send(NotificationEvent::Failed {
                        id: id.clone(),
                        message,
                    });
                    Ok(())
                },
            ))?;
            self.notifier.Show(&entry.toast)?;
            let mut entries = self.entries.borrow_mut();
            entries.retain(|_, entry| !entry.finished.load(Ordering::Relaxed));
            entries.insert(notification.id, entry);
            Ok(())
        })
    }
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let (tag, group) = identity(&id);
            ToastNotificationManager::History()?.RemoveGroupedTagWithId(
                &tag,
                &group,
                &self.app_id,
            )?;
            self.entries.borrow_mut().remove(&id);
            Ok(())
        })
    }
}
