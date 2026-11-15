use gpui::{Hsla, Pixels, px, rgb};
use gpui_fusion::{
    style::{
        Part, Property, Recipe, Resolved, State, StateSelector, StylePatch, StyleValue, Styled,
        Token, resolve_style,
    },
    theme::{ModeValues, ResolveContext, StyleSet, Theme, ThemeScope, ThemeStore, TokenDefinition},
};
use std::collections::{BTreeMap, BTreeSet};

fn theme() -> Theme {
    Theme {
        material: gpui_fusion::style::MaterialMode::Plain,
        modes: BTreeSet::from(["a".into(), "b".into()]),
        style_sets: BTreeMap::from([(
            "base".into(),
            StyleSet {
                extends: None,
                tokens: BTreeMap::from([
                    (
                        "measure".into(),
                        TokenDefinition::Length(ModeValues::constant(px(7.)).mode("b", px(19.))),
                    ),
                    (
                        "pigment".into(),
                        TokenDefinition::Color(
                            ModeValues::constant(rgb(0x193b71)).mode("b", rgb(0xd8b864)),
                        ),
                    ),
                    (
                        "coefficient".into(),
                        TokenDefinition::Number(ModeValues::constant(0.7)),
                    ),
                ]),
            },
        )]),
    }
}

#[test]
fn serialized_rules_preserve_theme_dependencies_and_fixed_values() {
    let theme = theme();
    let patch = StylePatch::default()
        .w(Token::<Pixels>::new("measure").scale(2.))
        .h(px(31.))
        .bg(Token::<Hsla>::new("pigment").opacity(0.4));
    let source = serde_json::to_string(&patch).unwrap();
    let loaded = StylePatch::from_json(&source).unwrap();
    assert_eq!(patch, loaded);
    let first = resolve_style(
        &[&loaded],
        &ResolveContext::new(&theme, "a", "base").unwrap(),
        &StateSelector::default(),
        None,
    )
    .unwrap();
    let second = resolve_style(
        &[&loaded],
        &ResolveContext::new(&theme, "b", "base").unwrap(),
        &StateSelector::default(),
        None,
    )
    .unwrap();
    assert_eq!(first.width, Resolved::Value(px(14.)));
    assert_eq!(second.width, Resolved::Value(px(38.)));
    assert_eq!(first.height, second.height);
    assert_ne!(first.background, second.background);
    let Resolved::Value(color) = second.background else {
        panic!("missing color")
    };
    assert!((color.a - 0.4).abs() < 0.0001);
}

#[test]
fn instance_base_and_clear_override_lower_layer_states() {
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    let recipe = Recipe {
        base: StylePatch::default()
            .w(px(21.))
            .bg(rgb(0x845129))
            .hover(|s| s.w(px(42.)).bg(rgb(0x294584))),
        ..Default::default()
    };
    let instance = StylePatch::default().w(px(13.)).clear_bg();
    let result = recipe
        .resolve(
            None,
            gpui_fusion::style::MaterialMode::Plain,
            &instance,
            &context,
            &State::Hovered.into(),
            None,
        )
        .unwrap();
    assert_eq!(result.width, Resolved::Value(px(13.)));
    assert_eq!(result.background, Resolved::Clear);
    let inherited = recipe
        .resolve(
            None,
            gpui_fusion::style::MaterialMode::Plain,
            &StylePatch::default(),
            &context,
            &State::Hovered.into(),
            None,
        )
        .unwrap();
    assert_eq!(inherited.width, Resolved::Value(px(42.)));
    assert!(matches!(inherited.background, Resolved::Value(_)));
}

#[derive(Clone, Copy)]
enum SegmentPart {
    Marker,
}
impl Part for SegmentPart {
    fn name(&self) -> &str {
        "marker"
    }
}
#[derive(Default)]
struct Element {
    style: StylePatch,
}
impl Styled for Element {
    type Part = SegmentPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        &mut self.style
    }
}

