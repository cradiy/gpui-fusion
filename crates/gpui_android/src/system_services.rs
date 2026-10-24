use crate::bridge::Host;
use anyhow::{Result, ensure};
use futures::channel::oneshot;
use gpui::{AppSettings, ForegroundExecutor, NetworkStatus, Subscription, Task};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

type AudioObserver = Rc<RefCell<Box<dyn FnMut(Result<gpui::AudioOutputState>)>>>;

type Observer = Rc<RefCell<Box<dyn FnMut(NetworkStatus)>>>;

pub(crate) struct SystemServices {
    host: Arc<Host>,
    next: Cell<u64>,
    closed: Cell<bool>,
    observers: RefCell<HashMap<u64, Observer>>,
    audio_observers: RefCell<HashMap<u64, AudioObserver>>,
    settings: RefCell<HashMap<u64, oneshot::Sender<Result<()>>>>,
}

impl SystemServices {
    pub(crate) fn new(host: Arc<Host>) -> Rc<Self> {
        Rc::new(Self {
            host,
            next: Cell::new(0),
            closed: Cell::new(false),
            observers: RefCell::default(),
            audio_observers: RefCell::default(),
            settings: RefCell::default(),
        })
    }

    fn token(&self) -> Result<u64> {
        ensure!(!self.closed.get(), "Android system services session closed");
        let token = self
            .next
            .get()
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("system service request IDs exhausted"))?;
        self.next.set(token);
        Ok(token)
    }

    pub(crate) fn observe_audio_outputs(
        self: &Rc<Self>,
        callback: Box<dyn FnMut(Result<gpui::AudioOutputState>)>,
    ) -> Result<Subscription> {
        let token = self.token()?;
        self.audio_observers
            .borrow_mut()
            .insert(token, Rc::new(RefCell::new(callback)));
        if let Err(error) = self.host.audio_output_operation(0, token, "") {
            self.audio_observers.borrow_mut().remove(&token);
            return Err(error);
        }
        let weak = Rc::downgrade(self);
        Ok(Subscription::new(move || {
            if let Some(state) = weak.upgrade() {
                state.audio_observers.borrow_mut().remove(&token);
                if !state.closed.get()
                    && let Err(error) = state.host.audio_output_operation(1, token, "")
                {
                    log::warn!("Android audio observer cleanup failed: {error:#}");
                }
            }
        }))
    }

    pub(crate) fn audio_outputs_changed(&self, token: u64, json: &str, error: Option<String>) {
        let callback = self.audio_observers.borrow().get(&token).cloned();
        if let Some(callback) = callback {
            let state = error.map_or_else(
                || decode_audio_outputs(json),
                |error| Err(anyhow::anyhow!(error)),
            );
            callback.borrow_mut()(state);
        }
    }

    pub(crate) fn open_audio_output_settings(
        &self,
        foreground: &ForegroundExecutor,
    ) -> Task<Result<()>> {
        let token = match self.token() {
            Ok(token) => token,
            Err(error) => return Task::ready(Err(error)),
        };
        let (tx, rx) = oneshot::channel();
        self.settings.borrow_mut().insert(token, tx);
        if let Err(error) = self.host.audio_output_operation(3, token, "") {
            self.settings.borrow_mut().remove(&token);
            return Task::ready(Err(error));
        }
        foreground.spawn(async move { rx.await? })
    }

    pub(crate) fn network_status(&self) -> Result<NetworkStatus> {
        ensure!(!self.closed.get(), "Android system services session closed");
        decode_network(self.host.network_status()?)
    }

    pub(crate) fn observe_network(
        self: &Rc<Self>,
        callback: Box<dyn FnMut(NetworkStatus)>,
    ) -> Result<Subscription> {
        let token = self.token()?;
        self.observers
            .borrow_mut()
            .insert(token, Rc::new(RefCell::new(callback)));
        if let Err(error) = self.host.observe_network(token, true) {
            let _observer = self.observers.borrow_mut().remove(&token);
            return Err(error);
        }
        let weak = Rc::downgrade(self);
        Ok(Subscription::new(move || {
            if let Some(state) = weak.upgrade() {
                let _observer = state.observers.borrow_mut().remove(&token);
                if !state.closed.get()
                    && let Err(error) = state.host.observe_network(token, false)
                {
                    log::warn!("Android network observer cleanup failed: {error:#}");
                }
            }
        }))
    }

    pub(crate) fn network_changed(&self, token: u64, status: i32) -> Result<()> {
        let status = decode_network(status)?;
        let callback = self.observers.borrow().get(&token).cloned();
        if let Some(callback) = callback {
            callback.borrow_mut()(status);
        }
        Ok(())
    }

    pub(crate) fn open_settings(
        &self,
        page: AppSettings,
        foreground: &ForegroundExecutor,
    ) -> Task<Result<()>> {
        let token = match self.token() {
            Ok(token) => token,
            Err(error) => return Task::ready(Err(error)),
        };
        let (tx, rx) = oneshot::channel();
        self.settings.borrow_mut().insert(token, tx);
        if let Err(error) = self.host.open_app_settings(token, page) {
            self.settings.borrow_mut().remove(&token);
            return Task::ready(Err(error));
        }
        foreground.spawn(async move { rx.await? })
    }

    pub(crate) fn settings_result(&self, token: u64, error: Option<String>) {
        let sender = self.settings.borrow_mut().remove(&token);
        if let Some(sender) = sender {
            let _ = sender.send(error.map_or(Ok(()), |error| Err(anyhow::anyhow!(error))));
        }
    }

    pub(crate) fn close(&self) {
        self.closed.set(true);
        self.audio_observers.borrow_mut().clear();
        let observers = std::mem::take(&mut *self.observers.borrow_mut());
        drop(observers);
        let settings = std::mem::take(&mut *self.settings.borrow_mut());
        for (_, sender) in settings {
            let _ = sender.send(Err(anyhow::anyhow!(
                "Android system services session closed"
            )));
        }
    }
}

