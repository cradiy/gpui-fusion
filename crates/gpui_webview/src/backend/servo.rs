use super::{ClipboardRequest, Engine, ImeState, Input, Signals, wayland::ChildSurface};
use crate::{
    Url, WebViewDialog, WebViewDialogKind, WebViewEvent, WebViewOptions, dialog::DialogCancellation,
};
use ::servo::{EventLoopWaker, RenderingContext, WebViewDelegate};
use anyhow::Result;
use gpui::{App, Bounds, CursorStyle, Global, MouseButton, Pixels, Point, Window};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, Weak},
};

type WakeTargets = Arc<Mutex<Vec<Weak<async_channel::Sender<()>>>>>;
#[derive(Clone)]
struct RuntimeWaker(WakeTargets);
impl EventLoopWaker for RuntimeWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {
        self.0.lock().unwrap().retain(|target| {
            let Some(target) = target.upgrade() else {
                return false;
            };
            let _ = target.try_send(());
            true
        });
    }
}

struct Runtime {
    servo: ::servo::Servo,
    wakes: WakeTargets,
}
impl Global for Runtime {}
impl Runtime {
    fn acquire(cx: &mut App, signals: &Signals) -> ::servo::Servo {
        if !cx.has_global::<Self>() {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            let wakes = Arc::new(Mutex::new(Vec::new()));
            let servo = ::servo::ServoBuilder::default()
                .preferences(::servo::Preferences {
                    dom_web_animations_enabled: true,
                    dom_exec_command_enabled: true,
                    dom_async_clipboard_enabled: true,
                    dom_async_clipboard_read_enabled: false,
                    dom_intersection_observer_enabled: true,
                    dom_adoptedstylesheet_enabled: true,
                    dom_visual_viewport_enabled: true,
                    layout_css_masking_enabled: true,
                    session_history_max_length: 0,
                    ..Default::default()
                })
                .event_loop_waker(Box::new(RuntimeWaker(wakes.clone())))
                .build();
            cx.set_global(Self { servo, wakes });
        }
        let runtime = cx.global::<Self>();
        runtime
            .wakes
            .lock()
            .unwrap()
            .push(Arc::downgrade(&signals.wake));
        runtime.servo.clone()
    }
}

struct ServoDelegate {
    signals: Rc<Signals>,
    ime_id: Cell<Option<::servo::EmbedderControlId>>,
    dialogs: Rc<RefCell<Vec<(::servo::EmbedderControlId, DialogCancellation)>>>,
}
impl std::ops::Deref for ServoDelegate {
    type Target = Signals;
    fn deref(&self) -> &Signals {
        &self.signals
    }
}
impl WebViewDelegate for ServoDelegate {
    fn notify_closed(&self, _: ::servo::WebView) {
        self.emit(WebViewEvent::Closed);
    }

    fn request_create_new(
        &self,
        parent: ::servo::WebView,
        request: ::servo::CreateNewWebViewRequest,
    ) {
        self.emit(WebViewEvent::NewWindowRequested(
            crate::WebViewWindowRequest::new(
                parent.url(),
                crate::new_window::NewWindow::Servo(request),
            ),
        ));
    }

    fn show_console_message(
        &self,
        _: ::servo::WebView,
        level: ::servo::ConsoleLogLevel,
        message: String,
    ) {
        use crate::WebViewConsoleLevel as Level;
        let level = match level {
            ::servo::ConsoleLogLevel::Log => Level::Log,
            ::servo::ConsoleLogLevel::Debug => Level::Debug,
            ::servo::ConsoleLogLevel::Info => Level::Info,
            ::servo::ConsoleLogLevel::Warn => Level::Warn,
            ::servo::ConsoleLogLevel::Error => Level::Error,
            ::servo::ConsoleLogLevel::Trace => Level::Trace,
            ::servo::ConsoleLogLevel::Dir => Level::Dir,
        };
        self.emit(WebViewEvent::ConsoleMessage(crate::WebViewConsoleMessage {
            level,
            message,
        }));
    }

