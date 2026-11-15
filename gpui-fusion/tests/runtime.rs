use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    components::button::Button,
    components::slider::Slider,
    components::switch::Switch,
    default_theme,
    style::Styled as FusionStyled,
    theme::runtime::ThemeRuntime,
    theme::{Theme, ThemeScope, ThemeStore},
};

struct Harness {
    button: Entity<Button>,
    switch: Entity<Switch>,
    slider: Entity<Slider>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_4().children([
            self.button.clone().into_any_element(),
            self.switch.clone().into_any_element(),
            self.slider.clone().into_any_element(),
        ])
    }
}
fn sized_theme(base_width: f32, compact_width: f32) -> Theme {
    let mut theme = default_theme();
    let mut compact = theme.style_sets["neutral"].clone();
    compact.tokens.insert(
        "measure".into(),
        gpui_fusion::theme::TokenDefinition::Length(gpui_fusion::theme::ModeValues::constant(px(
            compact_width,
        ))),
    );
    theme.style_sets.get_mut("neutral").unwrap().tokens.insert(
        "measure".into(),
        gpui_fusion::theme::TokenDefinition::Length(gpui_fusion::theme::ModeValues::constant(px(
            base_width,
        ))),
    );
    theme.style_sets.insert("compact".into(), compact);
    theme
}

