use crate::{
    style::{Material, Recipe, State, StylePatch, Styled, Token},
    theme::{ModeValues, StyleSet, Theme, TokenDefinition},
};
use gpui::{Hsla, px, rgb};
use std::collections::{BTreeMap, BTreeSet};

/// Neutral component recipes with light and dark modes.
pub fn default_theme() -> Theme {
    let color = |name| Token::<Hsla>::new(name);
    let glass = |pressed: bool, frosted: bool| Material::Glass {
        blur: px(if frosted { 10. } else { 1. }).into(),
        refraction: px(if pressed { 8. } else { 1.5 }).into(),
        thickness: px(if pressed { 16. } else { 12. }).into(),
        frost: (if frosted { 0.7 } else { 0.08 }).into(),
        tint: color("surface").opacity(0.08),
        frost_center: color("surface").into(),
        frost_edge: color("muted_surface").into(),
    };
    let base = StylePatch::default()
        .h(px(32.))
        .text_size(px(14.))
        .px(px(15.))
        .gap(px(8.))
        .rounded(px(6.))
        .border(px(1.))
        .border_color(color("edge"))
        .text_color(color("foreground"))
        .bg(color("surface"))
        .hover(|s| {
            s.border_color(color("primary_hover"))
                .text_color(color("primary_hover"))
        })
        .pressed(|s| {
            s.border_color(color("primary_pressed"))
                .text_color(color("primary_pressed"))
        })
        .focus_visible(|s| s.border_color(color("focus")))
        .state(State::Disabled, |s| s.opacity(0.4));
    let mut variants = BTreeMap::from([
        (
            "primary".into(),
            StylePatch::default()
                .bg(color("primary"))
                .text_color(color("on_primary"))
                .border_color(gpui::transparent_black())
                .hover(|s| s.bg(color("primary_hover")))
                .pressed(|s| s.bg(color("primary_pressed"))),
        ),
        ("dashed".into(), StylePatch::default().border_dashed()),
        (
            "text".into(),
            StylePatch::default()
                .clear_bg()
                .border_color(gpui::transparent_black())
                .text_color(color("foreground"))
                .hover(|s| s.bg(color("foreground").opacity(0.04)))
                .pressed(|s| s.bg(color("foreground").opacity(0.08))),
        ),
        (
            "link".into(),
            StylePatch::default()
                .clear_bg()
                .border_color(gpui::transparent_black())
                .text_color(color("primary"))
                .hover(|s| s.text_color(color("primary_hover")))
                .pressed(|s| s.text_color(color("primary_pressed"))),
        ),
        (
            "glass".into(),
            StylePatch::default()
                .text_color(color("foreground"))
                .h(px(40.))
                .px(px(18.))
                .rounded(px(20.))
                .clear_bg()
                .border_color(gpui::transparent_black())
                .material(glass(false, false))
                .pressed(|s| s.material(glass(true, false))),
        ),
        (
            "frosted".into(),
            StylePatch::default()
                .text_color(color("foreground"))
                .h(px(40.))
                .px(px(18.))
                .rounded(px(20.))
                .clear_bg()
                .border_color(gpui::transparent_black())
                .material(glass(false, true))
                .pressed(|s| s.material(glass(true, true))),
        ),
    ]);
    for variant in variants.values_mut() {
        *variant = std::mem::take(variant).focus_visible(|s| s.border_color(color("focus")));
    }
    let palette = [
        ("muted_surface", 0xf0f0f0, 0x303030),
        ("surface", 0xffffff, 0x25272b),
        ("foreground", 0x202126, 0xf1f2f4),
        ("edge", 0xd9d9d9, 0x424242),
        ("primary", 0x1677ff, 0x1668dc),
        ("primary_hover", 0x4096ff, 0x3c89e8),
        ("primary_pressed", 0x0958d9, 0x1554ad),
        ("on_primary", 0xffffff, 0xffffff),
        ("focus", 0x2866c7, 0x8cb7ff),
    ];
    Theme {
        modes: BTreeSet::from(["light".into(), "dark".into()]),
        style_sets: BTreeMap::from([(
            "neutral".into(),
            StyleSet {
                extends: None,
                tokens: palette
                    .into_iter()
                    .map(|(name, light, dark)| {
                        (
                            name.into(),
                            TokenDefinition::Color(
                                ModeValues::constant(rgb(light)).mode("dark", rgb(dark)),
                            ),
                        )
                    })
                    .collect(),
                recipes: BTreeMap::from([(
                    "button".into(),
                    Recipe {
                        parts: BTreeSet::from(["label".into(), "icon".into()]),
                        base,
                        variants,
                    },
                )]),
            },
        )]),
    }
}
