use crate::{
    bridge::Host,
    dispatcher::AndroidDispatcher,
    surface::NativeWindow,
    window::{AndroidWindow, AndroidWindowHandle},
};
use anyhow::{Result, bail};
use futures::channel::oneshot;
use gpui::*;
use gpui_wgpu::{CosmicTextSystem, GpuContext, WgpuRenderer, WgpuSurfaceConfig};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

/// Platform owned by one Android `GpuiSession`. Its window survives View and
/// Surface replacement until the session is explicitly closed.
pub struct AndroidPlatform {
    pub(crate) dispatcher: Arc<AndroidDispatcher>,
    pub(crate) host: Arc<Host>,
    text: Arc<CosmicTextSystem>,
    pub(crate) context: GpuContext,
    pub(crate) window: Rc<AndroidWindow>,
    handle: Cell<Option<AnyWindowHandle>>,
    lifecycle: RefCell<Option<Box<dyn FnMut(AppLifecyclePhase)>>>,
    quit: RefCell<Option<Box<dyn FnMut()>>>,
}

impl AndroidPlatform {
    pub(crate) fn new(
        host: Arc<Host>,
        native: NativeWindow,
        width: i32,
        height: i32,
        density: f32,
    ) -> Result<Rc<Self>> {
        anyhow::ensure!(
            width > 0 && height > 0 && density.is_finite() && density > 0.,
            "invalid Android surface geometry"
        );
        let dispatcher = AndroidDispatcher::new(host.clone());
        let text = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
        text.add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
        ))])?;
        let context = Rc::new(RefCell::new(None));
        let renderer = WgpuRenderer::new(
            context.clone(),
            &native,
            WgpuSurfaceConfig {
                size: size(DevicePixels(width), DevicePixels(height)),
                transparent: false,
                preferred_present_mode: None,
            },
            None,
        )?;
        let window = Rc::new(AndroidWindow::new(native, renderer, width, height, density));
        Ok(Rc::new(Self {
            dispatcher,
            host,
            text,
            context,
            window,
            handle: Cell::new(None),
            lifecycle: RefCell::default(),
            quit: RefCell::default(),
        }))
    }

    pub(crate) fn lifecycle(&self, phase: AppLifecyclePhase) {
        let callback = self.lifecycle.borrow_mut().take();
        if let Some(mut callback) = callback {
            callback(phase);
            *self.lifecycle.borrow_mut() = Some(callback);
        }
    }

    pub(crate) fn close(&self) {
        self.window.detach();
        let callback = self.quit.borrow_mut().take();
        if let Some(mut callback) = callback {
            callback();
        }
        self.dispatcher.close();
    }
}

fn unsupported<T>() -> oneshot::Receiver<Result<T>> {
    let (tx, rx) = oneshot::channel();
    let _ = tx.send(Err(anyhow::anyhow!(
        "Android file dialogs are not implemented"
    )));
    rx
}

impl Platform for AndroidPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, launch: Box<dyn FnOnce()>) {
        launch();
    }
    fn run_app(&self, application: ApplicationHandle, launch: Box<dyn FnOnce()>) {
        crate::bridge::retain_application(application);
        launch();
    }
    fn quit(&self) {
        self.host.request_close();
    }
    fn restart(&self, _: Option<PathBuf>) {}
    fn activate(&self, _: bool) {}
    fn hide(&self) {}
    fn hide_other_apps(&self) {}
    fn unhide_other_apps(&self) {}
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.window.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.window.display.clone())
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.handle.get().filter(|_| self.window.active.get())
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        if matches!(params.kind, WindowKind::AnchoredPopup(_)) {
            return Err(popup::PopupNotSupportedError.into());
        }
        if !matches!(params.kind, WindowKind::Normal) {
            bail!("Android hosts support normal windows only");
        }
        if self.handle.get().is_some() {
            bail!("a GpuiSession hosts one GPUI window");
        }
        self.handle.set(Some(handle));
        Ok(Box::new(AndroidWindowHandle(self.window.clone())))
    }
    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }
    fn open_url(&self, _: &str) {}
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "declare Android URL schemes in the manifest"
        )))
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        unsupported()
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        unsupported()
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        *self.quit.borrow_mut() = Some(callback);
    }
    fn on_reopen(&self, _: Box<dyn FnMut()>) {}
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}
    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        *self.lifecycle.borrow_mut() = Some(callback);
    }
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) {}
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) {}
    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }
    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}
    fn compositor_name(&self) -> &'static str {
        "Android"
    }
    fn app_path(&self) -> Result<PathBuf> {
        bail!("Android applications are packaged libraries")
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        bail!("Android auxiliary executables are unsupported")
    }
    fn set_cursor_style(&self, _: CursorStyle) {}
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        false
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }
    fn write_to_clipboard(&self, _: ClipboardItem) {}
    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>>> {
        Task::ready(Err(anyhow::anyhow!("Android clipboard is not implemented")))
    }
    fn write_to_clipboard_async(&self, _: ClipboardItem) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!("Android clipboard is not implemented")))
    }
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is not implemented"
        )))
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is not implemented"
        )))
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "Android credential storage is not implemented"
        )))
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(AndroidKeyboardLayout)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}

struct AndroidKeyboardLayout;
impl PlatformKeyboardLayout for AndroidKeyboardLayout {
    fn id(&self) -> &str {
        "android"
    }
    fn name(&self) -> &str {
        "Android"
    }
}
