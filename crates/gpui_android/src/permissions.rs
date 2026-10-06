use crate::bridge::Host;
use anyhow::{Result, bail, ensure};
use futures::channel::oneshot;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// Current Android permission state. Recheck before accessing protected resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionStatus {
    /// Access is currently granted.
    Granted,
    /// Access is denied. A false rationale hint does not prove permanent denial.
    Denied {
        /// Android recommends explaining the permission before another request.
        should_show_rationale: bool,
    },
}

pub(crate) fn decode_status(code: i32) -> Result<PermissionStatus> {
    match code {
        0 => Ok(PermissionStatus::Granted),
        1 | 2 => Ok(PermissionStatus::Denied {
            should_show_rationale: code == 2,
        }),
        -1 => bail!("Android permission request was cancelled"),
        -2 => bail!("permission is not declared in the Android manifest"),
        -3 => bail!("permission is unknown or requires a specialized Android authorization flow"),
        -4 => bail!("permission requests require an attached, active Activity"),
        -5 => bail!("another Android permission request is pending"),
        _ => bail!("Android could not request this permission"),
    }
}

type Pending = (u64, oneshot::Sender<Result<PermissionStatus>>);

pub(crate) struct PermissionState {
    host: Arc<Host>,
    pending: RefCell<Option<Pending>>,
    next: Cell<u64>,
    closed: Cell<bool>,
}

impl PermissionState {
    pub(crate) fn new(host: Arc<Host>) -> Rc<Self> {
        Rc::new(Self {
            host,
            pending: RefCell::new(None),
            next: Cell::new(0),
            closed: Cell::new(false),
        })
    }

    pub(crate) fn complete(&self, token: u64, status: i32) {
        let pending = {
            let mut slot = self.pending.borrow_mut();
            if slot.as_ref().is_some_and(|(id, _)| *id == token) {
                slot.take()
            } else {
                None
            }
        };
        if let Some((_, sender)) = pending {
            let _ = sender.send(decode_status(status));
        }
    }

    pub(crate) fn close(&self) {
        self.closed.set(true);
        let pending = self.pending.borrow_mut().take();
        if let Some((_, sender)) = pending {
            let _ = sender.send(Err(anyhow::anyhow!("Android permission session closed")));
        }
    }
}

/// Main-thread access to a session's Android permissions.
/// Obtain this handle from [`crate::AndroidPlatform::permissions`] during application startup.
#[derive(Clone)]
pub struct AndroidPermissions(pub(crate) Rc<PermissionState>);

impl AndroidPermissions {
    /// Checks a manifest-declared normal or dangerous permission without showing a dialog.
    pub fn status(&self, permission: &str) -> Result<PermissionStatus> {
        ensure!(!self.0.closed.get(), "Android permission session closed");
        decode_status(self.0.host.permission_status(permission)?)
    }

    /// Requests one manifest-declared runtime permission after a user action.
    /// Only one dialog may be pending per session. Host detachment cancels the request.
    /// Dropping the future discards its result; it does not dismiss a system dialog.
    pub async fn request(&self, permission: &str) -> Result<PermissionStatus> {
        if self.status(permission)? == PermissionStatus::Granted {
            return Ok(PermissionStatus::Granted);
        }
        ensure!(
            self.0.pending.borrow().is_none(),
            "another Android permission request is pending"
        );
        let token = self
            .0
            .next
            .get()
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("permission request identifiers exhausted"))?;
        self.0.next.set(token);
        let (tx, rx) = oneshot::channel();
        *self.0.pending.borrow_mut() = Some((token, tx));
        let _guard = RequestGuard {
            state: self.0.clone(),
            token,
        };
        self.0.host.request_permission(permission, token)?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Android permission request cancelled"))?
    }
}

struct RequestGuard {
    state: Rc<PermissionState>,
    token: u64,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let removed = {
            let mut slot = self.state.pending.borrow_mut();
            if slot.as_ref().is_some_and(|(id, _)| *id == self.token) {
                slot.take().is_some()
            } else {
                false
            }
        };
        if removed {
            let _ = self.state.host.cancel_permission(self.token);
        }
    }
}
