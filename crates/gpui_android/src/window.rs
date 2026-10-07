use crate::surface::NativeWindow;
use anyhow::Result;
use futures::channel::oneshot;
use gpui::*;
use gpui_wgpu::{GpuContext, WgpuRenderer, WgpuSurfaceConfig};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

#[derive(Debug)]
pub(crate) struct AndroidDisplay {
    size: Cell<Size<Pixels>>,
    scale: Cell<f32>,
}
impl PlatformDisplay for AndroidDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(1))
    }
    fn scale_factor(&self) -> f32 {
        self.scale.get()
    }
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(Point::default(), self.size.get())
    }
    fn default_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }
}

#[derive(Default)]
struct Callbacks {
    picture_in_picture: Option<Box<dyn FnMut(bool)>>,
    back: Option<Box<dyn FnMut()>>,
    back_gesture: Option<Box<dyn FnMut(BackGestureEvent)>>,
    frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active: Option<Box<dyn FnMut(bool)>>,
    hover: Option<Box<dyn FnMut(bool)>>,
    appearance: Option<Box<dyn FnMut()>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    insets: Option<Box<dyn FnMut(WindowInsets)>>,
    close: Option<Box<dyn FnOnce()>>,
}

pub(crate) struct AndroidWindow {
    host: Arc<crate::bridge::Host>,
    back_enabled: Cell<bool>,
    fullscreen: Cell<bool>,
    picture_in_picture: Cell<bool>,
    picture_in_picture_request: RefCell<Option<oneshot::Sender<Result<()>>>>,
    picture_in_picture_source: Cell<Option<Bounds<Pixels>>>,
    sent_picture_in_picture_source: Cell<Option<Option<Bounds<Pixels>>>>,
    insets: RefCell<WindowInsets>,
    // Renderer must be dropped before the last native window reference.
    renderer: RefCell<WgpuRenderer>,
    native: RefCell<Option<NativeWindow>>,
    pub display: Rc<AndroidDisplay>,
    pub active: Cell<bool>,
    pub appearance: Cell<WindowAppearance>,
    force_frame: Cell<bool>,
    pub(crate) pointer: Cell<Point<Pixels>>,
    pub(crate) hovered: Cell<bool>,
    modifiers: Cell<Modifiers>,
    pub(crate) handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) input_focus: Cell<Option<FocusId>>,
    pub(crate) input_epoch: Cell<u64>,
    pub(crate) input_mode: Cell<TextInputMode>,
    pub(crate) input_purpose: Cell<TextInputPurpose>,
    pub(crate) input_action: Cell<Option<TextInputAction>>,
    pub(crate) input_dirty: Cell<bool>,
    callbacks: RefCell<Callbacks>,
}

impl AndroidWindow {
    pub fn new(
        host: Arc<crate::bridge::Host>,
        native: NativeWindow,
        renderer: WgpuRenderer,
        width: i32,
        height: i32,
        density: f32,
        appearance: WindowAppearance,
    ) -> Self {
        Self {
            host,
            back_enabled: Cell::new(false),
            fullscreen: Cell::new(false),
            picture_in_picture: Cell::new(false),
            picture_in_picture_request: RefCell::default(),
            picture_in_picture_source: Cell::default(),
            sent_picture_in_picture_source: Cell::default(),
            insets: RefCell::default(),
            renderer: RefCell::new(renderer),
            native: RefCell::new(Some(native)),
            display: Rc::new(AndroidDisplay {
                size: Cell::new(size(
                    px(width as f32 / density),
                    px(height as f32 / density),
                )),
                scale: Cell::new(density),
            }),
            active: Cell::new(false),
            appearance: Cell::new(appearance),
            force_frame: Cell::new(true),
            pointer: Cell::default(),
            hovered: Cell::new(false),
            modifiers: Cell::default(),
            handler: RefCell::default(),
            input_focus: Cell::new(None),
            input_epoch: Cell::new(0),
            input_mode: Cell::new(TextInputMode::default()),
            input_purpose: Cell::new(TextInputPurpose::default()),
            input_action: Cell::new(None),
            input_dirty: Cell::new(true),
            callbacks: RefCell::default(),
        }
    }

