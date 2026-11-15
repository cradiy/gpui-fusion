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
        ("slider_thumb", 0xf8f9fb, 0xdfe3e9),
        ("slider_thumb_edge", 0xe6e9ef, 0xc1c8d2),
        ("slider_disabled_fill", 0xaeb5bf, 0x737e8d),
        ("foreground", 0x202126, 0xf1f2f4),
        ("edge", 0xd9d9d9, 0x424242),
        ("primary", 0x1677ff, 0x1668dc),
        ("primary_hover", 0x4096ff, 0x3c89e8),
        ("primary_pressed", 0x0958d9, 0x1554ad),
        ("on_primary", 0xffffff, 0xffffff),
        ("focus", 0x2866c7, 0x8cb7ff),
    ];
    let mut theme = Theme {
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
    };
    let switch = Recipe {
        parts: BTreeSet::from(["thumb".into()]),
        base: StylePatch::default()
            .w(px(52.))
            .h(px(28.))
            .rounded(px(14.))
            .bg(color("edge"))
            .border(px(1.))
            .border_color(gpui::transparent_black())
            .state(State::Selected, |s| s.bg(color("primary")))
            .focus_visible(|s| s.border_color(color("focus")))
            .state(State::Disabled, |s| s.opacity(0.4))
            .part(crate::style::PartName("thumb".into()), |s| {
                s.bg(gpui::rgb(0xffffff))
            }),
        variants: BTreeMap::from([(
            "glass".into(),
            StylePatch::default()
                .w(px(64.))
                .h(px(28.))
                .rounded(px(14.))
                .part(crate::style::PartName("thumb".into()), |s| {
                    s.material(Material::Glass {
                        blur: px(1.).into(),
                        refraction: px(12.).into(),
                        thickness: px(19.).into(),
                        frost: 1.0.into(),
                        tint: gpui::transparent_black().into(),
                        frost_center: gpui::rgb(0xffffff).into(),
                        frost_edge: gpui::rgb(0xe9ebef).into(),
                    })
                }),
        )]),
    };
    theme
        .style_sets
        .get_mut("neutral")
        .unwrap()
        .recipes
        .insert("switch".into(), switch);
    let slider = Recipe {
        parts: BTreeSet::from(["track".into(), "fill".into(), "thumb".into()]),
        base: StylePatch::default()
            .w(px(280.))
            .h(px(36.))
            .part(crate::style::PartName("track".into()), |s| {
                s.h(px(6.)).bg(color("edge"))
            })
            .part(crate::style::PartName("fill".into()), |s| {
                s.bg(color("primary"))
                    .state(State::Disabled, |s| s.bg(color("slider_disabled_fill")))
            })
            .part(crate::style::PartName("thumb".into()), |s| {
                s.w(px(22.))
                    .h(px(22.))
                    .bg(color("slider_thumb"))
                    .border(px(2.))
                    .border_color(color("primary"))
                    .state(State::Disabled, |s| {
                        s.border_color(color("slider_disabled_fill"))
                    })
                    .hover(|s| s.border_color(color("primary_hover")))
                    .focus_visible(|s| s.border(px(3.)).border_color(color("focus")))
            }),
        variants: BTreeMap::from([(
            "glass".into(),
            StylePatch::default()
                .h(px(40.))
                .part(crate::style::PartName("track".into()), |s| s.h(px(12.)))
                .part(crate::style::PartName("thumb".into()), |s| {
                    s.w(px(50.))
                        .h(px(26.))
                        .border(px(0.))
                        .material(Material::Glass {
                            blur: px(1.).into(),
                            refraction: px(14.).into(),
                            thickness: px(20.).into(),
                            frost: 1.0.into(),
                            tint: color("surface").opacity(0.04),
                            frost_center: color("slider_thumb").into(),
                            frost_edge: color("slider_thumb_edge").into(),
                        })
                        .focus_visible(|s| s.border(px(2.)).border_color(color("focus")))
                }),
        )]),
    };
    theme
        .style_sets
        .get_mut("neutral")
        .unwrap()
        .recipes
        .insert("slider".into(), slider);
    theme
}
