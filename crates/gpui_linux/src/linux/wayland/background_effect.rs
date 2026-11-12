use gpui::{Bounds, Pixels};
use wayland_backend::client::ObjectId;
use wayland_client::{
    Proxy, QueueHandle, WEnum,
    protocol::{wl_compositor, wl_surface},
};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1, ext_background_effect_surface_v1,
};
use wayland_protocols_plasma::blur::client::{org_kde_kwin_blur, org_kde_kwin_blur_manager};

use super::WaylandClientStatePtr;

pub(super) struct BackgroundEffectManager {
    pub name: u32,
    pub proxy: ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    pub blur: bool,
}

#[derive(Default)]
pub(super) struct BlurGlobals {
    pub standard: Option<BackgroundEffectManager>,
    pub kde: Option<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager>,
}

impl BlurGlobals {
    pub fn set_capabilities(
        &mut self,
        proxy: &ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
        flags: WEnum<ext_background_effect_manager_v1::Capability>,
    ) -> bool {
        let Some(manager) = self
            .standard
            .as_mut()
            .filter(|manager| manager.proxy == *proxy)
        else {
            return false;
        };
        let bits = match flags {
            WEnum::Value(flags) => flags.bits(),
            WEnum::Unknown(bits) => bits,
        };
        let available = bits & ext_background_effect_manager_v1::Capability::Blur.bits() != 0;
        let changed = manager.blur != available;
        manager.blur = available;
        changed
    }

    fn manager_id(&self) -> Option<ObjectId> {
        match &self.standard {
            Some(manager) => manager.blur.then(|| manager.proxy.id()),
            None => self.kde.as_ref().map(Proxy::id),
        }
    }
}

enum Effect {
    Standard {
        manager: ObjectId,
        effect: ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
    },
    Kde {
        manager: org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
        effect: org_kde_kwin_blur::OrgKdeKwinBlur,
    },
}

impl Effect {
    fn manager_id(&self) -> ObjectId {
        match self {
            Self::Standard { manager, .. } => manager.clone(),
            Self::Kde { manager, .. } => manager.id(),
        }
    }
}

#[derive(Default)]
pub(super) struct SurfaceBlur {
    effect: Option<Effect>,
    region: Option<Vec<Bounds<Pixels>>>,
    applied_bounds: Option<Bounds<Pixels>>,
    dirty: bool,
}

impl SurfaceBlur {
    pub fn set_region(&mut self, region: Option<&[Bounds<Pixels>]>) -> bool {
        if self.region.as_deref() == region {
            return false;
        }
        self.region = region.map(<[_]>::to_vec);
        self.dirty = true;
        true
    }

    pub fn clear(&mut self, surface: &wl_surface::WlSurface) -> bool {
        self.applied_bounds = None;
        match self.effect.take() {
            Some(Effect::Standard { effect, .. }) => effect.destroy(),
            Some(Effect::Kde { manager, effect }) => {
                manager.unset(surface);
                effect.release();
            }
            None => return false,
        }
        true
    }

    /// Stages surface-local state for the next surface commit. Stable frames
    /// do not allocate regions or submit protocol requests.
    pub fn update(
        &mut self,
        surface: &wl_surface::WlSurface,
        compositor: &wl_compositor::WlCompositor,
        qh: &QueueHandle<WaylandClientStatePtr>,
        globals: &BlurGlobals,
        enabled: bool,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let manager = globals.manager_id();
        if !enabled || manager.is_none() || self.region.as_ref().is_some_and(Vec::is_empty) {
            return self.clear(surface);
        }
        if self.effect.as_ref().map(Effect::manager_id) != manager {
            self.clear(surface);
            self.effect = Some(match &globals.standard {
                Some(manager) => Effect::Standard {
                    manager: manager.proxy.id(),
                    effect: manager.proxy.get_background_effect(surface, qh, ()),
                },
                None => {
                    let manager = globals.kde.as_ref().unwrap();
                    Effect::Kde {
                        manager: manager.clone(),
                        effect: manager.create(surface, qh, ()),
                    }
                }
            });
            self.dirty = true;
        }
        if !self.dirty && self.applied_bounds == Some(bounds) {
            return false;
        }

        let region = compositor.create_region(qh, ());
        for rect in self
            .region
            .as_deref()
            .unwrap_or(std::slice::from_ref(&bounds))
        {
            let left = f32::from(rect.origin.x).floor() as i32;
            let top = f32::from(rect.origin.y).floor() as i32;
            let right = f32::from(rect.right()).ceil() as i32;
            let bottom = f32::from(rect.bottom()).ceil() as i32;
            let width = right.saturating_sub(left);
            let height = bottom.saturating_sub(top);
            if rect.size.width > gpui::px(0.)
                && rect.size.height > gpui::px(0.)
                && width > 0
                && height > 0
            {
                region.add(left, top, width, height);
            }
        }
        match self.effect.as_ref().unwrap() {
            Effect::Standard { effect, .. } => effect.set_blur_region(Some(&region)),
            Effect::Kde { effect, .. } => {
                effect.set_region(Some(&region));
                effect.commit();
            }
        }
        region.destroy();
        self.applied_bounds = Some(bounds);
        self.dirty = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};
    use std::{
        io::{self, Read},
        os::unix::net::UnixStream,
    };
    use wayland_client::{Connection, delegate_noop, protocol::wl_registry};