#[gpui::test]
fn injected_theme_replacement_refreshes_existing_controls_and_preserves_scopes(
    cx: &mut TestAppContext,
) {
    let view = cx.open_window(size(px(460.), px(260.)), |window, cx| {
        let mut styles = gpui_fusion::default_styles();
        for recipe in styles.recipes.values_mut() {
            recipe.base = std::mem::take(&mut recipe.base)
                .w(gpui_fusion::style::Token::<gpui::Pixels>::new("measure"));
        }
        ThemeRuntime::install_with_styles(
            ThemeStore::new(sized_theme(126., 184.), "light").unwrap(),
            "neutral",
            styles,
            cx,
        )
        .unwrap();
        let button = cx.new(|cx| {
            Button::new("Apply", window, cx).scope(ThemeScope {
                style_set: Some("neutral".into()),
                ..Default::default()
            })
        });
        let switch = cx.new(|cx| Switch::new("Updates", true, window, cx).animated(false));
        let slider = cx.new(|cx| Slider::new("Level", 47., window, cx).animated(false));
        Harness {
            button,
            switch,
            slider,
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    for selector in ["fusion-button", "fusion-switch", "fusion-slider"] {
        assert_eq!(visual.debug_bounds(selector).unwrap().size.width, px(126.));
    }
    view.update(&mut visual.cx, |h, window, cx| {
        h.slider.read(cx).focus_handle(cx).focus(window, cx);
        ThemeRuntime::global(cx)
            .update(cx, |theme, cx| theme.set_style_set("compact", cx).unwrap());
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-button").unwrap().size.width,
        px(126.)
    );
    for selector in ["fusion-switch", "fusion-slider"] {
        assert_eq!(visual.debug_bounds(selector).unwrap().size.width, px(184.));
    }
    view.update(&mut visual.cx, |_, _, cx| {
        let entity = ThemeRuntime::global(cx);
        let revision = entity.read(cx).revision();
        let installed = ThemeRuntime::install(
            ThemeStore::new(sized_theme(152., 238.), "dark").unwrap(),
            "compact",
            cx,
        )
        .unwrap();
        assert_eq!(entity.entity_id(), installed.entity_id());
        assert!(installed.read(cx).revision() > revision);
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-button").unwrap().size.width,
        px(152.)
    );
    for selector in ["fusion-switch", "fusion-slider"] {
        assert_eq!(visual.debug_bounds(selector).unwrap().size.width, px(238.));
    }
    view.update(&mut visual.cx, |h, window, cx| {
        assert!(h.switch.read(cx).checked());
        assert_eq!(h.slider.read(cx).value(), 47.);
        assert!(h.slider.read(cx).focus_handle(cx).is_focused(window));
        let theme = ThemeRuntime::global(cx);
        let revision = theme.read(cx).revision();
        assert!(
            ThemeRuntime::install(
                ThemeStore::new(default_theme(), "light").unwrap(),
                "absent",
                cx
            )
            .is_err()
        );
        theme.update(cx, |theme, cx| {
            assert!(theme.set_style_set("absent", cx).is_err());
            assert!(
                theme
                    .replace_json(&serde_json::to_string(&default_theme()).unwrap(), cx)
                    .is_err()
            );
            assert_eq!(theme.revision(), revision);
            assert_eq!(theme.style_set(), "compact");
            assert_eq!(theme.store().mode(), "dark");
        });
    })
    .unwrap();
}

#[gpui::test]
fn file_theme_updates_material_and_color_without_refreshing_fixed_materials(
    cx: &mut TestAppContext,
) {
    use gpui_fusion::{style::MaterialMode, theme::ThemeConfig};
    use std::{cell::Cell, rc::Rc, time::Duration};
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut config = ThemeConfig {
        theme: default_theme(),
        mode: "light".into(),
        style_set: "neutral".into(),
    };
    std::fs::write(file.path(), serde_json::to_string(&config).unwrap()).unwrap();
    let view = cx.open_window(size(px(460.), px(260.)), |window, cx| {
        ThemeRuntime::install_file(file.path(), cx).unwrap();
        Harness {
            button: cx
                .new(|cx| Button::new("Apply", window, cx).transition_duration(Duration::ZERO)),
            switch: cx.new(|cx| {
                Switch::new("Updates", true, window, cx)
                    .material_mode(MaterialMode::Plain)
                    .animated(false)
            }),
            slider: cx.new(|cx| {
                Slider::new("Level", 47., window, cx)
                    .material_mode(MaterialMode::Glass)
                    .animated(false)
            }),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    let button_height = visual.debug_bounds("fusion-button").unwrap().size.height;
    let switch_width = visual.debug_bounds("fusion-switch").unwrap().size.width;
    let counts = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0)]);
    let entities = view
        .update(&mut visual.cx, |h, _, _| {
            (h.button.clone(), h.switch.clone(), h.slider.clone())
        })
        .unwrap();
    let _subscriptions = visual.update(|_, cx| {
        let b = counts.clone();
        let s = counts.clone();
        let l = counts.clone();
        vec![
            cx.observe(&entities.0, move |_, _| b[0].set(b[0].get() + 1)),
            cx.observe(&entities.1, move |_, _| s[1].set(s[1].get() + 1)),
            cx.observe(&entities.2, move |_, _| l[2].set(l[2].get() + 1)),
        ]
    });
    config.theme.material = MaterialMode::Glass;
    std::fs::write(file.path(), serde_json::to_string(&config).unwrap()).unwrap();
    visual.update(|_, cx| {
        ThemeRuntime::global(cx).update(cx, |theme, cx| theme.reload_file(file.path(), cx).unwrap())
    });
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(counts[0].get() > 0);
    assert_eq!(counts[1].get(), 0);
    assert_eq!(counts[2].get(), 0);
    assert!(visual.debug_bounds("fusion-button").unwrap().size.height > button_height);
    assert_eq!(
        visual.debug_bounds("fusion-switch").unwrap().size.width,
        switch_width
    );
    for count in counts.iter() {
        count.set(0);
    }
    config.mode = "dark".into();
    std::fs::write(file.path(), serde_json::to_string(&config).unwrap()).unwrap();
    visual.update(|_, cx| {
        ThemeRuntime::global(cx).update(cx, |theme, cx| theme.reload_file(file.path(), cx).unwrap())
    });
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(counts.iter().all(|count| count.get() > 0));
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.switch.read(cx).checked());
        assert_eq!(h.slider.read(cx).value(), 47.);
        h.switch
            .update(cx, |control, cx| control.set_material_mode(None, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-switch").unwrap().size.width > switch_width);
    std::fs::write(file.path(), "{").unwrap();
    visual.update(|_, cx| {
        ThemeRuntime::global(cx).update(cx, |theme, cx| {
            let before = theme.configuration();
            assert!(theme.reload_file(file.path(), cx).is_err());
            assert_eq!(theme.configuration(), before);
        })
    });
    let mut invalid = config.clone();
    invalid.mode = "absent".into();
    std::fs::write(file.path(), serde_json::to_string(&invalid).unwrap()).unwrap();
    visual.update(|_, cx| {
        ThemeRuntime::global(cx).update(cx, |theme, cx| {
            assert!(theme.reload_file(file.path(), cx).is_err());
            assert_eq!(theme.configuration(), config);
            let mut invalid_styles = gpui_fusion::default_styles();
            invalid_styles.recipes.get_mut("button").unwrap().base =
                gpui_fusion::style::StylePatch::default()
                    .w(gpui_fusion::style::Token::<gpui::Pixels>::new("absent"));
            let before = theme.styles().clone();
            assert!(
                theme
                    .replace_styles_json(&serde_json::to_string(&invalid_styles).unwrap(), cx)
                    .is_err()
            );
            assert_eq!(theme.styles(), &before);
        })
    });
}
