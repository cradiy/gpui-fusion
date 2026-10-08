use crate::bridge::Host;
use anyhow::{Result, anyhow, ensure};
use futures::channel::oneshot;
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

/// Notification shown while Android grants foreground execution for data transfer.
#[derive(Clone, Debug, Serialize)]
pub struct DataSyncNotification {
    /// Stable Android notification channel identifier.
    pub channel: String,
    /// User-visible channel name, configurable in Android notification settings.
    pub channel_name: String,
    /// Notification title.
    pub title: String,
    /// Notification body.
    pub body: String,
    /// Optional drawable resource name; unavailable resources use the host's default icon.
    pub icon: Option<String>,
}

/// Why the system stopped a foreground execution lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackgroundStopReason {
    /// Android exhausted the data-sync foreground service time allowance.
    Timeout,
    /// The service was destroyed while its application process remained alive.
    ServiceStopped,
    /// The owning GPUI session was closed.
    SessionClosed,
}

struct Pending {
    ready: Option<oneshot::Sender<Result<()>>>,
    stopped: oneshot::Sender<BackgroundStopReason>,
}

pub(crate) struct BackgroundState {
    host: Arc<Host>,
    pending: RefCell<HashMap<String, Pending>>,
}

impl BackgroundState {
    pub(crate) fn new(host: Arc<Host>) -> Rc<Self> {
        Rc::new(Self {
            host,
            pending: RefCell::default(),
        })
    }

    pub(crate) fn event(&self, token: &str, event: &str, error: &str) {
        if event == "ready" {
            let ready = self
                .pending
                .borrow_mut()
                .get_mut(token)
                .and_then(|item| item.ready.take());
            if let Some(ready) = ready {
                let _ = ready.send(Ok(()));
            }
        } else {
            let pending = self.pending.borrow_mut().remove(token);
            if let Some(pending) = pending {
                if let Some(ready) = pending.ready {
                    let _ = ready.send(Err(anyhow!("Android foreground service: {error}")));
                }
                let reason = if event == "timeout" {
                    BackgroundStopReason::Timeout
                } else {
                    BackgroundStopReason::ServiceStopped
                };
                let _ = pending.stopped.send(reason);
            }
        }
    }

    pub(crate) fn close(&self) {
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        for (_, item) in pending {
            if let Some(ready) = item.ready {
                let _ = ready.send(Err(anyhow!("Android session closed")));
            }
            let _ = item.stopped.send(BackgroundStopReason::SessionClosed);
        }
    }
}

/// Session-bound Android foreground execution. Obtain through
/// [`crate::AndroidPlatform::background_execution`] during application startup.
#[derive(Clone)]
pub struct AndroidBackgroundExecution(pub(crate) Rc<BackgroundState>);

impl AndroidBackgroundExecution {
    /// Starts data-sync foreground execution after a user action in an active Activity.
    /// Requires the `data-sync` host feature and foreground service manifest permissions.
    /// Completes only after Android has promoted the service. One lease may run per process.
    /// Dropping this future cancels its pending start. No work is scheduled or persisted.
    pub async fn start_data_sync(
        &self,
        notification: DataSyncNotification,
    ) -> Result<BackgroundExecution> {
        ensure!(
            !notification.channel.trim().is_empty()
                && !notification.channel_name.trim().is_empty()
                && !notification.title.trim().is_empty(),
            "notification channel, channel name and title are required"
        );
        self.start(
            "start",
            "stop",
            serde_json::json!({"notification": notification}),
        )
        .await
    }

    pub(crate) async fn start_media_playback(&self, session: &str) -> Result<BackgroundExecution> {
        self.start(
            "media_start",
            "media_stop",
            serde_json::json!({"session": session}),
        )
        .await
    }

    async fn start(
        &self,
        operation: &str,
        stop_operation: &'static str,
        mut payload: serde_json::Value,
    ) -> Result<BackgroundExecution> {
        let token = uuid::Uuid::new_v4().to_string();
        let (ready_tx, ready_rx) = oneshot::channel();
        let (stopped_tx, stopped_rx) = oneshot::channel();
        self.0.pending.borrow_mut().insert(
            token.clone(),
            Pending {
                ready: Some(ready_tx),
                stopped: stopped_tx,
            },
        );
        let lease = BackgroundExecution {
            state: self.0.clone(),
            token,
            stopped: stopped_rx,
            stop_operation,
        };
        payload["token"] = lease.token.clone().into();
        self.0
            .host
            .background_operation(operation, &payload.to_string())?;
        ready_rx
            .await
            .map_err(|_| anyhow!("Android service start cancelled"))??;
        Ok(lease)
    }
}

/// Foreground execution allowance, not a task or a guarantee against process termination.
/// Keep this main-thread handle alive while work runs; dropping it stops the service.
/// Activity configuration changes retain it, but closing the GPUI session stops it.
pub struct BackgroundExecution {
    state: Rc<BackgroundState>,
    token: String,
    stopped: oneshot::Receiver<BackgroundStopReason>,
    stop_operation: &'static str,
}

impl BackgroundExecution {
    /// Updates notification content. Keep the original channel and channel name.
    pub fn update(&self, notification: DataSyncNotification) -> Result<()> {
        ensure!(
            self.stop_operation == "stop",
            "update media notifications through SystemMediaSession"
        );
        self.state.host.background_operation(
            "update",
            &serde_json::json!({"token": self.token, "notification": notification}).to_string(),
        )
    }

    /// Waits for a live-process timeout, service destruction or session close.
    /// Applications should cancel their work when this resolves. Process death cannot deliver a callback.
    pub async fn stopped(&mut self) -> BackgroundStopReason {
        (&mut self.stopped)
            .await
            .unwrap_or(BackgroundStopReason::SessionClosed)
    }
}

impl Drop for BackgroundExecution {
    fn drop(&mut self) {
        if self
            .state
            .pending
            .borrow_mut()
            .remove(&self.token)
            .is_some()
        {
            let _ = self
                .state
                .host
                .background_operation(self.stop_operation, &self.token);
        }
    }
}