#[test]
fn component_parts_merge_without_leaking_layout_and_match_combined_states() {
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    let element = Element::default()
        .w(px(200.))
        .part(SegmentPart::Marker, |s| s.w(px(9.)))
        .part(SegmentPart::Marker, |s| {
            s.bg(rgb(0x982375))
                .state(State::Selected & State::Hovered, |s| s.w(px(17.)))
        });
    element.style.validate(&context, &["marker"]).unwrap();
    assert!(element.style.validate(&context, &["caption"]).is_err());
    let marker = resolve_style(
        &[&element.style],
        &context,
        &(State::Selected & State::Hovered),
        Some("marker"),
    )
    .unwrap();
    assert_eq!(marker.width, Resolved::Value(px(17.)));
    assert!(matches!(marker.background, Resolved::Value(_)));
    let idle = resolve_style(
        &[&element.style],
        &context,
        &State::Selected.into(),
        Some("marker"),
    )
    .unwrap();
    assert_eq!(idle.width, Resolved::Value(px(9.)));
    let root = resolve_style(&[&element.style], &context, &StateSelector::default(), None).unwrap();
    assert_eq!(root.width, Resolved::Value(px(200.)));
    assert_eq!(root.background, Resolved::Unset);
}

#[test]
fn inherited_aliases_follow_child_and_scope_but_qualified_tokens_do_not() {
    let mut theme = theme();
    theme.style_sets.get_mut("base").unwrap().tokens.insert(
        "alias".into(),
        TokenDefinition::Length(ModeValues::constant(Token::<Pixels>::new("measure"))),
    );
    theme.style_sets.insert(
        "child".into(),
        StyleSet {
            extends: Some("base".into()),
            tokens: BTreeMap::from([(
                "measure".into(),
                TokenDefinition::Length(ModeValues::constant(px(23.))),
            )]),
        },
    );
    theme.validate().unwrap();
    let scope = ThemeScope {
        style_set: Some("child".into()),
        tokens: BTreeMap::from([(
            "measure".into(),
            TokenDefinition::Length(ModeValues::constant(px(29.))),
        )]),
    };
    let context = ResolveContext::new(&theme, "a", "base")
        .unwrap()
        .scoped(&scope)
        .unwrap();
    assert_eq!(
        StyleValue::from(Token::<Pixels>::new("alias"))
            .resolve(&context)
            .unwrap(),
        px(29.)
    );
    assert_eq!(
        StyleValue::from(Token::<Pixels>::new("alias").in_style_set("base"))
            .resolve(&context)
            .unwrap(),
        px(7.)
    );
}

#[test]
fn reload_is_atomic_and_mode_changes_recompute_without_mutating_rules() {
    let mut store = ThemeStore::new(theme(), "a").unwrap();
    let before = store.theme().clone();
    let mut invalid = before.clone();
    invalid.style_sets.get_mut("base").unwrap().tokens.insert(
        "loop".into(),
        TokenDefinition::Number(ModeValues::constant(Token::<f32>::new("loop"))),
    );
    assert!(
        store
            .replace_json(&serde_json::to_string(&invalid).unwrap())
            .is_err()
    );
    assert_eq!(store.theme(), &before);
    assert_eq!(store.revision(), 0);
    assert!(store.set_mode("b").unwrap());
    assert!(!store.set_mode("b").unwrap());
    assert_eq!(store.revision(), 1);
    assert!(store.set_mode("missing").is_err());
    assert_eq!(store.mode(), "b");
    store
        .replace_json(&serde_json::to_string(&before).unwrap())
        .unwrap();
    assert_eq!(store.revision(), 2);
}

