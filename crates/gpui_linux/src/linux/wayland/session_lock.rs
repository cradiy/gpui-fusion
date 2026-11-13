use super::client::{WaylandClientStatePtr, get_window};
use anyhow::{Context as _, Result, ensure};
use gpui::{SessionLock, SessionLockEvent};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};
use wayland_backend::client::ObjectId;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::GlobalList,
    protocol::{wl_output::WlOutput, wl_surface::WlSurface},
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::{self, ExtSessionLockManagerV1},
    ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1},
    ext_session_lock_v1::{self, ExtSessionLockV1},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pending,
    Locked,
    Retired,
}

#[derive(Clone)]
pub(super) struct LockManager {
    manager: Option<ExtSessionLockManagerV1>,
    pub(super) active: Rc<RefCell<Option<Rc<Lock>>>>,
}
impl LockManager {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<WaylandClientStatePtr>) -> Self {
        Self {
            manager: globals.bind(qh, 1..=1, ()).ok(),
            active: Rc::default(),
        }
    }
    pub fn request(
        &self,
        qh: &QueueHandle<WaylandClientStatePtr>,
        callback: Box<dyn FnMut(SessionLockEvent)>,
    ) -> Result<Rc<dyn SessionLock>> {
        let manager = self
            .manager
            .as_ref()
            .context("compositor does not support ext-session-lock-v1")?;
        let mut active = self.active.borrow_mut();
        ensure!(
            active
                .as_ref()
                .is_none_or(|lock| lock.phase.get() == Phase::Retired),
            "a session lock is already active on this connection"
        );
        let lock = Rc::new(Lock {
            proxy: manager.lock(qh, ()),
            phase: Cell::new(Phase::Pending),
            outputs: RefCell::default(),
            callback: RefCell::new(callback),
        });
        *active = Some(lock.clone());
        Ok(lock)
    }
    pub fn outputs_changed(&self) {
        let lock = self.active.borrow().clone();
        if let Some(lock) = lock.filter(|lock| lock.phase.get() != Phase::Retired) {
            (lock.callback.borrow_mut())(SessionLockEvent::OutputsChanged);
        }
    }
    pub fn surface(
        &self,
        surface: &WlSurface,
        output: &WlOutput,
        qh: &QueueHandle<WaylandClientStatePtr>,
    ) -> Result<LockSurface> {
        let lock = self
            .active
            .borrow()
            .clone()
            .context("request a session lock before creating its windows")?;
        ensure!(
            lock.phase.get() != Phase::Retired,
            "session lock has been retired"
        );
        ensure!(
            lock.outputs.borrow_mut().insert(output.id()),
            "a lock surface already exists for this output"
        );
        let proxy = lock
            .proxy
            .get_lock_surface(surface, output, qh, surface.id());
        Ok(LockSurface {
            proxy,
            output: output.id(),
            lock,
            destroyed: Cell::new(false),
        })
    }
}
pub(super) struct LockSurface {
    pub proxy: ExtSessionLockSurfaceV1,
    output: ObjectId,
    lock: Rc<Lock>,
    destroyed: Cell<bool>,
}
impl LockSurface {
    pub fn destroy(&self) {
        if !self.destroyed.replace(true) {
            self.proxy.destroy();
            self.lock.outputs.borrow_mut().remove(&self.output);
        }
    }
}
impl Drop for LockSurface {
    fn drop(&mut self) {
        self.destroy();
    }
}

pub(super) struct Lock {
    proxy: ExtSessionLockV1,
    phase: Cell<Phase>,
    outputs: RefCell<HashSet<ObjectId>>,
    callback: RefCell<Box<dyn FnMut(SessionLockEvent)>>,
}
impl SessionLock for Lock {
    fn unlock(&self) -> Result<()> {
        ensure!(
            self.phase.get() == Phase::Locked,
            "session lock is not confirmed or was retired"
        );
        self.phase.set(Phase::Retired);
        self.proxy.unlock_and_destroy();
        Ok(())
    }
}
impl Dispatch<ExtSessionLockV1, ()> for WaylandClientStatePtr {
    fn event(
        this: &mut Self,
        proxy: &ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let client = this.get_client();
        let lock = client.borrow().globals.session_lock.active.borrow().clone();
        let Some(lock) = lock.filter(|lock| lock.proxy == *proxy) else {
            return;
        };
        match event {
            ext_session_lock_v1::Event::Locked if lock.phase.get() == Phase::Pending => {
                lock.phase.set(Phase::Locked);
                (lock.callback.borrow_mut())(SessionLockEvent::Locked);
            }
            ext_session_lock_v1::Event::Finished => {
                match lock.phase.replace(Phase::Retired) {
                    Phase::Pending => lock.proxy.destroy(),
                    Phase::Locked => lock.proxy.unlock_and_destroy(),
                    Phase::Retired => return,
                }
                (lock.callback.borrow_mut())(SessionLockEvent::Finished);
            }
            _ => (),
        }
    }
}
delegate_noop!(WaylandClientStatePtr: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);

impl Dispatch<ExtSessionLockSurfaceV1, ObjectId> for WaylandClientStatePtr {
    fn event(
        this: &mut Self,
        _: &ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        surface_id: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let client = this.get_client();
        let window = get_window(&mut client.borrow_mut(), surface_id);
        if let (
            Some(window),
            ext_session_lock_surface_v1::Event::Configure {
                serial,
                width,
                height,
            },
        ) = (window, event)
        {
            window.handle_lock_configure(serial, width, height);
        }
    }
}