    pub fn system_back(&self) -> bool {
        if !self.active.get() || !self.back_enabled.get() {
            return false;
        }
        let callback = self.callbacks.borrow_mut().back.take();
        let Some(mut callback) = callback else {
            return false;
        };
        callback();
        self.callbacks.borrow_mut().back.get_or_insert(callback);
        true
    }

    pub fn back_gesture(&self, event: BackGestureEvent) {
        let callback = self.callbacks.borrow_mut().back_gesture.take();
        if let Some(mut callback) = callback {
            callback(event);
            self.callbacks
                .borrow_mut()
                .back_gesture
                .get_or_insert(callback);
        }
    }

    pub fn attach(
        &self,
        native: NativeWindow,
        width: i32,
        height: i32,
        density: f32,
        context: &GpuContext,
    ) -> Result<()> {
        anyhow::ensure!(
            width > 0 && height > 0 && density.is_finite() && density > 0.,
            "invalid Android surface geometry"
        );
        let same_window = self
            .native
            .borrow()
            .as_ref()
            .is_some_and(|current| current.is_same_window(&native));
        let drawable_size = size(DevicePixels(width), DevicePixels(height));
        if same_window {
            self.renderer
                .borrow_mut()
                .update_drawable_size(drawable_size);
        } else {
            self.detach();
            let config = WgpuSurfaceConfig {
                size: drawable_size,
                transparent: false,
                preferred_present_mode: None,
            };
            let instance = context
                .borrow()
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("GPU context unavailable"))?
                .instance
                .clone();
            self.renderer
                .borrow_mut()
                .replace_surface(&native, config, &instance)?;
            *self.native.borrow_mut() = Some(native);
        }
        self.force_frame.set(true);
        let insets = self.insets.borrow().clone();
        self.set_viewport(width, height, density, insets)
    }

    pub fn set_viewport(
        &self,
        width: i32,
        height: i32,
        density: f32,
        insets: WindowInsets,
    ) -> Result<()> {
        anyhow::ensure!(
            width > 0 && height > 0 && density.is_finite() && density > 0.,
            "invalid Android viewport geometry"
        );
        let logical = size(px(width as f32 / density), px(height as f32 / density));
        let resized = self.display.size.get() != logical || self.display.scale.get() != density;
        let insets_changed = *self.insets.borrow() != insets;
        if !resized && !insets_changed {
            return Ok(());
        }
        *self.insets.borrow_mut() = insets.clone();
        self.display.scale.set(density);
        self.display.size.set(logical);
        self.force_frame.set(true);
        if resized {
            let callback = self.callbacks.borrow_mut().resize.take();
            if let Some(mut callback) = callback {
                callback(logical, density);
                self.callbacks.borrow_mut().resize = Some(callback);
            }
        }
        if insets_changed {
            let callback = self.callbacks.borrow_mut().insets.take();
            if let Some(mut callback) = callback {
                callback(insets);
                self.callbacks.borrow_mut().insets = Some(callback);
            }
        }
        Ok(())
    }

    pub fn detach(&self) {
        self.renderer.borrow_mut().unconfigure_surface();
        self.native.borrow_mut().take();
    }

    pub fn set_active(&self, active: bool) {
        if self.active.replace(active) == active {
            return;
        }
        self.force_frame.set(true);
        let callback = self.callbacks.borrow_mut().active.take();
        if let Some(mut callback) = callback {
            callback(active);
            self.callbacks.borrow_mut().active = Some(callback);
        }
    }

    pub fn frame(&self) -> Result<()> {
        if !self.active.get() && !self.picture_in_picture.get() {
            return Ok(());
        }
        self.render_frame(false)
    }

    pub fn picture_in_picture_changed(&self, enabled: bool) {
        if self.picture_in_picture.replace(enabled) == enabled {
            return;
        }
        self.force_frame.set(true);
        let callback = self.callbacks.borrow_mut().picture_in_picture.take();
        if let Some(mut callback) = callback {
            callback(enabled);
            self.callbacks
                .borrow_mut()
                .picture_in_picture
                .get_or_insert(callback);
        }
        self.host.request_frame();
    }

    pub fn picture_in_picture_result(&self, result: Result<()>) {
        if let Some(sender) = self.picture_in_picture_request.borrow_mut().take() {
            let _ = sender.send(result);
        }
    }

    fn flush_picture_in_picture_source(&self) {
        if self.picture_in_picture.get() {
            return;
        }
        let bounds = self.picture_in_picture_source.get();
        if self.sent_picture_in_picture_source.get() == Some(bounds) {
            return;
        }
        match self.host.set_picture_in_picture_source_bounds(bounds) {
            Ok(()) => self.sent_picture_in_picture_source.set(Some(bounds)),
            Err(error) => {
                log::warn!("Unable to update picture-in-picture source bounds: {error:#}")
            }
        }
    }

    pub fn set_appearance(&self, appearance: WindowAppearance) {
        if self.appearance.replace(appearance) == appearance {
            return;
        }
        self.force_frame.set(true);
        let callback = self.callbacks.borrow_mut().appearance.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks
                .borrow_mut()
                .appearance
                .get_or_insert(callback);
        }
    }

    pub fn redraw(&self) -> Result<()> {
        self.render_frame(true)
    }

    fn render_frame(&self, redraw: bool) -> Result<()> {
        if self.native.borrow().is_none() {
            return Ok(());
        }
        anyhow::ensure!(
            !self.renderer.borrow().device_lost(),
            "Android GPU device lost; close and recreate the GpuiSession"
        );
        let callback = self.callbacks.borrow_mut().frame.take();
        if let Some(mut callback) = callback {
            callback(RequestFrameOptions {
                require_presentation: redraw,
                force_render: self.force_frame.replace(false) || redraw,
            });
            self.callbacks.borrow_mut().frame = Some(callback);
        }
        self.flush_picture_in_picture_source();
        Ok(())
    }

    pub fn input(&self, input: PlatformInput) -> DispatchEventResult {
        if let PlatformInput::KeyDown(event) = &input {
            self.modifiers.set(event.keystroke.modifiers);
        } else if let PlatformInput::KeyUp(event) = &input {
            self.modifiers.set(event.keystroke.modifiers);
        }
        let callback = self.callbacks.borrow_mut().input.take();
        if let Some(mut callback) = callback {
            let result = callback(input);
            self.callbacks.borrow_mut().input = Some(callback);
            result
        } else {
            DispatchEventResult::default()
        }
    }

    pub fn touch(&self, id: i32, phase: TouchPhase, x: f32, y: f32) -> bool {
        let position = point(
            px(x / self.display.scale.get()),
            px(y / self.display.scale.get()),
        );
        self.pointer.set(position);
        self.input(PlatformInput::Touch(TouchEvent {
            id: TouchId(id as u64),
            phase,
            position,
            force: None,
        }))
        .default_prevented
    }

    pub fn pinch(&self, phase: TouchPhase, x: f32, y: f32, delta: f32) {
        let scale = self.display.scale.get();
        let position = point(px(x / scale), px(y / scale));
        self.pointer.set(position);
        self.input(PlatformInput::Pinch(PinchEvent {
            position,
            delta,
            phase,
            modifiers: self.modifiers.get(),
        }));
    }

    pub fn long_press(&self, x: f32, y: f32) -> bool {
        let scale = self.display.scale.get();
        let position = point(px(x / scale), px(y / scale));
        self.pointer.set(position);
        self.input(PlatformInput::LongPress(LongPressEvent { position }))
            .default_prevented
    }

    pub fn focus_text_input(&self, x: f32, y: f32) -> bool {
        let position = point(
            px(x / self.display.scale.get()),
            px(y / self.display.scale.get()),
        );
        self.pointer.set(position);
        self.input(PlatformInput::TextInputFocus(TextInputFocusEvent {
            position,
        }))
        .default_prevented
    }

    pub fn tap(&self, x: f32, y: f32) {
        let position = point(
            px(x / self.display.scale.get()),
            px(y / self.display.scale.get()),
        );
        self.pointer.set(position);
        self.input(PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }));
        self.input(PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }));
    }

    pub fn scroll(&self, phase: TouchPhase, x: f32, y: f32, dx: f32, dy: f32) {
        let scale = self.display.scale.get();
        self.input(PlatformInput::ScrollWheel(ScrollWheelEvent {
            position: point(px(x / scale), px(y / scale)),
            delta: ScrollDelta::Pixels(point(px(dx / scale), px(dy / scale))),
            touch_phase: phase,
            ..Default::default()
        }));
    }

    pub fn mouse(&self, event: PlatformInput, hovered: bool, modifiers: Modifiers) {
        self.modifiers.set(modifiers);
        if self.hovered.replace(hovered) != hovered {
            let callback = self.callbacks.borrow_mut().hover.take();
            if let Some(mut callback) = callback {
                callback(hovered);
                self.callbacks.borrow_mut().hover.get_or_insert(callback);
            }
        }
        self.input(event);
    }
}

