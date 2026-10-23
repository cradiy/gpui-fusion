use gpui_icons_macros::file_enum;

#[file_enum(path = "tests/assets", ext = "svg", rename_all = "PascalCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestIcon {}

#[test]
fn generates_variants_and_display_names_from_files() {
    assert_eq!(TestIcon::ArrowLeft.to_string(), "arrow-left.svg");
    assert_eq!(TestIcon::Volume2.to_string(), "volume-2.svg");
}