#[test]
fn invalid_references_types_and_inactive_rules_are_rejected() {
    let mut theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    assert!(
        StyleValue::from(Token::<Pixels>::new("pigment"))
            .resolve(&context)
            .unwrap_err()
            .to_string()
            .contains("type mismatch")
    );
    assert!(
        StyleValue::from(Token::<Hsla>::new("absent"))
            .resolve(&context)
            .is_err()
    );
    let patch = StylePatch::default().hover(|s| s.opacity(1.5));
    assert!(patch.validate(&context, &[]).is_err());
    let patch = StylePatch::default().w(px(-1.));
    assert!(patch.validate(&context, &[]).is_err());
    assert!(
        StyleValue::from(Token::<Hsla>::new("pigment"))
            .opacity(f32::NAN)
            .resolve(&context)
            .is_err()
    );
    theme.style_sets.get_mut("base").unwrap().extends = Some("base".into());
    assert!(theme.validate().unwrap_err().message.contains("cycle"));
}

#[test]
fn json_rejects_misspelled_fields_and_roundtrips_theme() {
    let source = serde_json::to_string(&theme()).unwrap();
    assert_eq!(Theme::from_json(&source).unwrap(), theme());
    let mut value = serde_json::to_value(StylePatch::default()).unwrap();
    value["properties"]["widht"] = serde_json::json!({"action": "clear"});
    assert!(StylePatch::from_json(&value.to_string()).is_err());
}

#[test]
fn shorthand_order_and_gpui_translation_preserve_clears() {
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    let mut patch = StylePatch::default().p(px(4.)).pl(px(11.)).rounded(px(6.));
    patch.properties.width = Property::Clear;
    let computed = resolve_style(&[&patch], &context, &StateSelector::default(), None).unwrap();
    let gpui = computed.to_gpui();
    assert_eq!(gpui.size.width, Some(gpui::Length::Auto));
    assert_eq!(gpui.padding.left, Some(px(11.).into()));
    assert_eq!(gpui.padding.right, Some(px(4.).into()));
    assert_eq!(gpui.corner_radii.top_left, Some(px(6.).into()));
    let color_only = StylePatch::default().bg(rgb(0x987461));
    let other = resolve_style(
        &[&patch, &color_only],
        &context,
        &StateSelector::default(),
        None,
    )
    .unwrap();
    assert!(!other.changes_from(&computed).layout);
    assert!(other.changes_from(&computed).paint);
    assert!(computed.changes_from(&Default::default()).layout);
}

#[test]
fn expressions_mix_resolved_inputs_and_reject_invalid_weights() {
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    let value = StyleValue::from(Token::<Pixels>::new("measure")).mix(px(15.), 0.25);
    assert_eq!(value.resolve(&context).unwrap(), px(9.));
    assert!(value.mix(px(1.), f32::INFINITY).resolve(&context).is_err());
}

#[test]
fn a_scope_can_derive_a_token_from_the_same_qualified_theme_token() {
    let theme = theme();
    let scope = ThemeScope {
        tokens: BTreeMap::from([(
            "measure".into(),
            TokenDefinition::Length(ModeValues::constant(
                Token::<Pixels>::new("measure")
                    .in_style_set("base")
                    .scale(3.),
            )),
        )]),
        ..Default::default()
    };
    let context = ResolveContext::new(&theme, "a", "base")
        .unwrap()
        .scoped(&scope)
        .unwrap();
    assert_eq!(
        StyleValue::from(Token::<Pixels>::new("measure"))
            .resolve(&context)
            .unwrap(),
        px(21.)
    );
}