    fn show_embedder_control(&self, view: ::servo::WebView, control: ::servo::EmbedderControl) {
        let id = control.id();
        if let ::servo::EmbedderControl::SimpleDialog(dialog) = control {
            let kind = match &dialog {
                ::servo::SimpleDialog::Alert(_) => WebViewDialogKind::Alert,
                ::servo::SimpleDialog::Confirm(_) => WebViewDialogKind::Confirm,
                ::servo::SimpleDialog::Prompt(prompt) => WebViewDialogKind::Prompt {
                    default_value: prompt.current_value().into(),
                },
            };
            let wake = self.wake.clone();
            let request = WebViewDialog::new(
                kind,
                dialog.message().into(),
                view.url(),
                Box::new(move |value| {
                    match (dialog, value) {
                        (::servo::SimpleDialog::Prompt(mut prompt), Some(value)) => {
                            prompt.set_current_value(&value);
                            prompt.confirm();
                        }
                        (dialog, Some(_)) => dialog.confirm(),
                        (dialog, None) => dialog.dismiss(),
                    }
                    let _ = wake.try_send(());
                }),
            );
            self.dialogs
                .borrow_mut()
                .retain(|(_, pending)| pending.is_pending());
            self.dialogs.borrow_mut().push((id, request.cancellation()));
            self.emit(WebViewEvent::DialogRequested(request));
            return;
        }
        let ::servo::EmbedderControl::InputMethod(control) = control else {
            return;
        };
        if self.ime_id.replace(Some(control.id())) != Some(control.id()) {
            self.ime_revision
                .set(self.ime_revision.get().wrapping_add(1));
        }
        let rect = control.position();
        let mode = if control.input_method_type() == ::servo::InputMethodType::Password {
            gpui::TextInputMode::Password
        } else if control.multiline() {
            gpui::TextInputMode::Multiline
        } else {
            gpui::TextInputMode::SingleLine
        };
        let purpose = match control.input_method_type() {
            ::servo::InputMethodType::Email => gpui::TextInputPurpose::Email,
            ::servo::InputMethodType::Url => gpui::TextInputPurpose::Url,
            ::servo::InputMethodType::Tel => gpui::TextInputPurpose::Phone,
            ::servo::InputMethodType::Number => gpui::TextInputPurpose::Number {
                decimal: true,
                signed: true,
            },
            _ => gpui::TextInputPurpose::Text,
        };
        *self.ime.borrow_mut() = Some(ImeState {
            bounds: Bounds::new(
                gpui::point(gpui::px(rect.min.x as f32), gpui::px(rect.min.y as f32)),
                gpui::size(
                    gpui::px(rect.size().width as f32),
                    gpui::px(rect.size().height as f32),
                ),
            ),
            mode,
            purpose,
        });
        self.request_frame();
    }
    fn hide_embedder_control(&self, _: ::servo::WebView, id: ::servo::EmbedderControlId) {
        self.dialogs
            .borrow_mut()
            .retain(|(pending, _)| *pending != id);
        if self.ime_id.get() == Some(id) {
            self.ime_id.set(None);
            self.ime_revision
                .set(self.ime_revision.get().wrapping_add(1));
            self.ime.borrow_mut().take();
            self.request_frame();
        }
    }
    fn notify_new_frame_ready(&self, _: ::servo::WebView) {
        self.request_frame();
    }
    fn notify_animating_changed(&self, _: ::servo::WebView, value: bool) {
        self.animating.set(value);
        let _ = self.wake.try_send(());
    }
    fn notify_page_title_changed(&self, _: ::servo::WebView, title: Option<String>) {
        self.emit(WebViewEvent::TitleChanged(title));
    }
    fn notify_load_status_changed(&self, view: ::servo::WebView, status: ::servo::LoadStatus) {
        match status {
            ::servo::LoadStatus::Started => self.emit(WebViewEvent::LoadStarted),
            ::servo::LoadStatus::Complete => self.emit(WebViewEvent::LoadFinished),
            _ => {}
        }
        if let Some(url) = view.url() {
            self.emit(WebViewEvent::UrlChanged(url));
        }
    }
    fn notify_crashed(&self, _: ::servo::WebView, reason: String, _: Option<String>) {
        self.emit(WebViewEvent::Failed(reason));
    }
    fn notify_cursor_changed(&self, _: ::servo::WebView, cursor: ::servo::Cursor) {
        let cursor = match cursor {
            ::servo::Cursor::Text => CursorStyle::IBeam,
            ::servo::Cursor::Pointer => CursorStyle::PointingHand,
            ::servo::Cursor::Crosshair => CursorStyle::Crosshair,
            ::servo::Cursor::Grab => CursorStyle::OpenHand,
            ::servo::Cursor::Grabbing => CursorStyle::ClosedHand,
            _ => CursorStyle::Arrow,
        };
        if self.cursor.replace(cursor) != cursor {
            self.request_frame();
        }
    }
}

