use crate::{
    style::{StyleValue, Token},
    theme::{ModeValues, StyleSet, Theme, TokenDefinition},
};
use gpui::{Hsla, rgb, rgba};
use std::collections::{BTreeMap, BTreeSet};

/// Semantic color palettes with light and dark modes.
pub fn default_theme() -> Theme {
    let palette = [
        ("canvas", 0xf5f6f8, 0x16181c),
        ("muted_surface", 0xf0f0f0, 0x303030),
        ("surface", 0xffffff, 0x25272b),
        ("surface_raised", 0xffffff, 0x2e3137),
        ("surface_disabled", 0xf0f0f0, 0x303030),
        ("surface_sunken", 0xe9ecf1, 0x101216),
        ("surface_inverse", 0x202126, 0xf1f2f4),
        ("surface_contrast", 0xf8f9fb, 0xdfe3e9),
        ("edge_contrast", 0xe6e9ef, 0xc1c8d2),
        ("fill_hover", 0xe9ecf1, 0x343840),
        ("fill_pressed", 0xdce1e8, 0x414650),
        ("fill_disabled", 0xaeb5bf, 0x737e8d),
        ("foreground", 0x202126, 0xf1f2f4),
        ("foreground_secondary", 0x505866, 0xb8c0cc),
        ("foreground_muted", 0x626c7b, 0x99a3b3),
        ("foreground_disabled", 0x929292, 0x777777),
        ("foreground_inverse", 0xf1f2f4, 0x202126),
        ("edge", 0xd9d9d9, 0x424242),
        ("edge_subtle", 0xe7e9ee, 0x343840),
        ("edge_strong", 0x8893a3, 0x8290a4),
        ("primary", 0x1677ff, 0x1668dc),
        ("primary_hover", 0x4096ff, 0x3c89e8),
        ("primary_pressed", 0x0958d9, 0x1554ad),
        ("primary_subtle", 0xeaf3ff, 0x162e4c),
        ("primary_foreground", 0x0958d9, 0x91caff),
        ("primary_edge", 0x91caff, 0x285994),
        ("on_primary", 0xffffff, 0xffffff),
        ("accent", 0x6d28d9, 0x7c3aed),
        ("accent_hover", 0x7c3aed, 0x8b5cf6),
        ("accent_pressed", 0x5b21b6, 0x6d28d9),
        ("accent_subtle", 0xf3edff, 0x302048),
        ("accent_foreground", 0x5b21b6, 0xd8b4fe),
        ("accent_edge", 0xc4b5fd, 0x684696),
        ("on_accent", 0xffffff, 0xffffff),
        ("success", 0x237a40, 0x4ade80),
        ("success_hover", 0x1c6635, 0x86efac),
        ("success_pressed", 0x14532d, 0x22c55e),
        ("success_subtle", 0xeaf8ee, 0x173425),
        ("success_foreground", 0x166534, 0x86efac),
        ("success_edge", 0x86d5a0, 0x2f714b),
        ("on_success", 0xffffff, 0x052e16),
        ("warning", 0xfbbf24, 0xfbbf24),
        ("warning_hover", 0xfcd34d, 0xfcd34d),
        ("warning_pressed", 0xf59e0b, 0xf59e0b),
        ("warning_subtle", 0xfff8df, 0x3b2f16),
        ("warning_foreground", 0x854d0e, 0xfde68a),
        ("warning_edge", 0xe9c65b, 0x856426),
        ("on_warning", 0x302000, 0x302000),
        ("error", 0xb91c1c, 0xf87171),
        ("error_hover", 0x991b1b, 0xfca5a5),
        ("error_pressed", 0x7f1d1d, 0xef4444),
        ("error_subtle", 0xffeeee, 0x3e2025),
        ("error_foreground", 0x991b1b, 0xfca5a5),
        ("error_edge", 0xf1aaaa, 0x8c414b),
        ("on_error", 0xffffff, 0x450a0a),
        ("info", 0x0369a1, 0x38bdf8),
        ("info_hover", 0x075985, 0x7dd3fc),
        ("info_pressed", 0x0c4a6e, 0x0ea5e9),
        ("info_subtle", 0xeaf7ff, 0x163243),
        ("info_foreground", 0x075985, 0x7dd3fc),
        ("info_edge", 0x91cce8, 0x316e8c),
        ("on_info", 0xffffff, 0x082f49),
        ("focus", 0x2866c7, 0x8cb7ff),
    ];
    let mut tokens: BTreeMap<_, _> = palette
        .into_iter()
        .map(|(name, light, dark)| {
            (
                name.into(),
                TokenDefinition::Color(ModeValues::constant(rgb(light)).mode("dark", rgb(dark))),
            )
        })
        .collect();
    for (name, source) in [
        ("divider", "edge_subtle"),
        ("placeholder", "foreground_muted"),
        ("link", "primary_foreground"),
        ("selection", "primary_subtle"),
        ("on_selection", "primary_foreground"),
        ("selection_edge", "primary_edge"),
    ] {
        tokens.insert(
            name.into(),
            TokenDefinition::Color(ModeValues::constant(Token::<Hsla>::new(source))),
        );
    }
    for (name, weight) in [("link_hover", 0.12), ("link_pressed", 0.24)] {
        tokens.insert(
            name.into(),
            TokenDefinition::Color(ModeValues::constant(
                StyleValue::from(Token::<Hsla>::new("link"))
                    .mix(Token::<Hsla>::new("foreground"), weight),
            )),
        );
    }
    for (name, light, dark) in [
        ("scrim", 0x10182866, 0x00000099),
        ("shadow", 0x10182824, 0x00000066),
    ] {
        tokens.insert(
            name.into(),
            TokenDefinition::Color(ModeValues::constant(rgba(light)).mode("dark", rgba(dark))),
        );
    }
    for (name, value) in [
        ("glass_blur", 1.),
        ("glass_refraction", 8.),
        ("glass_thickness", 16.),
        ("control_height", 32.),
        ("font_body", 14.),
        ("radius_control", 6.),
        ("spacing_inline", 15.),
        ("spacing_compact", 8.),
    ] {
        tokens.insert(
            name.into(),
            TokenDefinition::Length(ModeValues::constant(gpui::px(value))),
        );
    }
    tokens.insert(
        "glass_edge_color".into(),
        TokenDefinition::Color(ModeValues::constant(gpui::transparent_black())),
    );
    tokens.insert(
        "glass_edge_light".into(),
        TokenDefinition::Number(ModeValues::constant(1.)),
    );
    tokens.insert(
        "glass_frost".into(),
        TokenDefinition::Number(ModeValues::constant(1.)),
    );
    Theme {
        material: crate::style::MaterialMode::Plain,
        modes: BTreeSet::from(["light".into(), "dark".into()]),
        style_sets: BTreeMap::from([(
            "neutral".into(),
            StyleSet {
                extends: None,
                tokens,
            },
        )]),
    }
}
