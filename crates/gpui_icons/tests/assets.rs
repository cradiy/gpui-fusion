use gpui::AssetSource;
use gpui_icons::{GpuiIcons, IconAssets};
use std::{collections::BTreeSet, fs, path::Path};

#[test]
fn bundled_inventory_loads_exact_svg_sources() {
    let assets = IconAssets::new();
    let listed = assets.list("icons/lucide/").unwrap();
    let unique: BTreeSet<_> = listed.iter().map(|path| path.as_ref()).collect();
    assert_eq!(listed.len(), unique.len());
    for icon in GpuiIcons::ALL {
        let (provider, filename) = icon
            .path()
            .strip_prefix("icons/")
            .unwrap()
            .split_once('/')
            .unwrap();
        let category = icon
            .required_feature()
            .unwrap()
            .strip_prefix(&format!("{provider}-"))
            .unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(provider)
            .join(category)
            .join(filename);
        let original = fs::read(source).unwrap();
        match assets.load(icon.path()) {
            Ok(Some(bytes)) => {
                assert!(unique.contains(icon.path()));
                assert_eq!(bytes.as_ref(), original, "{}", icon.path());
            }
            Err(error) => {
                assert!(!unique.contains(icon.path()));
                assert!(error.to_string().contains(icon.required_feature().unwrap()));
                assert!(error.to_string().contains(icon.path()));
            }
            Ok(None) => panic!("known icon missing from asset routing: {}", icon.path()),
        }
    }
    #[cfg(feature = "all")]
    assert_eq!(listed.len(), GpuiIcons::ALL.len());
    #[cfg(feature = "lucide-core")]
    assert!(
        assets
            .load(GpuiIcons::LucideChevronDown.path())
            .unwrap()
            .is_some()
    );
    #[cfg(not(feature = "lucide-core"))]
    assert!(assets.load(GpuiIcons::LucideChevronDown.path()).is_err());
    #[cfg(not(feature = "lucide-common"))]
    assert!(assets.load(GpuiIcons::LucideMoon.path()).is_err());
}

#[test]
fn unrelated_assets_can_be_delegated_to_another_source() {
    let assets = IconAssets::new();
    for path in [
        "",
        "images/banner.png",
        "icons/other/ornament.svg",
        "icons/lucide/not-an-icon.svg",
    ] {
        assert!(assets.load(path).unwrap().is_none());
    }
    assert!(assets.list("images/").unwrap().is_empty());
}
