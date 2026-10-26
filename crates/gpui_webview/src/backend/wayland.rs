use anyhow::{Result, bail};
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandWindowHandle, WindowHandle,
};
use std::ptr::NonNull;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_compositor, wl_region, wl_registry, wl_subcompositor, wl_subsurface, wl_surface,
    },
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

pub struct ChildSurface {
    viewport: wp_viewport::WpViewport,
    subsurface: wl_subsurface::WlSubsurface,
    surface: wl_surface::WlSurface,
    queue: EventQueue<Events>,
    connection: Connection,
    display: RawDisplayHandle,
}
pub struct Events;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Events {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(Events: ignore wl_compositor::WlCompositor);
delegate_noop!(Events: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(Events: ignore wl_surface::WlSurface);
delegate_noop!(Events: ignore wl_subsurface::WlSubsurface);
delegate_noop!(Events: ignore wl_region::WlRegion);
delegate_noop!(Events: ignore wp_viewporter::WpViewporter);
delegate_noop!(Events: ignore wp_viewport::WpViewport);
impl ChildSurface {
    pub fn new(window: &gpui::Window) -> Result<Self> {
        let display = window.display_handle()?.as_raw();
        let RawDisplayHandle::Wayland(display_handle) = display else {
            bail!("Wayland display required")
        };
        let RawWindowHandle::Wayland(parent_handle) =
            HasWindowHandle::window_handle(window)?.as_raw()
        else {
            bail!("Wayland window required")
        };
        // The view releases this child in the host window's close observer.
        let backend = unsafe {
            wayland_backend::client::Backend::from_foreign_display(
                display_handle.display.as_ptr().cast(),
            )
        };
        let connection = Connection::from_backend(backend);
        let (globals, queue) = registry_queue_init::<Events>(&connection)?;
        let qh = queue.handle();
        let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 1..=5, ())?;
        let subcompositor: wl_subcompositor::WlSubcompositor = globals.bind(&qh, 1..=1, ())?;
        let viewporter: wp_viewporter::WpViewporter = globals.bind(&qh, 1..=1, ())?;
        let parent_id = unsafe {
            wayland_backend::client::ObjectId::from_ptr(
                wl_surface::WlSurface::interface(),
                parent_handle.surface.as_ptr().cast(),
            )?
        };
        let parent = wl_surface::WlSurface::from_id(&connection, parent_id)?;
        let surface = compositor.create_surface(&qh, ());
        let subsurface = subcompositor.get_subsurface(&surface, &parent, &qh, ());
        subsurface.set_desync();
        let region = compositor.create_region(&qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        let viewport = viewporter.get_viewport(&surface, &qh, ());
        connection.flush()?;
        Ok(Self {
            viewport,
            subsurface,
            surface,
            queue,
            connection,
            display,
        })
    }
    pub fn place(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.subsurface.set_position(x, y);
        self.viewport.set_destination(width.max(1), height.max(1));
        self.surface.commit();
        let _ = self.queue.dispatch_pending(&mut Events);
        let _ = self.connection.flush();
    }
}
impl HasDisplayHandle for ChildSurface {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe { DisplayHandle::borrow_raw(self.display) })
    }
}
impl HasWindowHandle for ChildSurface {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let pointer = NonNull::new(self.surface.id().as_ptr().cast()).unwrap();
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer)))
        })
    }
}
impl Drop for ChildSurface {
    fn drop(&mut self) {
        self.viewport.destroy();
        self.subsurface.destroy();
        self.surface.destroy();
        let _ = self.connection.flush();
    }
}
