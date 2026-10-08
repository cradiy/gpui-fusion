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
    foreground: ForegroundExecutor,
    pub(crate) host: Arc<Host>,
    text: Arc<CosmicTextSystem>,
    pub(crate) context: GpuContext,
    pub(crate) window: Rc<AndroidWindow>,
    pub(crate) permissions: Rc<crate::permissions::PermissionState>,
    pub(crate) background: Rc<crate::background::BackgroundState>,
    pub(crate) files: Rc<crate::file_dialog::FileDialog>,
    pub(crate) shares: Rc<crate::share::ShareReceiver>,
    pub(crate) services: Rc<crate::system_services::SystemServices>,
    handle: Cell<Option<AnyWindowHandle>>,
    lifecycle: RefCell<Option<Box<dyn FnMut(AppLifecyclePhase)>>>,
    quit: RefCell<Option<Box<dyn FnMut()>>>,
    open_urls: RefCell<Option<Box<dyn FnMut(Vec<String>)>>>,
    pending_urls: RefCell<Vec<String>>,
}

impl AndroidPlatform {
    /// Android foreground execution for application-owned data transfers.
    pub fn background_execution(&self) -> crate::AndroidBackgroundExecution {
        crate::AndroidBackgroundExecution(self.background.clone())
    }
    /// App-private files excluded from Android system backup.
    pub fn no_backup_directory(
        &self,
    ) -> futures::future::LocalBoxFuture<'static, Result<gpui_io::LocationHandle>> {
        let request = crate::file_system::no_backup(&self.host, crate::dispatcher::io_executor());
        Box::pin(async move { request?.await })
    }
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
        let foreground = ForegroundExecutor::new(dispatcher.clone());
        let appearance = host.window_appearance()?;
        let text = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
        text.add_font_files(&host.system_font_paths()?);
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
        let window = Rc::new(AndroidWindow::new(
            host.clone(),
            native,
            renderer,
            width,
            height,
            density,
            appearance,
        ));
        Ok(Rc::new(Self {
            files: crate::file_dialog::FileDialog::new(
                host.clone(),
                BackgroundExecutor::new(dispatcher.clone()),
                foreground.clone(),
            ),
            shares: crate::share::ShareReceiver::new(host.clone(), &foreground),
            services: crate::system_services::SystemServices::new(host.clone()),
            foreground,
            dispatcher,
            permissions: crate::permissions::PermissionState::new(host.clone()),
            background: crate::background::BackgroundState::new(host.clone()),
            host,
            text,
            context,
            window,
            handle: Cell::new(None),
            lifecycle: RefCell::default(),
            quit: RefCell::default(),
            open_urls: RefCell::default(),
            pending_urls: RefCell::default(),
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
        self.open_urls.borrow_mut().take();
        self.pending_urls.borrow_mut().clear();
        self.permissions.close();
        self.services.close();
        self.files.close();
        self.shares.close();
        self.window.detach();
        let callback = self.quit.borrow_mut().take();
        if let Some(mut callback) = callback {
            callback();
        }
        self.dispatcher.close();
    }

    fn read_clipboard(&self) -> Result<Option<ClipboardItem>> {
        Ok(self.host.read_clipboard()?.map(ClipboardItem::new_string))
    }

    pub(crate) fn receive_url(&self, url: String) {
        self.pending_urls.borrow_mut().push(url);
    }

    pub(crate) fn dispatch_open_urls(&self) {
        if self.pending_urls.borrow().is_empty() {
            return;
        }
        let Some(mut callback) = self.open_urls.borrow_mut().take() else {
            return;
        };
        let urls = std::mem::take(&mut *self.pending_urls.borrow_mut());
        callback(urls);
        self.open_urls.borrow_mut().get_or_insert(callback);
    }

    /// Returns the session's main-thread Android permission interface.
    pub fn permissions(&self) -> crate::AndroidPermissions {
        crate::AndroidPermissions(self.permissions.clone())
    }

    fn write_clipboard(&self, item: ClipboardItem) -> Result<()> {
        anyhow::ensure!(!item.entries().is_empty(), "clipboard item is empty");
        let mut text = String::new();
        for entry in item.entries() {
            let ClipboardEntry::String(entry) = entry else {
                bail!("Android clipboard supports text only");
            };
            text.push_str(entry.text());
        }
        self.host.write_clipboard(&text)
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
    fn network_status(&self) -> Result<NetworkStatus> {
        self.services.network_status()
    }

    fn observe_network(&self, callback: Box<dyn FnMut(NetworkStatus)>) -> Result<Subscription> {
        self.services.observe_network(callback)
    }

    fn open_app_settings(&self, page: AppSettings) -> Task<Result<()>> {
        self.services.open_settings(page, &self.foreground)
    }
    fn system_media_session(
        &self,
        options: gpui::gpui_notifications::MediaSessionOptions,
    ) -> futures::future::LocalBoxFuture<
        'static,
        Result<gpui::gpui_notifications::SystemMediaSession>,
    > {
        let host = self.host.clone();
        let background = self.background_execution();
        Box::pin(async move { crate::notifications::create_media(host, background, options) })
    }
    fn notifications(
        &self,
        options: gpui::gpui_notifications::NotificationOptions,
    ) -> futures::future::LocalBoxFuture<
        'static,
        Result<gpui::gpui_notifications::NotificationCenter>,
    > {
        let host = self.host.clone();
        let permissions = self.permissions();
        Box::pin(async move { crate::notifications::create(host, permissions, options) })
    }
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground.clone()
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
        self.window.appearance.get()
    }
    fn open_url(&self, url: &str) {
        if let Err(error) = self.host.open_url(url) {
            log::error!("Failed to open Android URL: {error:#}");
        }
    }
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        *self.open_urls.borrow_mut() = Some(callback);
    }
    fn on_receive_share(&self, callback: Box<dyn FnMut(Result<ReceivedShare>)>) {
        self.shares.set_callback(callback);
    }
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
    fn prompt_for_directory(&self) -> oneshot::Receiver<Result<Option<gpui_io::LocationHandle>>> {
        self.files.prompt_directory()
    }
    fn prompt_for_files(
        &self,
        options: FilePromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<SelectedFile>>>> {
        self.files.prompt(options)
    }
    fn file_system(&self, app_id: &str) -> Result<gpui_io::FileSystem> {
        gpui_io::validate_app_id(app_id)?;
        crate::file_system::file_system(&self.host, crate::dispatcher::io_executor())
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn prompt_for_file_save(
        &self,
        options: FileSaveOptions,
    ) -> oneshot::Receiver<Result<Option<SelectedFile>>> {
        self.files.prompt_save(options)
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn open_file_with_system(&self, file: &SelectedFile) -> Task<Result<()>> {
        let intent = crate::file::view_intent(&self.host, file);
        let file = file.clone();
        let host = self.host.clone();
        self.foreground.spawn(async move {
            let intent = intent.await?;
            let result = host.open_file_intent(&intent);
            drop(file);
            result
        })
    }
    fn share(&self, options: ShareOptions) -> Task<Result<()>> {
        let requests: Vec<_> = options
            .files
            .iter()
            .map(|file| crate::file::view_intent(&self.host, file))
            .collect();
        let host = self.host.clone();
        self.foreground.spawn(async move {
            let mut intents = Vec::with_capacity(requests.len());
            for request in requests {
                intents.push(request.await?);
            }
            let result = host.share(options.text.as_deref(), options.title.as_deref(), &intents);
            drop(options);
            result
        })
    }
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
    fn set_cursor_style(&self, style: CursorStyle) {
        // Android PointerIcon.TYPE_* values, available since API 24.
        let icon = match style {
            CursorStyle::Arrow => 1000,
            CursorStyle::ContextualMenu => 1001,
            CursorStyle::PointingHand => 1002,
            CursorStyle::Crosshair => 1007,
            CursorStyle::IBeam => 1008,
            CursorStyle::IBeamCursorForVerticalLayout => 1009,
            CursorStyle::DragLink => 1010,
            CursorStyle::DragCopy => 1011,
            CursorStyle::OperationNotAllowed => 1012,
            CursorStyle::ResizeLeft
            | CursorStyle::ResizeRight
            | CursorStyle::ResizeLeftRight
            | CursorStyle::ResizeColumn => 1014,
            CursorStyle::ResizeUp
            | CursorStyle::ResizeDown
            | CursorStyle::ResizeUpDown
            | CursorStyle::ResizeRow => 1015,
            CursorStyle::ResizeUpRightDownLeft => 1016,
            CursorStyle::ResizeUpLeftDownRight => 1017,
            CursorStyle::OpenHand => 1020,
            CursorStyle::ClosedHand => 1021,
        };
        if let Err(error) = self.host.set_cursor(icon) {
            log::error!("Failed to set Android pointer icon: {error:#}");
        }
    }
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        self.window.hovered.get()
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        match self.read_clipboard() {
            Ok(item) => item,
            Err(error) => {
                log::error!("Failed to read Android clipboard: {error:#}");
                None
            }
        }
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        if let Err(error) = self.write_clipboard(item) {
            log::error!("Failed to write Android clipboard: {error:#}");
        }
    }
    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>>> {
        Task::ready(self.read_clipboard())
    }
    fn write_to_clipboard_async(&self, item: ClipboardItem) -> Task<Result<()>> {
        Task::ready(self.write_clipboard(item))
    }
    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        crate::credentials::write(
            &self.host,
            self.foreground_executor(),
            url,
            username,
            password,
        )
    }
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        crate::credentials::read(&self.host, self.foreground_executor(), url)
    }
    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        crate::credentials::delete(&self.host, self.foreground_executor(), url)
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
