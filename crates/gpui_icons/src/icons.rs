use gpui::SharedString;

/// Source-prefixed icon names, independent of embedded resource availability.
#[crate::file_enum(
    ext = "svg",
    group(
        path = "assets/lucide/core",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-core"
    ),
    group(
        path = "assets/lucide/common",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-common"
    ),
    group(
        path = "assets/lucide/media",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-media"
    ),
    group(
        path = "assets/lucide/files",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-files"
    ),
    group(
        path = "assets/lucide/editing",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-editing"
    ),
    group(
        path = "assets/lucide/development",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-development"
    ),
    group(
        path = "assets/lucide/connectivity",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-connectivity"
    ),
    group(
        path = "assets/lucide/misc",
        variant_prefix = "Lucide",
        asset_prefix = "icons/lucide",
        feature = "lucide-misc"
    )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuiIcons {}

impl From<GpuiIcons> for SharedString {
    fn from(icon: GpuiIcons) -> Self {
        icon.path().into()
    }
}