impl ::servo::ClipboardDelegate for Signals {
    fn get_text(&self, _: ::servo::WebView, request: ::servo::StringRequest) {
        self.clipboard
            .borrow_mut()
            .push(ClipboardRequest::Read(Box::new(move |text| {
                request.success(text.unwrap_or_default())
            })));
        let _ = self.wake.try_send(());
    }
    fn set_text(&self, _: ::servo::WebView, text: String) {
        self.clipboard
            .borrow_mut()
            .push(ClipboardRequest::Write(text));
        let _ = self.wake.try_send(());
    }
    fn clear(&self, view: ::servo::WebView) {
        self.set_text(view, String::new());
    }
}

pub(super) struct ServoBackend {
    dialogs: Rc<RefCell<Vec<(::servo::EmbedderControlId, DialogCancellation)>>>,
    webview: ::servo::WebView,
    servo: ::servo::Servo,
    context: Rc<::servo::WindowRenderingContext>,
    child: ChildSurface,
    signals: Rc<Signals>,
    bounds: Bounds<Pixels>,
    scale: Cell<f32>,
    input_scale: Point<f32>,
    composition_revision: Option<u64>,
}
impl ServoBackend {
    pub fn new(
        options: &WebViewOptions,
        new_window: Option<crate::new_window::NewWindow>,
        signals: Rc<Signals>,
        window: &Window,
        cx: &mut App,
    ) -> Result<Self> {
        let child = ChildSurface::new(window)?;
        let context = Rc::new(
            ::servo::WindowRenderingContext::new(
                child.display_handle()?,
                child.window_handle()?,
                dpi::PhysicalSize::new(1, 1),
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?,
        );
        context
            .make_current()
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let servo = Runtime::acquire(cx, &signals);
        let dialogs = Rc::new(RefCell::new(Vec::new()));
        let builder = match new_window {
            Some(crate::new_window::NewWindow::Servo(request)) => request.builder(context.clone()),
            None => ::servo::WebViewBuilder::new(&servo, context.clone()).url(options.url.clone()),
        };
        let webview = builder
            .hidpi_scale_factor(euclid::Scale::new(window.scale_factor()))
            .delegate(Rc::new(ServoDelegate {
                signals: signals.clone(),
                ime_id: Cell::new(None),
                dialogs: dialogs.clone(),
            }))
            .clipboard_delegate(signals.clone())
            .build();
        webview.show();
        Ok(Self {
            dialogs,
            webview,
            servo,
            context,
            child,
            signals,
            bounds: Bounds::default(),
            scale: Cell::new(window.scale_factor()),
            input_scale: gpui::point(window.scale_factor(), window.scale_factor()),
            composition_revision: None,
        })
    }
    fn point(&self, position: Point<Pixels>) -> ::servo::WebViewPoint {
        ::servo::DevicePoint::new(
            (f32::from(position.x) - f32::from(self.bounds.origin.x).round()) * self.input_scale.x,
            (f32::from(position.y) - f32::from(self.bounds.origin.y).round()) * self.input_scale.y,
        )
        .into()
    }
}
impl Drop for ServoBackend {
    fn drop(&mut self) {
        self.dialogs.borrow_mut().clear();
    }
}
impl Engine for ServoBackend {
    fn place(&mut self, bounds: Bounds<Pixels>, scale: f32) {
        if self.bounds == bounds && self.scale.get() == scale {
            return;
        }
        self.bounds = bounds;
        self.scale.set(scale);
        let width = f32::from(bounds.size.width).round().max(1.);
        let height = f32::from(bounds.size.height).round().max(1.);
        let physical_width = (width * scale).round().max(1.) as u32;
        let physical_height = (height * scale).round().max(1.) as u32;
        self.input_scale = gpui::point(
            physical_width as f32 / width,
            physical_height as f32 / height,
        );
        self.child.place(
            f32::from(bounds.origin.x).round() as i32,
            f32::from(bounds.origin.y).round() as i32,
            width as i32,
            height as i32,
        );
        self.webview
            .set_hidpi_scale_factor(euclid::Scale::new(scale));
        self.webview
            .resize(dpi::PhysicalSize::new(physical_width, physical_height));
        self.signals.request_frame();
    }
    fn paint(&mut self) -> Result<()> {
        if self.signals.dirty.replace(false) {
            self.context
                .make_current()
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            if self.webview.paint_if_ready() {
                self.context.present();
            }
        }
        Ok(())
    }
    fn poll(&mut self) {
        self.servo.spin_event_loop();
        if self.signals.ime.borrow().is_none()
            || self.composition_revision != Some(self.signals.ime_revision.get())
        {
            self.composition_revision = None;
        }
    }
    fn input(&mut self, input: Input) {
        use ::servo::{InputEvent, Key, MouseButtonAction, NamedKey};
        let event = match input {
            Input::Composition { text, commit } => {
                let revision = self.signals.ime_revision.get();
                if self.composition_revision != Some(revision) {
                    self.webview.notify_input_event(InputEvent::Ime(
                        ::servo::ImeEvent::Composition(::servo::CompositionEvent {
                            state: ::servo::CompositionState::Start,
                            data: String::new(),
                        }),
                    ));
                }
                self.composition_revision = (!commit).then_some(revision);
                if commit {
                    self.webview.notify_input_event(InputEvent::Ime(
                        ::servo::ImeEvent::Composition(::servo::CompositionEvent {
                            state: ::servo::CompositionState::Update,
                            data: text.clone(),
                        }),
                    ));
                }
                InputEvent::Ime(::servo::ImeEvent::Composition(::servo::CompositionEvent {
                    state: if commit {
                        ::servo::CompositionState::End
                    } else {
                        ::servo::CompositionState::Update
                    },
                    data: text,
                }))
            }
            Input::Focus(focused) => {
                self.webview.set_focused(focused);
                return;
            }
            Input::Move(position) => {
                InputEvent::MouseMove(::servo::MouseMoveEvent::new(self.point(position)))
            }
            Input::Button {
                position,
                button,
                pressed,
            } => {
                let button = match button {
                    MouseButton::Left => ::servo::MouseButton::Primary,
                    MouseButton::Right => ::servo::MouseButton::Secondary,
                    MouseButton::Middle => ::servo::MouseButton::Auxiliary,
                    _ => return,
                };
                InputEvent::MouseButton(::servo::MouseButtonEvent::new(
                    if pressed {
                        MouseButtonAction::Down
                    } else {
                        MouseButtonAction::Up
                    },
                    button,
                    self.point(position),
                ))
            }
            Input::Scroll { position, delta } => InputEvent::Wheel(::servo::WheelEvent::new(
                ::servo::WheelDelta {
                    x: f32::from(delta.x) as f64,
                    y: f32::from(delta.y) as f64,
                    z: 0.,
                    mode: ::servo::WheelMode::DeltaPixel,
                },
                self.point(position),
            )),
            Input::Key {
                keystroke,
                pressed,
                repeat,
            } => {
                let named = match keystroke.key.as_str() {
                    "backspace" => Some(NamedKey::Backspace),
                    "delete" => Some(NamedKey::Delete),
                    "enter" => Some(NamedKey::Enter),
                    "tab" => Some(NamedKey::Tab),
                    "escape" => Some(NamedKey::Escape),
                    "left" => Some(NamedKey::ArrowLeft),
                    "right" => Some(NamedKey::ArrowRight),
                    "up" => Some(NamedKey::ArrowUp),
                    "down" => Some(NamedKey::ArrowDown),
                    "home" => Some(NamedKey::Home),
                    "end" => Some(NamedKey::End),
                    "pageup" => Some(NamedKey::PageUp),
                    "pagedown" => Some(NamedKey::PageDown),
                    _ => None,
                };
                let key = if let Some(named) = named {
                    Key::Named(named)
                } else if keystroke.key == "space" {
                    Key::Character(" ".into())
                } else if let Some(text) = keystroke.key_char {
                    Key::Character(text)
                } else if keystroke.key.chars().count() == 1 {
                    Key::Character(keystroke.key)
                } else {
                    return;
                };
                let mut modifiers = ::servo::Modifiers::empty();
                modifiers.set(::servo::Modifiers::CONTROL, keystroke.modifiers.control);
                modifiers.set(::servo::Modifiers::SHIFT, keystroke.modifiers.shift);
                modifiers.set(::servo::Modifiers::ALT, keystroke.modifiers.alt);
                modifiers.set(::servo::Modifiers::META, keystroke.modifiers.platform);
                InputEvent::Keyboard(::servo::KeyboardEvent::new_without_event(
                    if pressed {
                        ::servo::KeyState::Down
                    } else {
                        ::servo::KeyState::Up
                    },
                    key,
                    ::servo::Code::Unidentified,
                    ::servo::Location::Standard,
                    modifiers,
                    repeat,
                    false,
                ))
            }
        };
        self.webview.notify_input_event(event);
    }
    fn navigate(&mut self, url: Url) {
        self.dialogs.borrow_mut().clear();
        self.webview.load(url);
    }
    fn reload(&mut self) {
        self.dialogs.borrow_mut().clear();
        self.webview.reload();
    }
    fn back(&mut self) {
        self.dialogs.borrow_mut().clear();
        self.webview.go_back(1);
    }
    fn forward(&mut self) {
        self.dialogs.borrow_mut().clear();
        self.webview.go_forward(1);
    }
}