#[test]
fn recipes_and_materials_are_separate_from_inherited_theme_values() {
    use gpui_fusion::style::{ComponentStyles, MaterialMode};
    let mut theme = theme();
    theme.style_sets.insert(
        "child".into(),
        StyleSet {
            extends: Some("base".into()),
            ..Default::default()
        },
    );
    let styles = ComponentStyles {
        recipes: BTreeMap::from([(
            "indicator".into(),
            Recipe {
                base: StylePatch::default().w(Token::<Pixels>::new("measure")),
                glass: StylePatch::default().rounded(px(3.)),
                variants: BTreeMap::from([(
                    "compact".into(),
                    StylePatch::default().h(px(24.)).hover(|s| s.opacity(0.5)),
                )]),
                ..Default::default()
            },
        )]),
    };
    let encoded = serde_json::to_string(&theme).unwrap();
    assert!(!encoded.contains("recipes"));
    let theme = Theme::from_json(&encoded).unwrap();
    let styles = ComponentStyles::from_json(&serde_json::to_string(&styles).unwrap()).unwrap();
    styles.validate(&theme).unwrap();
    let context = ResolveContext::new(&theme, "b", "child").unwrap();
    let instance = StylePatch::default().opacity(0.9);
    let recipe = styles.recipe("indicator").unwrap();
    let result = recipe
        .resolve(
            Some("compact"),
            MaterialMode::Glass,
            &instance,
            &context,
            &State::Hovered.into(),
            None,
        )
        .unwrap();
    assert_eq!(result.width, Resolved::Value(px(19.)));
    assert_eq!(result.height, Resolved::Value(px(24.)));
    assert_eq!(result.radius, Resolved::Value(px(3.)));
    assert_eq!(result.opacity, Resolved::Value(0.9));
    let plain = recipe
        .resolve(
            Some("compact"),
            MaterialMode::Plain,
            &instance,
            &context,
            &StateSelector::default(),
            None,
        )
        .unwrap();
    assert_eq!(plain.radius, Resolved::Unset);
    assert!(
        recipe
            .resolve(
                Some("missing"),
                MaterialMode::Plain,
                &instance,
                &context,
                &StateSelector::default(),
                None
            )
            .is_err()
    );
    assert!(
        recipe
            .resolve(
                None,
                MaterialMode::Glass,
                &instance,
                &context,
                &StateSelector::default(),
                Some("missing")
            )
            .is_err()
    );
}

#[test]
fn partial_mode_overrides_inherit_missing_modes_through_scopes_and_style_sets() {
    let mut theme = theme();
    theme.style_sets.insert(
        "child".into(),
        StyleSet {
            extends: Some("base".into()),
            tokens: BTreeMap::from([(
                "measure".into(),
                TokenDefinition::Length(ModeValues {
                    default: None,
                    modes: BTreeMap::from([("b".into(), px(41.).into())]),
                }),
            )]),
        },
    );
    theme.validate().unwrap();
    let scope = ThemeScope {
        tokens: BTreeMap::from([(
            "measure".into(),
            TokenDefinition::Length(ModeValues {
                default: None,
                modes: BTreeMap::from([("a".into(), px(37.).into())]),
            }),
        )]),
        ..Default::default()
    };
    let value = StyleValue::from(Token::<Pixels>::new("measure"));
    let a = ResolveContext::new(&theme, "a", "child").unwrap();
    assert_eq!(value.resolve(&a).unwrap(), px(7.));
    assert_eq!(value.resolve(&a.scoped(&scope).unwrap()).unwrap(), px(37.));
    let b = ResolveContext::new(&theme, "b", "child")
        .unwrap()
        .scoped(&scope)
        .unwrap();
    assert_eq!(value.resolve(&b).unwrap(), px(41.));
}

