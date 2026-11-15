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
        edge_light: Token::<f32>::new("glass_edge_light").into(),
        edge_color: Token::<Hsla>::new("glass_edge_color").into(),
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
                    edge_light: Token::<f32>::new("glass_edge_light").into(),
                    edge_color: Token::<Hsla>::new("glass_edge_color").into(),
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
                    edge_light: Token::<f32>::new("glass_edge_light").into(),
                    edge_color: Token::<Hsla>::new("glass_edge_color").into(),
                    frost: Token::<f32>::new("glass_frost").into(),
                    tint: color("surface").opacity(0.04),
                    frost_center: color("surface_contrast").into(),
                    frost_edge: color("edge_contrast").into(),
                })
                .focus_visible(|s| s.border(px(2.)).border_color(color("focus")))
        }),
    };
    let segmented = Recipe {
        parts: BTreeSet::from(["item".into(), "indicator".into()]),
        base: StylePatch::default()
            .w(px(300.))
            .h(px(40.))
            .p(px(4.))
            .rounded(px(999.))
            .bg(crate::style::StyleValue::from(color("muted_surface"))
                .mix(color("surface_sunken"), 0.35))
            .part(crate::style::PartName("item".into()), |s| {
                s.text_size(length("font_body"))
                    .px(px(10.))
                    .gap(px(6.))
                    .text_color(color("foreground_muted"))
                    .state(State::Selected, |s| s.text_color(color("foreground")))
                    .hover(|s| s.text_color(color("foreground")))
                    .state(State::Disabled, |s| {
                        s.text_color(color("foreground_disabled"))
                    })
            })
            .part(crate::style::PartName("indicator".into()), |s| {
                s.bg(color("surface_raised"))
                    .text_color(color("foreground"))
                    .border(px(0.))
                    .border_color(color("focus"))
                    .focus_visible(|s| s.border(px(2.)))
                    .state(State::Disabled, |s| s.bg(color("fill_disabled")))
            }),
        glass: StylePatch::default().part(crate::style::PartName("indicator".into()), |s| {
            s.material(Material::Glass {
                blur: length("glass_blur").into(),
                refraction: length("glass_refraction").into(),
                thickness: length("glass_thickness").into(),
                edge_light: Token::<f32>::new("glass_edge_light").into(),
                edge_color: Token::<Hsla>::new("glass_edge_color").into(),
                frost: Token::<f32>::new("glass_frost").scale(0.92),
                tint: color("surface").opacity(0.025),
                frost_center: color("surface_raised").into(),
                frost_edge: color("surface").into(),
            })
            .state(State::Disabled, |s| s.material(Material::Plain))
        }),
        variants: BTreeMap::new(),
    };
    let input_glass = |focused: bool| Material::Glass {
        blur: length("glass_blur").scale(14.),
        refraction: length("glass_refraction").scale(0.5),
        thickness: length("glass_thickness").scale(0.5),
        edge_light: Token::<f32>::new("glass_edge_light").scale(if focused { 0.55 } else { 0.4 }),
        edge_color: Token::<Hsla>::new("glass_edge_color").into(),
        frost: Token::<f32>::new("glass_frost").scale(0.55),
        tint: color("surface").opacity(0.08),
        frost_center: color("surface").into(),
        frost_edge: color("muted_surface").into(),
    };
    let input = Recipe {
        parts: BTreeSet::from([
            "placeholder".into(),
            "caret".into(),
            "selection".into(),
            "scrollbar".into(),
            "action_button".into(),
            "resize_handle".into(),
        ]),
        base: StylePatch::default()
            .text_size(length("font_body"))
            .px(px(12.))
            .py(px(8.))
            .gap(px(8.))
            .rounded(length("radius_control"))
            .border(px(1.))
            .bg(color("surface"))
            .text_color(color("foreground"))
            .border_color(color("edge"))
            .hover(|s| s.border_color(color("primary_hover")))
            .focus_visible(|s| {
                s.border_color(color("focus"))
                    .glass_edge_color(color("focus").opacity(0.35))
            })
            .state(State::Disabled, |s| {
                s.bg(color("surface_disabled"))
                    .text_color(color("foreground_disabled"))
                    .border_color(color("edge_subtle"))
            })
            .part(crate::style::PartName("placeholder".into()), |s| {
                s.text_color(color("placeholder"))
                    .state(State::Disabled, |s| {
                        s.text_color(color("foreground_disabled"))
                    })
            })
            .part(crate::style::PartName("action_button".into()), |s| {
                s.text_color(color("foreground_muted"))
                    .text_size(px(18.))
                    .w(px(24.))
                    .h(px(24.))
                    .rounded(px(4.))
                    .border_color(color("focus"))
                    .state(State::Disabled, |s| {
                        s.text_color(color("foreground_disabled"))
                    })
            })
            .part(crate::style::PartName("resize_handle".into()), |s| {
                s.text_color(color("foreground_muted").opacity(0.7))
            })
            .part(crate::style::PartName("caret".into()), |s| {
                s.w(px(1.5)).text_color(color("foreground"))
            })
            .part(crate::style::PartName("selection".into()), |s| {
                s.bg(color("selection"))
            })
            .part(crate::style::PartName("scrollbar".into()), |s| {
                s.bg(color("foreground_muted").opacity(0.5))
                    .h(px(24.))
                    .rounded(px(999.))
                    .border_color(color("focus"))
                    .hover(|s| s.bg(color("foreground_muted")))
                    .pressed(|s| s.bg(color("foreground")))
            }),
        glass: StylePatch::default()
            .rounded(px(10.))
            .material(input_glass(false))
            .focus_visible(|s| s.material(input_glass(true)))
            .state(State::Disabled, |s| s.material(Material::Plain)),
        variants: BTreeMap::from([(
            "invalid".into(),
            StylePatch::default()
                .border_color(color("error"))
                .glass_edge_color(color("error").opacity(0.4))
                .hover(|s| {
                    s.border_color(color("error_hover"))
                        .glass_edge_color(color("error_hover").opacity(0.4))
                })
                .focus_visible(|s| {
                    s.border_color(color("error"))
                        .glass_edge_color(color("error").opacity(0.4))
                }),
        )]),
    };
    crate::style::ComponentStyles {
        recipes: BTreeMap::from([
            ("button".into(), button),
            ("switch".into(), switch),
            ("slider".into(), slider),
            ("segmented".into(), segmented),
            ("input".into(), input),
        ]),
    }
}
