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
    frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    close: Option<Box<dyn FnOnce()>>,
}

pub(crate) struct AndroidWindow {
    // Renderer must be dropped before the last native window reference.
    renderer: RefCell<WgpuRenderer>,
    native: RefCell<Option<NativeWindow>>,
    pub display: Rc<AndroidDisplay>,
    pub active: Cell<bool>,
    force_frame: Cell<bool>,
    pub(crate) pointer: Cell<Point<Pixels>>,
    pub(crate) handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) input_focus: Cell<Option<FocusId>>,
    pub(crate) input_epoch: Cell<u64>,
    pub(crate) input_dirty: Cell<bool>,
    callbacks: RefCell<Callbacks>,
}

impl AndroidWindow {
    pub fn new(
        native: NativeWindow,
        renderer: WgpuRenderer,
        width: i32,
        height: i32,
        density: f32,
    ) -> Self {
        Self {
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
            force_frame: Cell::new(true),
            pointer: Cell::default(),
            handler: RefCell::default(),
            input_focus: Cell::new(None),
            input_epoch: Cell::new(0),
            input_dirty: Cell::new(true),
            callbacks: RefCell::default(),
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
        self.detach();
        let config = WgpuSurfaceConfig {
            size: size(DevicePixels(width), DevicePixels(height)),
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
        self.display.scale.set(density);
        let logical = size(px(width as f32 / density), px(height as f32 / density));
        self.display.size.set(logical);
        self.force_frame.set(true);
        let callback = self.callbacks.borrow_mut().resize.take();
        if let Some(mut callback) = callback {
            callback(logical, density);
            self.callbacks.borrow_mut().resize = Some(callback);
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
        if self.native.borrow().is_none() || !self.active.get() {
            return Ok(());
        }
        anyhow::ensure!(
            !self.renderer.borrow().device_lost(),
            "Android GPU device lost; close and recreate the GpuiSession"
        );
        let callback = self.callbacks.borrow_mut().frame.take();
        if let Some(mut callback) = callback {
            callback(RequestFrameOptions {
                require_presentation: false,
                force_render: self.force_frame.replace(false),
            });
            self.callbacks.borrow_mut().frame = Some(callback);
        }
        Ok(())
    }

    pub fn input(&self, input: PlatformInput) -> DispatchEventResult {
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
        WindowAppearance::Dark
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }
    fn mouse_position(&self) -> Point<Pixels> {
        self.pointer.get()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
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
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, _: &str) {}
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.callbacks.borrow_mut().frame = Some(callback);
    }
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.callbacks.borrow_mut().input = Some(callback);
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().active = Some(callback);
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.callbacks.borrow_mut().resize = Some(callback);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.callbacks.borrow_mut().close = Some(callback);
    }
    fn on_appearance_changed(&self, _: Box<dyn FnMut()>) {}
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
}
