use crate::style::{Material, Recipe, State, StylePatch, Styled, Token};
use gpui::{Hsla, Pixels, px};
use std::collections::{BTreeMap, BTreeSet};

/// Component structure and material adaptations using semantic theme tokens.
pub fn default_styles() -> crate::style::ComponentStyles {
    let color = |name| Token::<Hsla>::new(name);
    let length = |name| Token::<Pixels>::new(name);
    let glass = |pressed: bool| Material::Glass {
        blur: length("glass_blur").into(),
        refraction: length("glass_refraction").scale(if pressed { 1. } else { 0.1875 }),
        thickness: length("glass_thickness").scale(if pressed { 1. } else { 0.75 }),
        frost: Token::<f32>::new("glass_frost").scale(0.08),
        tint: color("surface").opacity(0.08),
        frost_center: color("surface").into(),
        frost_edge: color("muted_surface").into(),
    };
    let base = StylePatch::default()
        .h(length("control_height"))
        .text_size(length("font_body"))
        .px(length("spacing_inline"))
        .gap(length("spacing_compact"))
        .rounded(length("radius_control"))
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
    ]);
    for variant in variants.values_mut() {
        *variant = std::mem::take(variant).focus_visible(|s| s.border_color(color("focus")));
    }
    let glass = StylePatch::default()
        .text_color(color("foreground"))
        .h(px(40.))
        .px(px(18.))
        .rounded(px(20.))
        .clear_bg()
        .border_color(gpui::transparent_black())
        .material(glass(false))
        .pressed(|s| s.material(glass(true)))
        .focus_visible(|s| s.border_color(color("focus")));
    let button = Recipe {
        parts: BTreeSet::from(["label".into(), "icon".into()]),
        base,
        glass,
        variants,
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
                s.bg(color("surface_contrast"))
            }),
        variants: BTreeMap::new(),
        glass: StylePatch::default()
            .w(px(64.))
            .h(px(28.))
            .rounded(px(14.))
            .part(crate::style::PartName("thumb".into()), |s| {
                s.material(Material::Glass {
                    blur: length("glass_blur").into(),
                    refraction: length("glass_refraction").scale(1.5),
                    thickness: length("glass_thickness").scale(1.1875),
                    frost: Token::<f32>::new("glass_frost").into(),
                    tint: gpui::transparent_black().into(),
                    frost_center: color("surface_contrast").into(),
                    frost_edge: color("edge_contrast").into(),
                })
            }),
    };

    let slider = Recipe {
        parts: BTreeSet::from(["track".into(), "fill".into(), "thumb".into()]),
        base: StylePatch::default()
            .w(px(280.))
            .h(px(36.))
            .px(px(11.))
            .part(crate::style::PartName("track".into()), |s| {
                s.h(px(6.)).bg(color("edge"))
            })
            .part(crate::style::PartName("fill".into()), |s| {
                s.bg(color("primary"))
                    .state(State::Disabled, |s| s.bg(color("fill_disabled")))
            })
            .part(crate::style::PartName("thumb".into()), |s| {
                s.w(px(22.))
                    .h(px(22.))
                    .bg(color("surface_contrast"))
                    .border(px(2.))
                    .border_color(color("primary"))
                    .state(State::Disabled, |s| s.border_color(color("fill_disabled")))
                    .hover(|s| s.border_color(color("primary_hover")))
                    .focus_visible(|s| s.border(px(3.)).border_color(color("focus")))
            }),
        variants: BTreeMap::new(),
        glass: StylePatch::default().part(crate::style::PartName("thumb".into()), |s| {
            s.w(px(32.))
                .h(px(20.))
                .border(px(0.))
                .material(Material::Glass {
                    blur: length("glass_blur").into(),
                    refraction: length("glass_refraction").scale(1.75),
                    thickness: length("glass_thickness").scale(1.25),
                    frost: Token::<f32>::new("glass_frost").into(),
                    tint: color("surface").opacity(0.04),
                    frost_center: color("surface_contrast").into(),
                    frost_edge: color("edge_contrast").into(),
                })
                .focus_visible(|s| s.border(px(2.)).border_color(color("focus")))
        }),
    };
    crate::style::ComponentStyles {
        recipes: BTreeMap::from([
            ("button".into(), button),
            ("switch".into(), switch),
            ("slider".into(), slider),
        ]),
    }
}