    struct Registry;
    delegate_noop!(Registry: ignore wl_registry::WlRegistry);

    struct Request {
        object: u32,
        opcode: u16,
        args: Vec<u32>,
    }

    // Observe requests encoded by wayland-client without a renderer or desktop.
    struct Wire {
        connection: Connection,
        peer: UnixStream,
        qh: QueueHandle<WaylandClientStatePtr>,
        compositor: wl_compositor::WlCompositor,
        surface: wl_surface::WlSurface,
        globals: BlurGlobals,
        blur: SurfaceBlur,
    }

    impl Wire {
        fn new() -> Self {
            let (socket, peer) = UnixStream::pair().unwrap();
            peer.set_nonblocking(true).unwrap();
            let connection = Connection::from_socket(socket).unwrap();
            let registry_queue = connection.new_event_queue::<Registry>();
            let registry = connection
                .display()
                .get_registry(&registry_queue.handle(), ());
            let queue = connection.new_event_queue::<WaylandClientStatePtr>();
            let qh = queue.handle();
            let compositor: wl_compositor::WlCompositor = registry.bind(37, 4, &qh, ());
            let standard = BackgroundEffectManager {
                name: 43,
                proxy: registry.bind(43, 1, &qh, ()),
                blur: false,
            };
            let kde = registry.bind(51, 1, &qh, ());
            let surface = compositor.create_surface(&qh, ());
            let mut wire = Self {
                connection,
                peer,
                qh,
                compositor,
                surface,
                globals: BlurGlobals {
                    standard: Some(standard),
                    kde: Some(kde),
                },
                blur: SurfaceBlur::default(),
            };
            let proxy = wire.globals.standard.as_ref().unwrap().proxy.clone();
            wire.globals.set_capabilities(&proxy, WEnum::Unknown(1));
            wire.requests();
            wire
        }

        fn update(&mut self, enabled: bool, bounds: Bounds<Pixels>) -> bool {
            self.blur.update(
                &self.surface,
                &self.compositor,
                &self.qh,
                &self.globals,
                enabled,
                bounds,
            )
        }

        fn requests(&mut self) -> Vec<Request> {
            self.connection.flush().unwrap();
            let mut bytes = Vec::new();
            assert_eq!(
                self.peer.read_to_end(&mut bytes).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            let mut requests = Vec::new();
            let mut offset = 0;
            while offset < bytes.len() {
                let object = u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let header = u32::from_ne_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
                let end = offset + (header >> 16) as usize;
                let args = bytes[offset + 8..end]
                    .chunks_exact(4)
                    .map(|word| u32::from_ne_bytes(word.try_into().unwrap()))
                    .collect();
                requests.push(Request {
                    object,
                    opcode: header as u16,
                    args,
                });
                offset = end;
            }
            requests
        }
    }

    #[test]
    fn standard_regions_update_without_recreating_effect_or_polling() {
        let mut wire = Wire::new();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(320.), px(180.)));
        let manager = wire
            .globals
            .standard
            .as_ref()
            .unwrap()
            .proxy
            .id()
            .protocol_id();
        assert!(wire.update(true, bounds));
        let requests = wire.requests();
        let create = requests
            .iter()
            .find(|r| r.object == manager && r.opcode == 1)
            .unwrap();
        let effect = create.args[0];
        let region = requests
            .iter()
            .find(|r| r.object == wire.compositor.id().protocol_id() && r.opcode == 1)
            .unwrap()
            .args[0];
        assert!(
            requests
                .iter()
                .any(|r| r.object == region && r.opcode == 1 && r.args == [0, 0, 320, 180])
        );
        assert!(
            requests
                .iter()
                .any(|r| r.object == effect && r.opcode == 1 && r.args == [region])
        );
        assert!(requests.iter().any(|r| r.object == region && r.opcode == 0));
        // Blur is double-buffered: the caller owns the surface commit.
        assert!(
            !requests
                .iter()
                .any(|r| r.object == wire.surface.id().protocol_id() && r.opcode == 6)
        );
        assert!(!wire.update(true, bounds));
        assert!(wire.requests().is_empty());

