use crate::{Url, WebViewBackend, WebViewEvent, WebViewOptions};
use anyhow::Result;
#[cfg(not(all(feature = "servo", target_os = "linux")))]
use anyhow::bail;
use gpui::{App, Bounds, CursorStyle, Keystroke, MouseButton, Pixels, Point, Window};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

#[cfg(all(feature = "servo", target_os = "linux"))]
mod servo;
#[cfg(all(feature = "servo", target_os = "linux"))]
mod wayland;

pub(crate) struct Signals {
    pub wake: Arc<async_channel::Sender<()>>,
    pub dirty: Cell<bool>,
    pub animating: Cell<bool>,
    pub visible: Cell<bool>,
    pub cursor: Cell<CursorStyle>,
    pub events: RefCell<Vec<WebViewEvent>>,
    pub ime: RefCell<Option<ImeState>>,
    pub ime_revision: Cell<u64>,
    pub clipboard: RefCell<Vec<ClipboardRequest>>,
}

pub(crate) struct ImeState {
    pub bounds: Bounds<Pixels>,
    pub mode: gpui::TextInputMode,
    pub purpose: gpui::TextInputPurpose,
}

#[cfg_attr(not(all(feature = "servo", target_os = "linux")), allow(dead_code))]
pub(crate) enum ClipboardRequest {
    Read(Box<dyn FnOnce(Option<String>)>),
    Write(String),
}
impl Signals {
    pub fn request_frame(&self) {
        if !self.dirty.replace(true) {
            let _ = self.wake.try_send(());
        }
    }
    pub fn emit(&self, event: WebViewEvent) {
        self.events.borrow_mut().push(event);
        let _ = self.wake.try_send(());
    }
}

#[cfg_attr(not(all(feature = "servo", target_os = "linux")), allow(dead_code))]
pub(crate) enum Input {
    Move(Point<Pixels>),
    Button {
        position: Point<Pixels>,
        button: MouseButton,
        pressed: bool,
    },
    Scroll {
        position: Point<Pixels>,
        delta: Point<Pixels>,
    },
    Key {
        keystroke: Keystroke,
        pressed: bool,
        repeat: bool,
    },
    Focus(bool),
    Composition {
        text: String,
        commit: bool,
    },
}

pub(crate) trait Engine {
    fn set_visible(&mut self, visible: bool);
    fn place(&mut self, bounds: Bounds<Pixels>, scale: f32);
    fn paint(&mut self) -> Result<()>;
    fn poll(&mut self);
    fn input(&mut self, input: Input);
    fn navigate(&mut self, url: Url);
    fn reload(&mut self);
    fn back(&mut self);
    fn forward(&mut self);
}

pub(crate) fn create(
    options: &WebViewOptions,
    new_window: Option<crate::new_window::NewWindow>,
    signals: Rc<Signals>,
    window: &Window,
    cx: &mut App,
) -> Result<Box<dyn Engine>> {
    match options.backend {
        #[cfg(all(feature = "servo", target_os = "linux"))]
        WebViewBackend::Servo => Ok(Box::new(servo::ServoBackend::new(
            options, new_window, signals, window, cx,
        )?)),
        #[cfg(not(all(feature = "servo", target_os = "linux")))]
        WebViewBackend::Servo => {
            let _ = (new_window, signals, window, cx);
            bail!("Servo requires the gpui_webview/servo feature and Linux Wayland")
        }
    }
}
