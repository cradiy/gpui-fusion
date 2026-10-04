pub use gpui::layer_shell::*;

use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

pub(crate) fn wayland_layer(layer: Layer) -> zwlr_layer_shell_v1::Layer {
    match layer {
        Layer::Background => zwlr_layer_shell_v1::Layer::Background,
        Layer::Bottom => zwlr_layer_shell_v1::Layer::Bottom,
        Layer::Top => zwlr_layer_shell_v1::Layer::Top,
        Layer::Overlay => zwlr_layer_shell_v1::Layer::Overlay,
    }
}

pub(crate) fn wayland_anchor(anchor: Anchor) -> zwlr_layer_surface_v1::Anchor {
    zwlr_layer_surface_v1::Anchor::from_bits_truncate(anchor.bits())
}

pub(crate) fn wayland_keyboard_interactivity(
    value: KeyboardInteractivity,
    version: u32,
) -> zwlr_layer_surface_v1::KeyboardInteractivity {
    match value {
        KeyboardInteractivity::None => zwlr_layer_surface_v1::KeyboardInteractivity::None,
        KeyboardInteractivity::Exclusive => zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
        KeyboardInteractivity::OnDemand if version >= 4 => {
            zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand
        }
        KeyboardInteractivity::OnDemand => {
            log::warn!(
                "layer-shell v{version} does not support OnDemand keyboard interactivity; using None"
            );
            zwlr_layer_surface_v1::KeyboardInteractivity::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_modes_respect_layer_surface_version() {
        use zwlr_layer_surface_v1::KeyboardInteractivity as WireMode;

        for version in 1..=5 {
            assert_eq!(
                wayland_keyboard_interactivity(KeyboardInteractivity::None, version),
                WireMode::None
            );
            assert_eq!(
                wayland_keyboard_interactivity(KeyboardInteractivity::Exclusive, version),
                WireMode::Exclusive
            );
            assert_eq!(
                wayland_keyboard_interactivity(KeyboardInteractivity::OnDemand, version),
                if version < 4 {
                    WireMode::None
                } else {
                    WireMode::OnDemand
                },
            );
        }
    }
}
