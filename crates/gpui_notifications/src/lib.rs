//! System notifications with stable identifiers and asynchronous response events.

use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc};
mod media;
pub use media::*;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_family = "wasm")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

/// Application identity used by the operating system, not a window title.
#[derive(Clone, Debug)]
pub struct NotificationOptions {
    /// Desktop entry ID, macOS bundle ID, Windows AUMID, or Android package ID.
    pub app_id: String,
    /// Human-readable application name.
    pub app_name: String,
    /// Android notification channel. Its importance remains under user control.
    pub channel: NotificationChannel,
    /// Optional same-origin service-worker script importing the notification worker.
    pub web_service_worker: Option<String>,
    /// Register the current executable as an unpackaged Windows notification app.
    /// Installers can register the AUMID/CLSID themselves instead. Registration persists.
    pub windows_register_application: bool,
}

impl NotificationOptions {
    pub fn new(app_id: impl Into<String>, app_name: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            app_name: app_name.into(),
            channel: NotificationChannel {
                id: "general".into(),
                name: "General".into(),
            },
            web_service_worker: None,
            windows_register_application: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
}

/// The notification system's current authorization, independent of Do Not Disturb.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationPermission {
    Granted,
    Denied,
    NotDetermined,
}

/// Features offered by this connection. Desktop policies may suppress presentation.
#[derive(Clone, Copy, Debug)]
pub struct NotificationCapabilities {
    pub max_actions: usize,
    pub inline_reply: bool,
    /// Whether dismissal events are available (not guaranteed after process exit).
    pub dismissal_events: bool,
    pub progress: bool,
    pub resource_icons: bool,
    pub image_icons: bool,
}

/// Per-notification icon source. Support is reported by the notification center.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NotificationIcon {
    /// Android drawable name or Linux icon theme name.
    Resource(String),
    /// Absolute file URI on desktop, or an HTTP(S) image URL in a browser.
    ImageUri(String),
}

/// Plain-text content. Sending the same ID replaces the previous notification.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: String,
    /// `None`, or an unsupported source kind, uses the application's system icon.
    pub icon: Option<NotificationIcon>,
    pub actions: Vec<NotificationAction>,
    /// Suppress sound where supported. System/channel settings take precedence.
    pub silent: bool,
    /// Progress in the inclusive range 0..=100; presentation is platform-specific.
    pub progress: Option<u8>,
}

impl Notification {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            ..Self::default()
        }
    }

    fn validate(&self, capabilities: NotificationCapabilities) -> Result<()> {
        validate_id(&self.id)?;
        ensure!(!self.title.trim().is_empty(), "notification title is empty");
        match &self.icon {
            Some(NotificationIcon::Resource(name)) => {
                ensure!(!name.is_empty(), "notification icon resource name is empty")
            }
            Some(NotificationIcon::ImageUri(uri)) => {
                ensure!(!uri.is_empty(), "notification icon URI is empty")
            }
            None => {}
        }
        ensure!(
            self.actions.len() <= capabilities.max_actions,
            "notification backend supports at most {} actions",
            capabilities.max_actions
        );
        let mut ids = std::collections::HashSet::new();
        for action in &self.actions {
            validate_id(&action.id)?;
            ensure!(
                action.id != "default" && action.id != "dismiss",
                "default and dismiss are reserved notification action IDs"
            );
            ensure!(
                !action.label.trim().is_empty() && ids.insert(&action.id),
                "notification actions need unique IDs and nonempty labels"
            );
            ensure!(
                action.reply_placeholder.is_none() || capabilities.inline_reply,
                "inline notification replies are not supported on this platform"
            );
        }
        ensure!(
            self.progress.is_none_or(|value| value <= 100),
            "notification progress exceeds 100"
        );
        ensure!(
            self.progress.is_none() || capabilities.progress,
            "notification progress is not supported on this platform"
        );
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "notification identifiers must contain 1..=64 ASCII letters, digits, dots, underscores or hyphens"
    );
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationAction {
    pub id: String,
    pub label: String,
    /// Adds a native text entry to this action. `Some("")` uses no placeholder.
    pub reply_placeholder: Option<String>,
}

impl NotificationAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            reply_placeholder: None,
        }
    }
    pub fn reply(mut self, placeholder: impl Into<String>) -> Self {
        self.reply_placeholder = Some(placeholder.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotificationEvent {
    Activated {
        id: String,
        action: Option<String>,
        reply: Option<String>,
    },
    Dismissed {
        id: String,
    },
    Failed {
        id: String,
        message: String,
    },
}

/// Platform integration point. Methods run on the application thread.
pub trait NotificationBackend {
    fn capabilities(&self) -> NotificationCapabilities;
    fn permission(&self, request: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>>;
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>>;
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>>;
}

struct Inner {
    backend: Rc<dyn NotificationBackend>,
    events: RefCell<Option<async_channel::Receiver<NotificationEvent>>>,
}

/// Keep one center per application/channel. Clones share the event stream and backend.
/// Dropping a center releases callbacks; call `remove` to withdraw delivered notifications.
#[derive(Clone)]
pub struct NotificationCenter(Rc<Inner>);

impl NotificationCenter {
    pub fn from_backend(
        backend: Rc<dyn NotificationBackend>,
        events: async_channel::Receiver<NotificationEvent>,
    ) -> Self {
        Self(Rc::new(Inner {
            backend,
            events: RefCell::new(Some(events)),
        }))
    }

    /// Creates the desktop/browser implementation. Android hosts use `App::notifications`.
    pub async fn new(options: NotificationOptions) -> Result<Self> {
        ensure!(
            !options.app_id.is_empty() && !options.app_name.is_empty(),
            "notification application identity is empty"
        );
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        return linux::create(options).await;
        #[cfg(target_os = "macos")]
        return macos::create(options).await;
        #[cfg(target_os = "windows")]
        return windows::create(options).await;
        #[cfg(target_family = "wasm")]
        return web::create_center(options).await;
        #[cfg(not(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "macos",
            target_os = "windows",
            target_family = "wasm"
        )))]
        anyhow::bail!("notifications require a platform host");
    }

    pub fn capabilities(&self) -> NotificationCapabilities {
        self.0.backend.capabilities()
    }
    pub fn permission(&self) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        self.0.backend.permission(false)
    }
    /// Call directly from a user gesture; browsers require transient user activation.
    pub fn request_permission(&self) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        self.0.backend.permission(true)
    }
    pub async fn show(&self, mut notification: Notification) -> Result<()> {
        notification.validate(self.capabilities())?;
        let supported = match &notification.icon {
            Some(NotificationIcon::Resource(_)) => self.capabilities().resource_icons,
            Some(NotificationIcon::ImageUri(_)) => self.capabilities().image_icons,
            None => true,
        };
        if !supported {
            notification.icon = None;
        }
        self.0.backend.show(notification).await
    }
    pub async fn remove(&self, id: impl Into<String>) -> Result<()> {
        let id = id.into();
        validate_id(&id)?;
        self.0.backend.remove(id).await
    }
    /// Takes the single response stream. Call once, then receive on a foreground task.
    pub fn take_events(&self) -> Option<async_channel::Receiver<NotificationEvent>> {
        self.0.events.borrow_mut().take()
    }
}
