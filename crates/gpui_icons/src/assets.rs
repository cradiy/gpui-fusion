#[crate::file_assets(
    ext = "svg",
    group(
        path = "assets/lucide/core",
        asset_prefix = "icons/lucide",
        feature = "lucide-core"
    ),
    group(
        path = "assets/lucide/common",
        asset_prefix = "icons/lucide",
        feature = "lucide-common"
    ),
    group(
        path = "assets/lucide/media",
        asset_prefix = "icons/lucide",
        feature = "lucide-media"
    ),
    group(
        path = "assets/lucide/files",
        asset_prefix = "icons/lucide",
        feature = "lucide-files"
    ),
    group(
        path = "assets/lucide/editing",
        asset_prefix = "icons/lucide",
        feature = "lucide-editing"
    ),
    group(
        path = "assets/lucide/development",
        asset_prefix = "icons/lucide",
        feature = "lucide-development"
    ),
    group(
        path = "assets/lucide/connectivity",
        asset_prefix = "icons/lucide",
        feature = "lucide-connectivity"
    ),
    group(
        path = "assets/lucide/misc",
        asset_prefix = "icons/lucide",
        feature = "lucide-misc"
    )
)]
pub(crate) struct EmbeddedIcons;