impl HasWindowHandle for AndroidWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let native = self.native.borrow();
        let raw = native
            .as_ref()
            .ok_or(HandleError::Unavailable)?
            .window_handle()?
            .as_raw();
        // SAFETY: this window owns the native reference; Surface callbacks are
        // serialized on its thread and cannot run during a handle borrow by WGPU.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}
impl HasDisplayHandle for AndroidWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}

pub(crate) struct AndroidWindowHandle(pub Rc<AndroidWindow>);
impl std::ops::Deref for AndroidWindowHandle {
    type Target = AndroidWindow;
    fn deref(&self) -> &AndroidWindow {
        &self.0
    }
}
impl HasWindowHandle for AndroidWindowHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}
impl HasDisplayHandle for AndroidWindowHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}
impl PlatformWindow for AndroidWindowHandle {
    fn bounds(&self) -> Bounds<Pixels> {
        self.display.bounds()
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.display.size.get()
    }
    fn resize(&mut self, _: Size<Pixels>) {}
    fn scale_factor(&self) -> f32 {
        self.display.scale.get()
    }
    fn appearance(&self) -> WindowAppearance {
        self.appearance.get()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }
    fn mouse_position(&self) -> Point<Pixels> {
        self.pointer.get()
    }
    fn modifiers(&self) -> Modifiers {
        self.modifiers.get()
    }
    fn capslock(&self) -> Capslock {
        Capslock::default()
    }
    fn set_input_handler(&mut self, handler: PlatformInputHandler) {
        *self.handler.borrow_mut() = Some(handler);
        self.input_dirty.set(true);
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_dirty.set(true);
        self.handler.borrow_mut().take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {}
    fn is_active(&self) -> bool {
        self.active.get()
    }
    fn is_hovered(&self) -> bool {
        self.hovered.get()
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, _: &str) {}
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {
        let fullscreen = !self.fullscreen.get();
        match self.host.set_fullscreen(fullscreen) {
            Ok(true) => self.fullscreen.set(fullscreen),
            Ok(false) => log::warn!("Android host does not handle fullscreen requests"),
            Err(error) => log::error!("Android fullscreen request failed: {error}"),
        }
    }
    fn is_fullscreen(&self) -> bool {
        self.fullscreen.get()
    }
    fn supports_picture_in_picture(&self) -> bool {
        self.host.supports_picture_in_picture().unwrap_or(false)
    }
    fn is_picture_in_picture(&self) -> bool {
        self.picture_in_picture.get()
    }
    fn enter_picture_in_picture(&self, aspect_ratio: Size<u32>) -> oneshot::Receiver<Result<()>> {
        let (sender, receiver) = oneshot::channel();
        if self.picture_in_picture_request.borrow().is_some() {
            let _ = sender.send(Err(anyhow::anyhow!(
                "picture-in-picture request already pending"
            )));
            return receiver;
        }
        *self.picture_in_picture_request.borrow_mut() = Some(sender);
        self.flush_picture_in_picture_source();
        if let Err(error) = self.host.enter_picture_in_picture(aspect_ratio) {
            self.picture_in_picture_result(Err(error));
        }
        receiver
    }
    fn on_picture_in_picture_changed(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().picture_in_picture = Some(callback);
    }
    fn set_picture_in_picture_source_bounds(&self, bounds: Option<Bounds<Pixels>>) {
        if !self.picture_in_picture.get() {
            self.picture_in_picture_source.set(bounds);
        }
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.callbacks.borrow_mut().frame = Some(callback);
    }

    fn frame_requester(&self) -> Option<Rc<dyn Fn()>> {
        let host = self.host.clone();
        Some(Rc::new(move || host.request_frame()))
    }
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.callbacks.borrow_mut().input = Some(callback);
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().active = Some(callback);
    }
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().hover = Some(callback);
    }
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.callbacks.borrow_mut().resize = Some(callback);
    }
    fn insets(&self) -> WindowInsets {
        self.insets.borrow().clone()
    }
    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.callbacks.borrow_mut().insets = Some(callback);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.callbacks.borrow_mut().close = Some(callback);
    }
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().appearance = Some(callback);
    }
    fn draw(&self, scene: &Scene) {
        let mut renderer = self.renderer.borrow_mut();
        let presented = renderer.draw(scene);
        let needs_redraw = renderer.needs_redraw();
        if !presented || needs_redraw {
            self.force_frame.set(true);
        }
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.borrow().sprite_atlas().clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn supports_backdrop_blur(&self) -> bool {
        self.renderer.borrow().supports_backdrop_blur()
    }
    fn supports_subtree_effects(&self) -> bool {
        true
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        Some(self.renderer.borrow().gpu_specs())
    }
    fn update_ime_position(&self, _: Bounds<Pixels>) {}

    fn perform_haptic_feedback(&self, feedback: HapticFeedback) -> bool {
        if !self.active.get() || self.native.borrow().is_none() {
            return false;
        }
        self.host
            .perform_haptic_feedback(feedback)
            .unwrap_or_else(|error| {
                log::error!("Unable to perform Android haptic feedback: {error:#}");
                false
            })
    }

    fn show_soft_keyboard(&self) {
        if let Err(error) = self.host.set_keyboard_visible(true) {
            log::error!("Unable to show Android keyboard: {error:#}");
        }
    }

    fn hide_soft_keyboard(&self) {
        if let Err(error) = self.host.set_keyboard_visible(false) {
            log::error!("Unable to hide Android keyboard: {error:#}");
        }
    }

    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().back = Some(callback);
        if self.back_enabled.get() {
            if let Err(error) = self.host.set_back_enabled(true) {
                log::error!("Unable to enable Android Back: {error:#}");
            }
        }
    }

    fn set_back_gesture_handler(&self, callback: Box<dyn FnMut(BackGestureEvent)>) {
        self.callbacks.borrow_mut().back_gesture = Some(callback);
    }

    fn set_back_enabled(&self, enabled: bool) {
        if self.back_enabled.replace(enabled) != enabled {
            if let Err(error) = self.host.set_back_enabled(enabled) {
                log::error!("Unable to update Android Back: {error:#}");
            }
        }
    }
}