fn decode_network(bits: i32) -> Result<NetworkStatus> {
    ensure!(bits >= 0, "Android network status unavailable");
    Ok(if bits & 1 == 0 {
        NetworkStatus::Disconnected
    } else {
        NetworkStatus::Connected {
            internet_validated: (bits & 2 != 0).then_some(bits & 4 != 0),
            metered: (bits & 2 != 0).then_some(bits & 8 != 0),
        }
    })
}

fn decode_audio_outputs(json: &str) -> Result<gpui::AudioOutputState> {
    #[derive(serde::Deserialize)]
    struct Snapshot {
        devices: Vec<Device>,
        authorization_required: bool,
    }
    #[derive(serde::Deserialize)]
    struct Device {
        id: String,
        name: String,
        kind: i32,
        selected: bool,
    }
    let snapshot: Snapshot = serde_json::from_str(json)?;
    let mut ids = std::collections::HashSet::new();
    let mut devices = Vec::with_capacity(snapshot.devices.len());
    for device in snapshot.devices {
        ensure!(
            !device.id.is_empty() && ids.insert(device.id.clone()),
            "Invalid audio output identifier"
        );
        use gpui::AudioOutputKind::*;
        devices.push(gpui::AudioOutputDevice {
            id: device.id,
            name: device.name,
            selected: device.selected,
            kind: match device.kind {
                1 | 2 => BuiltIn,
                3 | 4 => Headphones,
                7 | 8 | 23 | 26 | 27 | 30 => Bluetooth,
                11 | 12 | 22 => Usb,
                9 | 10 | 29 => Hdmi,
                _ => Other,
            },
        });
    }
    Ok(gpui::AudioOutputState {
        devices,
        authorization_required: snapshot.authorization_required,
    })
}