        let rects = [
            Bounds::new(point(px(3.25), px(5.75)), size(px(17.5), px(23.5))),
            Bounds::new(point(px(50.), px(60.)), size(px(12.), px(8.))),
        ];
        assert!(wire.blur.set_region(Some(&rects)));
        assert!(wire.update(true, bounds));
        let requests = wire.requests();
        assert!(!requests.iter().any(|r| r.object == manager));
        let adds = requests
            .iter()
            .filter(|r| r.opcode == 1 && r.args.len() == 4)
            .map(|r| r.args.clone())
            .collect::<Vec<_>>();
        assert_eq!(adds, [vec![3, 5, 18, 25], vec![50, 60, 12, 8]]);
        assert!(!wire.blur.set_region(Some(&rects)));
        assert!(!wire.update(true, bounds));
        assert!(wire.requests().is_empty());

        assert!(wire.blur.set_region(Some(&[])));
        assert!(wire.update(true, bounds));
        assert!(
            wire.requests()
                .iter()
                .any(|r| r.object == effect && r.opcode == 0)
        );
        assert!(!wire.update(true, bounds));
        assert!(wire.requests().is_empty());
        assert!(wire.blur.set_region(None));
        assert!(wire.update(true, bounds));
        wire.requests();
        let resized = Bounds::new(bounds.origin, size(px(640.), px(360.)));
        assert!(wire.update(true, resized));
        let requests = wire.requests();
        assert!(!requests.iter().any(|r| r.object == manager));
        assert!(
            requests
                .iter()
                .any(|r| r.opcode == 1 && r.args == [0, 0, 640, 360])
        );
        assert!(wire.update(false, resized));
        wire.requests();
        assert!(wire.update(true, resized));
        assert!(
            wire.requests()
                .iter()
                .any(|r| r.object == manager && r.opcode == 1)
        );
        wire.blur.clear(&wire.surface);
        wire.surface.destroy();
        let requests = wire.requests();
        assert_eq!(
            requests.last().unwrap().object,
            wire.surface.id().protocol_id()
        );
    }

    #[test]
    fn standard_capability_withdrawal_does_not_activate_kde_blur() {
        let mut wire = Wire::new();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(200.), px(100.)));
        let manager = wire
            .globals
            .standard
            .as_ref()
            .unwrap()
            .proxy
            .id()
            .protocol_id();
        let kde = wire.globals.kde.as_ref().unwrap().id().protocol_id();
        let proxy = wire.globals.standard.as_ref().unwrap().proxy.clone();
        wire.globals.set_capabilities(&proxy, WEnum::Unknown(0));
        assert!(!wire.update(true, bounds));
        assert!(wire.requests().is_empty());
        let proxy = wire.globals.standard.as_ref().unwrap().proxy.clone();
        wire.globals.set_capabilities(&proxy, WEnum::Unknown(0x81));
        assert!(wire.update(true, bounds));
        let requests = wire.requests();
        assert!(
            requests
                .iter()
                .any(|r| r.object == manager && r.opcode == 1)
        );
        assert!(!requests.iter().any(|r| r.object == kde));
        let proxy = wire.globals.standard.as_ref().unwrap().proxy.clone();
        wire.globals.set_capabilities(&proxy, WEnum::Unknown(0));
        assert!(wire.update(true, bounds));
        let requests = wire.requests();
        assert!(!requests.iter().any(|r| r.object == kde));
        assert!(!wire.update(true, bounds));
        assert!(wire.requests().is_empty());
        let proxy = wire.globals.standard.as_ref().unwrap().proxy.clone();
        wire.globals.set_capabilities(&proxy, WEnum::Unknown(0x81));
        assert!(wire.update(true, bounds));
        wire.requests();
        wire.globals.standard = None;
        assert!(wire.update(true, bounds));
        let requests = wire.requests();
        assert!(requests.iter().any(|r| r.object == kde));
        assert!(wire.update(false, bounds));
        assert!(
            wire.requests()
                .iter()
                .any(|r| r.object == kde && r.opcode == 1)
        );
    }
}
