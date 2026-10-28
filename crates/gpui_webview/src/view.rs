use crate::{
    Url, WebViewEvent, WebViewOptions, WebViewStatus,
    backend::{self, Engine, Input, Signals},
};
use gpui::{prelude::*, *};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};
#[path = "input.rs"]
mod input;

/// A native browser rectangle. Its parent determines its size through GPUI layout.
///
/// Initialization is deferred until the containing window has presented a frame.
/// Subscribe to [`WebViewEvent`] for readiness and initialization failures.
pub struct WebView {
    options: WebViewOptions,
    new_window: Option<crate::WebViewWindowRequest>,
    engine: Rc<RefCell<Option<Box<dyn Engine>>>>,
    signals: Rc<Signals>,
    status: WebViewStatus,
    focus: FocusHandle,
    pressed_buttons: Vec<MouseButton>,
    pointer_position: Point<Pixels>,
    marked_text: Option<String>,
    input_revision: u64,
    pending_composition: Vec<(u64, String, bool)>,
    _focus_subscriptions: Vec<Subscription>,
}
impl EventEmitter<WebViewEvent> for WebView {}
impl Focusable for WebView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl WebView {
    pub fn new(options: WebViewOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        let signals = Rc::new(Signals {
            wake: Arc::new(sender),
            dirty: Cell::new(false),
            animating: Cell::new(false),
            cursor: Cell::new(CursorStyle::Arrow),
            events: RefCell::new(Vec::new()),
            ime: RefCell::new(None),
            ime_revision: Cell::new(0),
            clipboard: RefCell::new(Vec::new()),
        });
        let weak_signals = Rc::downgrade(&signals);
        cx.spawn(async move |this, cx| {
            loop {
                let Some(signals) = weak_signals.upgrade() else {
                    break;
                };
                let animating = signals.animating.get();
                drop(signals);
                if animating {
                    let _ = futures::future::select(
                        Box::pin(receiver.recv()),
                        Box::pin(cx.background_executor().timer(Duration::from_millis(16))),
                    )
                    .await;
                } else if receiver.recv().await.is_err() {
                    break;
                }
                // Bound work per host turn even when the engine produces more events while polling.
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
                while receiver.try_recv().is_ok() {}
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let weak = cx.weak_entity();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.initialize(window, cx));
            });
        });
        let focus = cx.focus_handle();
        let weak = cx.weak_entity();
        let window_id = window.window_handle().window_id();
        let subscriptions = vec![
            cx.on_focus(&focus, window, |this, _, cx| {
                this.input(Input::Focus(true), cx)
            }),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.invalidate_input();
                this.pointer_position = window.mouse_position();
                for button in std::mem::take(&mut this.pressed_buttons) {
                    this.input(
                        Input::Button {
                            position: this.pointer_position,
                            button,
                            pressed: false,
                        },
                        cx,
                    );
                }
                this.input(Input::Focus(false), cx)
            }),
            cx.on_window_closed(move |cx, closed| {
                if closed == window_id {
                    let _ = weak.update(cx, |this, cx| this.close(cx));
                }
            }),
        ];
        Self {
            options,
            new_window: None,
            engine: Rc::new(RefCell::new(None)),
            signals,
            status: WebViewStatus::Initializing,
            focus,
            pressed_buttons: Vec::new(),
            pointer_position: Point::default(),
            marked_text: None,
            input_revision: 0,
            pending_composition: Vec::new(),
            _focus_subscriptions: subscriptions,
        }
    }

    pub fn status(&self) -> &WebViewStatus {
        &self.status
    }

    /// Attach an auxiliary-window request when this view initializes.
    /// The engine preserves the requested destination and opener semantics.
    pub fn from_window_request(
        request: crate::WebViewWindowRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::new(
            WebViewOptions::new(
                crate::WebViewBackend::Servo,
                Url::parse("about:blank").unwrap(),
            ),
            window,
            cx,
        );
        view.new_window = Some(request);
        view
    }

    /// Permanently release this view's native resources. Create another view to reopen it.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.status == WebViewStatus::Closed {
            return;
        }
        self.engine.borrow_mut().take();
        if let Some(request) = self.new_window.take() {
            request.cancel();
        }
        self.pressed_buttons.clear();
        self.marked_text = None;
        self.pending_composition.clear();
        self.signals.ime.borrow_mut().take();
        self.signals
            .ime_revision
            .set(self.signals.ime_revision.get().wrapping_add(1));
        self.signals.clipboard.borrow_mut().clear();
        self.signals.wake.close();
        self.signals.animating.set(false);
        self.signals.events.borrow_mut().clear();
        self.status = WebViewStatus::Closed;
        cx.emit(WebViewEvent::Closed);
        cx.notify();
    }

    /// Navigate immediately, or replace the initial URL while initialization is pending.
    pub fn navigate(&mut self, url: Url, cx: &mut Context<Self>) {
        if let Some(request) = self.new_window.take() {
            request.cancel();
        }
        self.invalidate_input();
        self.options.url = url.clone();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.navigate(url);
        }
        self.poll(cx);
    }
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.invalidate_input();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.reload();
        }
        self.poll(cx);
    }
    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        self.invalidate_input();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.back();
        }
        self.poll(cx);
    }
    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.invalidate_input();
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.forward();
        }
        self.poll(cx);
    }
    fn initialize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.status != WebViewStatus::Initializing {
            return;
        }
        let request = self
            .new_window
            .take()
            .map(|request| request.take())
            .transpose();
        match request.and_then(|request| {
            backend::create(&self.options, request, self.signals.clone(), window, cx)
        }) {
            Ok(engine) => {
                *self.engine.borrow_mut() = Some(engine);
                self.status = WebViewStatus::Ready;
                self.signals.emit(WebViewEvent::Ready);
                self.signals.request_frame();
            }
            Err(error) => {
                self.status = WebViewStatus::Failed(error.to_string());
                self.signals.emit(WebViewEvent::Failed(error.to_string()));
            }
        }
        self.poll(cx);
        cx.notify();
    }
    fn input(&mut self, input: Input, cx: &mut Context<Self>) {
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.input(input);
        }
        self.poll(cx);
    }
    fn poll(&mut self, cx: &mut Context<Self>) {
        if let Some(engine) = self.engine.borrow_mut().as_mut() {
            engine.poll();
        }
        let revision = self.signals.ime_revision.get();
        if self.input_revision != revision || self.signals.ime.borrow().is_none() {
            self.marked_text = None;
            self.pending_composition
                .retain(|(queued, _, _)| *queued == revision);
            self.input_revision = revision;
        }
        let clipboard = std::mem::take(&mut *self.signals.clipboard.borrow_mut());
        for request in clipboard {
            match request {
                backend::ClipboardRequest::Read(reply) => {
                    let read = cx.read_from_clipboard_async();
                    cx.spawn(async move |_, _| {
                        reply(read.await.ok().flatten().and_then(|item| item.text()));
                    })
                    .detach();
                }
                backend::ClipboardRequest::Write(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text))
                }
            }
        }
        let events = std::mem::take(&mut *self.signals.events.borrow_mut());
        if self.signals.dirty.get() || !events.is_empty() {
            cx.notify();
        }
        for event in events {
            if matches!(event, WebViewEvent::Closed) {
                self.close(cx);
                break;
            }
            if let WebViewEvent::Failed(message) = &event {
                self.status = WebViewStatus::Failed(message.clone());
            }
            cx.emit(event);
        }
    }
}
impl Drop for WebView {
    fn drop(&mut self) {
        self.signals.wake.close();
        self.engine.borrow_mut().take();
    }
}
impl Render for WebView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.engine.clone();
        let paint = self.engine.clone();
        let signals = self.signals.clone();
        let pointer_view = cx.weak_entity();
        let input_view = cx.entity();
        let focus = self.focus.clone();
        let mut element = div()
            .id("webview")
            .size_full()
            .track_focus(&self.focus)
            .cursor(self.signals.cursor.get());
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            element = element.on_mouse_down(
                button,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.input(Input::Focus(true), cx);
                    this.pointer_position = e.position;
                    if !this.pressed_buttons.contains(&button) {
                        this.pressed_buttons.push(button);
                    }
                    this.input(
                        Input::Button {
                            position: e.position,
                            button,
                            pressed: true,
                        },
                        cx,
                    );
                }),
            );
        }
        element
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                this.pointer_position = e.position;
                this.input(Input::Move(e.position), cx)
            }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                this.input(
                    Input::Scroll {
                        position: e.position,
                        delta: e.delta.pixel_delta(px(24.)),
                    },
                    cx,
                )
            }))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                this.input(
                    Input::Key {
                        keystroke: e.keystroke.clone(),
                        pressed: true,
                        repeat: e.is_held,
                    },
                    cx,
                );
                cx.stop_propagation();
            }))
            .on_key_up(cx.listener(|this, e: &KeyUpEvent, _, cx| {
                this.input(
                    Input::Key {
                        keystroke: e.keystroke.clone(),
                        pressed: false,
                        repeat: false,
                    },
                    cx,
                );
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    move |bounds, window, _| {
                        if let Some(engine) = layout.borrow_mut().as_mut() {
                            engine.place(bounds, window.scale_factor());
                        }
                    },
                    move |bounds, _, window, cx| {
                        if input_view.read(cx).signals.ime.borrow().is_some() {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, input_view.clone()),
                                cx,
                            );
                        }
                        let moving_view = pointer_view.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let _ = moving_view.update(cx, |this, cx| {
                                if !this.pressed_buttons.is_empty() {
                                    this.pointer_position = e.position;
                                    this.input(Input::Move(e.position), cx);
                                    cx.stop_propagation();
                                }
                            });
                        });
                        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let _ = pointer_view.update(cx, |this, cx| {
                                if let Some(index) = this
                                    .pressed_buttons
                                    .iter()
                                    .position(|button| *button == e.button)
                                {
                                    this.pressed_buttons.remove(index);
                                    this.pointer_position = e.position;
                                    this.input(
                                        Input::Button {
                                            position: e.position,
                                            button: e.button,
                                            pressed: false,
                                        },
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }
                            });
                        });
                        if let Some(engine) = paint.borrow_mut().as_mut() {
                            if let Err(error) = engine.paint() {
                                signals.emit(WebViewEvent::Failed(error.to_string()));
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}
