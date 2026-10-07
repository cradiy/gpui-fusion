use crate::{AndroidPermissions, bridge::Host};
use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use gpui::gpui_notifications::*;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
};

thread_local! {
    static CENTERS: RefCell<HashMap<(usize, String), Weak<AndroidNotifications>>> = RefCell::default();
    static MEDIA: RefCell<HashMap<String, async_channel::Sender<MediaCommand>>> = RefCell::default();
}

pub(crate) fn deliver_media(json: &str) -> Result<()> {
    let envelope: serde_json::Value = serde_json::from_str(json)?;
    let id = envelope["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("media session ID missing"))?;
    let command: MediaCommand = serde_json::from_value(envelope["event"].clone())?;
    MEDIA.with(|sessions| {
        if let Some(sender) = sessions.borrow().get(id) {
            let _ = sender.try_send(command);
        }
    });
    Ok(())
}

pub(crate) fn create_media(
    host: Arc<Host>,
    background: crate::AndroidBackgroundExecution,
    options: MediaSessionOptions,
) -> Result<SystemMediaSession> {
    let id = uuid::Uuid::new_v4().to_string();
    host.notification_operation(
        "media_create",
        &serde_json::json!({"id": id, "app_id": options.app_id, "app_name": options.app_name})
            .to_string(),
    )?;
    let (sender, receiver) = async_channel::unbounded();
    MEDIA.with(|sessions| sessions.borrow_mut().insert(id.clone(), sender));
    Ok(SystemMediaSession::from_backend(
        Box::new(AndroidMedia {
            host,
            id,
            background,
        }),
        receiver,
    ))
}
struct AndroidMedia {
    background: crate::AndroidBackgroundExecution,
    host: Arc<Host>,
    id: String,
}
impl MediaSessionBackend for AndroidMedia {
    fn start_background_playback(&self) -> LocalBoxFuture<'_, Result<BackgroundPlayback>> {
        Box::pin(async move {
            let lease = self.background.start_media_playback(&self.id).await?;
            Ok(BackgroundPlayback::from_backend(AndroidBackgroundPlayback(
                lease,
            )))
        })
    }
    fn update(&self, state: MediaSessionState) -> Result<()> {
        self.host.notification_operation(
            "media_update",
            &serde_json::json!({"id": self.id, "state": state}).to_string(),
        )?;
        Ok(())
    }
}
struct AndroidBackgroundPlayback(crate::BackgroundExecution);
impl BackgroundPlaybackBackend for AndroidBackgroundPlayback {
    fn stopped(&mut self) -> LocalBoxFuture<'_, BackgroundPlaybackStopReason> {
        Box::pin(async move {
            match self.0.stopped().await {
                crate::BackgroundStopReason::SessionClosed => {
                    BackgroundPlaybackStopReason::SessionClosed
                }
                _ => BackgroundPlaybackStopReason::ServiceStopped,
            }
        })
    }
}
impl Drop for AndroidMedia {
    fn drop(&mut self) {
        MEDIA.with(|sessions| sessions.borrow_mut().remove(&self.id));
        let _ = self.host.notification_operation("media_close", &self.id);
    }
}
struct AndroidNotifications {
    host: Arc<Host>,
    permissions: AndroidPermissions,
    channel: String,
    events: async_channel::Sender<NotificationEvent>,
}
#[derive(serde::Deserialize)]
struct Envelope {
    channel: String,
    event: NotificationEvent,
}

pub(crate) fn deliver(host: &Arc<Host>, json: &str) -> Result<bool> {
    let envelope: Envelope = serde_json::from_str(json)?;
    let key = (Arc::as_ptr(host) as usize, envelope.channel);
    Ok(CENTERS
        .with(|centers| centers.borrow().get(&key).and_then(Weak::upgrade))
        .is_some_and(|center| center.events.try_send(envelope.event).is_ok()))
}

pub(crate) fn create(
    host: Arc<Host>,
    permissions: AndroidPermissions,
    options: NotificationOptions,
) -> Result<NotificationCenter> {
    let key = (Arc::as_ptr(&host) as usize, options.channel.id.clone());
    ensure!(
        CENTERS.with(|centers| centers.borrow().get(&key).and_then(Weak::upgrade).is_none()),
        "reuse the existing notification center for this channel"
    );
    let payload = serde_json::json!({"app_id": options.app_id, "channel": options.channel.id, "name": options.channel.name});
    host.notification_operation("create", &payload.to_string())?;
    let (events, rx) = async_channel::unbounded();
    let center = Rc::new(AndroidNotifications {
        host,
        permissions,
        channel: options.channel.id,
        events,
    });
    CENTERS.with(|centers| centers.borrow_mut().insert(key, Rc::downgrade(&center)));
    let pending = center
        .host
        .notification_operation("drain", &center.channel)?;
    for event in serde_json::from_str::<Vec<NotificationEvent>>(&pending)? {
        let _ = center.events.try_send(event);
    }
    Ok(NotificationCenter::from_backend(center, rx))
}

impl NotificationBackend for AndroidNotifications {
    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            max_actions: 3,
            inline_reply: true,
            dismissal_events: true,
            progress: true,
            resource_icons: true,
            image_icons: false,
        }
    }
    fn permission(&self, request: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        let host = self.host.clone();
        let channel = self.channel.clone();
        let permissions = self.permissions.clone();
        Box::pin(async move {
            let status = host.notification_operation("permission", &channel)?;
            if request && status == "runtime" {
                permissions
                    .request("android.permission.POST_NOTIFICATIONS")
                    .await?;
            }
            Ok(
                match host
                    .notification_operation("permission", &channel)?
                    .as_str()
                {
                    "granted" => NotificationPermission::Granted,
                    _ => NotificationPermission::Denied,
                },
            )
        })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.host.notification_operation(
                "show",
                &serde_json::json!({"channel": self.channel, "notification": notification})
                    .to_string(),
            )?;
            Ok(())
        })
    }
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.host.notification_operation(
                "remove",
                &serde_json::json!({"channel": self.channel, "id": id}).to_string(),
            )?;
            Ok(())
        })
    }
}
impl Drop for AndroidNotifications {
    fn drop(&mut self) {
        CENTERS.with(|centers| {
            centers
                .borrow_mut()
                .remove(&(Arc::as_ptr(&self.host) as usize, self.channel.clone()))
        });
    }
}