#[test]
fn gradients_preserve_tokens_layer_precedence_and_state_motion() {
    use gpui_fusion::style::{BorderMotion, Gradient};
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    let gradient = Gradient::new(
        70.,
        [
            (0., Token::<Hsla>::new("pigment").into()),
            (1., StyleValue::from(rgb(0x315d45))),
        ],
    );
    let patch = StylePatch::default()
        .bg_gradient(gradient.clone())
        .border_gradient(gradient)
        .border(px(2.))
        .pressed(|s| {
            s.border_motion(BorderMotion::Flow {
                seconds_per_cycle: 1.8,
            })
        });
    let loaded = StylePatch::from_json(&serde_json::to_string(&patch).unwrap()).unwrap();
    loaded.validate(&context, &[]).unwrap();
    let resolve =
        |layers: &[&StylePatch], states| resolve_style(layers, &context, &states, None).unwrap();
    let resting = resolve(&[&loaded], StateSelector::default());
    let pressed = resolve(&[&loaded], State::Pressed.into());
    assert!(resting.background_gradient.is_some());
    assert!(resting.border_gradient.is_some());
    assert!(resting.border_motion.is_none());
    assert!(matches!(
        pressed.border_motion,
        Some(BorderMotion::Flow { .. })
    ));
    assert!(!pressed.changes_from(&resting).layout);
    let overridden = resolve(
        &[
            &loaded,
            &StylePatch::default().clear_bg().border_color(rgb(0x123456)),
        ],
        State::Pressed.into(),
    );
    assert!(overridden.background_gradient.is_none());
    assert!(overridden.border_gradient.is_none());
    let other_mode = resolve_style(
        &[&loaded],
        &ResolveContext::new(&theme, "b", "base").unwrap(),
        &StateSelector::default(),
        None,
    )
    .unwrap();
    assert_ne!(resting.background_gradient, other_mode.background_gradient);
    assert_ne!(resting.border_gradient, other_mode.border_gradient);
}

#[test]
fn invalid_gradients_and_inactive_motion_are_rejected_before_paint() {
    use gpui_fusion::style::{BorderMotion, Gradient};
    let theme = theme();
    let context = ResolveContext::new(&theme, "a", "base").unwrap();
    for positions in [vec![0.], vec![0.8, 0.2], vec![-0.1, 1.], vec![0., f32::NAN]] {
        let gradient = Gradient::new(
            0.,
            positions
                .into_iter()
                .map(|p| (p, StyleValue::from(rgb(0x123456)))),
        );
        assert!(
            StylePatch::default()
                .hover(|s| s.bg_gradient(gradient))
                .validate(&context, &[])
                .is_err()
        );
    }
    for duration in [0., -2., f32::INFINITY] {
        assert!(
            StylePatch::default()
                .pressed(|s| s.border_motion(BorderMotion::Flow {
                    seconds_per_cycle: duration
                }))
                .validate(&context, &[])
                .is_err()
        );
    }
}

#[test]
fn glass_edge_color_cascades_without_replacing_body_material() {
    use gpui_fusion::{default_styles, default_theme, style::MaterialMode};
    let theme = default_theme();
    let styles = default_styles();
    let patch = StylePatch::default().glass_edge_color(Token::<Hsla>::new("accent"));
    let patch = StylePatch::from_json(&serde_json::to_string(&patch).unwrap()).unwrap();
    for mode in ["light", "dark"] {
        let context = ResolveContext::new(&theme, mode, "neutral").unwrap();
        let expected = context
            .resolve(&Token::<Hsla>::new("accent").into())
            .unwrap();
        for name in ["input", "button"] {
            let recipe = &styles.recipes[name];
            let resolve = |patch: &StylePatch| {
                recipe
                    .resolve(
                        None,
                        MaterialMode::Glass,
                        patch,
                        &context,
                        &State::FocusVisible.into(),
                        None,
                    )
                    .unwrap()
                    .material
                    .unwrap()
            };
            let original = resolve(&StylePatch::default());
            let changed = resolve(&patch);
            assert_eq!(changed.edge_color, expected);
            assert_eq!(changed.tint, original.tint);
            assert_eq!(changed.frost_center, original.frost_center);
            assert_eq!(changed.frost_edge, original.frost_edge);
            assert_eq!(changed.refraction, original.refraction);
            let mut cleared_patch = StylePatch::default();
            cleared_patch.properties.glass_edge_color = Property::Clear;
            let cleared = resolve(&cleared_patch);
            assert_eq!(cleared.edge_color.a, 0.);
        }
    }
}
