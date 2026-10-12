use crate::native::id;
use gpui::WindowAppearance;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark,
    NSAppearanceNameVibrantLight,
};

pub(crate) unsafe fn window_appearance_from_native(appearance: id) -> WindowAppearance {
    unsafe {
        let appearance = &*appearance.cast::<NSAppearance>();
        let name = appearance.name();
        if *name == *NSAppearanceNameVibrantLight {
            WindowAppearance::VibrantLight
        } else if *name == *NSAppearanceNameVibrantDark {
            WindowAppearance::VibrantDark
        } else if *name == *NSAppearanceNameAqua {
            WindowAppearance::Light
        } else if *name == *NSAppearanceNameDarkAqua {
            WindowAppearance::Dark
        } else {
            log::warn!("unknown appearance: {name}");
            WindowAppearance::Light
        }
    }
}
