use gpui_icons_macros::{file_assets, file_enum};

#[file_enum(
    ext = "svg",
    group(
        path = "tests/assets",
        variant_prefix = "Primary",
        asset_prefix = "primary"
    ),
    group(
        path = "tests/assets",
        variant_prefix = "Secondary",
        asset_prefix = "secondary",
        feature = "test-extra-assets"
    )
)]
#[derive(Clone, Copy)]
enum Glyph {}

#[file_assets(
    ext = "svg",
    group(path = "tests/assets", asset_prefix = "primary"),
    group(
        path = "tests/assets",
        asset_prefix = "secondary",
        feature = "test-extra-assets"
    )
)]
struct Resources;

#[test]
fn independent_names_and_assets_share_paths_across_namespaces() {
    assert_eq!(Glyph::ALL.len(), 4);
    assert_eq!(
        Glyph::SecondaryArrowLeft.required_feature(),
        Some("test-extra-assets")
    );
    assert_eq!(Glyph::PrimaryArrowLeft.required_feature(), None);
    assert_eq!(
        Resources::get(Glyph::PrimaryArrowLeft.path())
            .unwrap()
            .unwrap(),
        include_bytes!("assets/arrow-left.svg")
    );
    let secondary = Resources::get(Glyph::SecondaryArrowLeft.path());
    if cfg!(feature = "test-extra-assets") {
        assert_eq!(
            secondary.unwrap().unwrap(),
            include_bytes!("assets/arrow-left.svg")
        );
        assert_eq!(Resources::iter().count(), Glyph::ALL.len());
    } else {
        assert!(secondary.unwrap_err().contains("test-extra-assets"));
        assert!(Resources::iter().all(|path| path.starts_with("primary/")));
    }
    assert!(Resources::get("missing/arrow-left.svg").unwrap().is_none());
    assert!(Resources::get("primary/absent.svg").unwrap().is_none());
}
