#[cfg(feature = "wayland")]
mod clipboard;
mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(feature = "wayland")]
mod text_system;
mod tray;
#[cfg(feature = "wayland")]
mod wayland;

#[cfg(feature = "wayland")]
mod xdg_desktop_portal;

pub use dispatcher::*;
pub use headless::HeadlessWindowFactory;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(feature = "wayland")]
pub(crate) use text_system::*;
pub(crate) use tray::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;

use std::rc::Rc;

#[cfg(feature = "wayland")]
pub type WaylandSurfaceRoleFactory = wayland::ExternalWaylandSurfaceRoleFactory;

/// Returns the default platform implementation for the current OS.
pub fn current_platform(headless: bool) -> Rc<dyn gpui::Platform> {
    if headless {
        return Rc::new(LinuxPlatform {
            inner: HeadlessClient::new(),
        });
    }

    match gpui::guess_compositor() {
        #[cfg(feature = "wayland")]
        "Wayland" => Rc::new(LinuxPlatform {
            inner: WaylandClient::new(),
        }),

        "Headless" => Rc::new(LinuxPlatform {
            inner: HeadlessClient::new(),
        }),
        _ => unreachable!(
            r#"The "wayland" feature must be enabled on gpui_linux or gpui_platform for desktop windows."#
        ),
    }
}

/// Returns a Wayland platform that reuses an existing client connection and
/// assigns a caller-provided role to every window surface it creates.
#[cfg(feature = "wayland")]
pub fn wayland_platform_with_external_surface_role(
    connection: wayland_client::Connection,
    role: WaylandSurfaceRoleFactory,
) -> Rc<dyn gpui::Platform> {
    Rc::new(LinuxPlatform {
        inner: WaylandClient::with_connection_and_external_surface_role(connection, role),
    })
}

/// Returns a headless platform whose windows are supplied by an external host.
pub fn headless_platform_with_window_factory(
    factory: Rc<dyn HeadlessWindowFactory>,
) -> Rc<dyn gpui::Platform> {
    Rc::new(LinuxPlatform {
        inner: HeadlessClient::with_window_factory(Some(factory)),
    })
}
