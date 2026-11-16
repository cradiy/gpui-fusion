use gpui::{prelude::*, *};
use gpui_fusion::{
    components::button::Button,
    default_theme,
    style::{BorderMotion, Gradient, Material, StylePatch, Styled as FusionStyled, Token},
    theme::ThemeStore,
    theme::runtime::ThemeRuntime,
};

#[derive(Clone, Copy, PartialEq)]
enum MaterialChoice {
    Plain,
    Clear,
    Optical,
    Frosted,
}
#[derive(Clone, Copy, PartialEq)]
enum FillChoice {
    Theme,
    Solid,
    Gradient,
}
#[derive(Clone, Copy, PartialEq)]
enum BorderChoice {
    Theme,
    Solid,
    Dashed,
    Gradient,
    Flow,
}

struct Gallery {
    previews: Vec<Entity<Button>>,
    flat: Vec<Entity<Button>>,
    lens: Entity<Button>,
    theme: Entity<ThemeRuntime>,
    _subscription: Subscription,
    material: MaterialChoice,
    fill: FillChoice,
    border: BorderChoice,
    pill: bool,
    disabled: bool,
    activations: usize,
    palette: usize,
}
fn palette_tokens(palette: usize) -> (&'static str, &'static str) {
    match palette {
        1 => ("jade_start", "jade_end"),
        2 => ("violet_start", "violet_end"),
        _ => ("azure_start", "azure_end"),
    }
}
fn spectrum(alpha: f32, palette: usize, border: bool) -> Gradient {
    let (start, end) = palette_tokens(palette);
    let start = Token::<Hsla>::new(start).opacity(alpha);
    let end = Token::<Hsla>::new(end).opacity(alpha);
    if border {
        Gradient::new(
            0.,
            [
                (0., start.clone()),
                (0.4, start.clone()),
                (0.55, end),
                (0.7, start.clone()),
                (1., start),
            ],
        )
    } else {
        Gradient::new(125., [(0., start), (1., end)])
    }
}
fn glass(material: MaterialChoice, pressed: bool) -> Material {
    let (blur, frost, refraction, thickness) = match material {
        MaterialChoice::Optical => (0.5, 0., if pressed { 9. } else { 7. }, 14.),
        MaterialChoice::Frosted => (10., 0.3, if pressed { 5. } else { 3. }, 12.),
        _ => (1., 0.04, if pressed { 5. } else { 1.5 }, 12.),
    };
    Material::Glass {
        blur: px(blur).into(),
        refraction: px(refraction).into(),
        thickness: px(thickness).into(),
        edge_light: Token::<f32>::new("glass_edge_light").into(),
        reflection: Token::<f32>::new("glass_reflection").into(),
        edge_color: Token::<Hsla>::new("glass_edge_color").into(),
        frost: frost.into(),
        tint: Token::<Hsla>::new("surface").opacity(if material == MaterialChoice::Optical {
            0.
        } else {
            0.04
        }),
        transmission: Token::<f32>::new("glass_transmission").into(),
        frost_center: Token::<Hsla>::new("surface").into(),
        frost_edge: Token::<Hsla>::new("muted_surface").into(),
    }
}
impl Gallery {
    fn patch(&self, primary: bool) -> StylePatch {
        let glass_enabled = self.material != MaterialChoice::Plain;
        let mut patch = StylePatch::default()
            .h(px(40.))
            .px(px(20.))
            .rounded(px(if self.pill { 20. } else { 8. }));
        let (accent, _) = palette_tokens(self.palette);
        let density = if glass_enabled {
            if primary || self.fill != FillChoice::Theme {
                0.48
            } else {
                0.24
            }
        } else {
            1.
        };
        if glass_enabled {
            let material = self.material;
            patch = patch
                .material(glass(material, false))
                .pressed(|s| s.material(glass(material, true)));
        } else {
            patch = patch.material(Material::Plain);
        }
        match self.fill {
            FillChoice::Theme => {
                patch = patch
                    .bg(Token::<Hsla>::new(if primary { accent } else { "surface" })
                        .opacity(density))
                    .text_color(Token::<Hsla>::new(if primary {
                        "on_primary"
                    } else {
                        "foreground"
                    }));
            }
            FillChoice::Solid => {
                patch = patch
                    .bg(Token::<Hsla>::new(accent).opacity(density))
                    .text_color(Token::<Hsla>::new("on_primary"));
            }
            FillChoice::Gradient => {
                patch = patch
                    .bg_gradient(spectrum(density, self.palette, false))
                    .text_color(Token::<Hsla>::new("on_primary"));
            }
        }
        match self.border {
            BorderChoice::Theme => {}
            BorderChoice::Solid => {
                patch = patch
                    .border_solid()
                    .border_color(Token::<Hsla>::new(accent))
            }
            BorderChoice::Dashed => {
                patch = patch
                    .border_dashed()
                    .border_color(Token::<Hsla>::new(accent))
            }
            BorderChoice::Gradient => {
                patch = patch
                    .border(px(2.))
                    .border_solid()
                    .border_gradient(spectrum(1., self.palette, true))
            }
            BorderChoice::Flow => {
                let light = Token::<Hsla>::new("on_primary");
                let accent = Token::<Hsla>::new(accent);
                patch = patch.border(px(2.)).pressed(|s| {
                    s.border_solid()
                        .border_gradient(Gradient::new(
                            0.,
                            [
                                (0., accent.clone().opacity(0.)),
                                (0.70, accent.clone().opacity(0.)),
                                (0.82, accent.clone().opacity(0.35)),
                                (0.90, light.opacity(1.)),
                                (0.94, accent.clone().opacity(0.8)),
                                (1., accent.opacity(0.)),
                            ],
                        ))
                        .border_motion(BorderMotion::Flow {
                            seconds_per_cycle: 1.4,
                        })
                });
            }
        }
        patch
    }
    fn apply(&mut self, cx: &mut Context<Self>) {
        for (index, button) in self.previews.iter().enumerate() {
            let patch = self.patch(index == 0);
            button.update(cx, |button, cx| {
                button.set_style(patch, cx);
                button.set_disabled(self.disabled, cx);
            });
        }
        cx.notify();
    }
    fn choice(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        change: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let dark = self.theme.read(cx).store().mode() == "dark";
        div()
            .id(id)
            .focusable()
            .cursor_pointer()
            .px_3()
            .py_1()
            .rounded(px(8.))
            .text_size(px(13.))
            .border_1()
            .border_color(if selected {
                if dark {
                    rgba(0xffffff20)
                } else {
                    rgba(0x18243618)
                }
            } else {
                rgba(0x00000000)
            })
            .bg(if selected {
                if dark { rgb(0x34373e) } else { rgb(0xffffff) }
            } else if dark {
                rgb(0x22252b)
            } else {
                rgb(0xedeff2)
            })
            .hover(|s| s.bg(if dark { rgb(0x3b3e46) } else { rgb(0xe2e6ec) }))
            .focus_visible(|s| s.border_color(rgb(0x1677ff)))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                change(this, cx);
                this.apply(cx);
            }))
    }
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theme.read(cx).store().mode() == "dark";
        div()
            .id("gallery")
            .overflow_y_scroll()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .bg(if dark { rgb(0x181b23) } else { rgb(0xf7f8fa) })
            .text_color(if dark { rgb(0xf1f2f5) } else { rgb(0x252b36) })
            .child(div().text_size(px(24.)).child("Button studio"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child("Material")
                    .child(self.choice(
                        "plain",
                        "Plain",
                        self.material == MaterialChoice::Plain,
                        |s, _| s.material = MaterialChoice::Plain,
                        cx,
                    ))
                    .child(self.choice(
                        "clear",
                        "Clear glass",
                        self.material == MaterialChoice::Clear,
                        |s, _| s.material = MaterialChoice::Clear,
                        cx,
                    ))
                    .child(self.choice(
                        "optical",
                        "Optical",
                        self.material == MaterialChoice::Optical,
                        |s, _| s.material = MaterialChoice::Optical,
                        cx,
                    ))
                    .child(self.choice(
                        "frost",
                        "Frosted",
                        self.material == MaterialChoice::Frosted,
                        |s, _| s.material = MaterialChoice::Frosted,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child("Fill")
                    .child(self.choice(
                        "theme-fill",
                        "Theme",
                        self.fill == FillChoice::Theme,
                        |s, _| s.fill = FillChoice::Theme,
                        cx,
                    ))
                    .child(self.choice(
                        "solid-fill",
                        "Solid",
                        self.fill == FillChoice::Solid,
                        |s, _| s.fill = FillChoice::Solid,
                        cx,
                    ))
                    .child(self.choice(
                        "gradient-fill",
                        "Gradient",
                        self.fill == FillChoice::Gradient,
                        |s, _| s.fill = FillChoice::Gradient,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child("Palette")
                    .child(self.choice(
                        "azure",
                        "Azure",
                        self.palette == 0,
                        |s, _| s.palette = 0,
                        cx,
                    ))
                    .child(self.choice("jade", "Jade", self.palette == 1, |s, _| s.palette = 1, cx))
                    .child(self.choice(
                        "violet",
                        "Violet",
                        self.palette == 2,
                        |s, _| s.palette = 2,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child("Border")
                    .child(self.choice(
                        "theme-border",
                        "Theme",
                        self.border == BorderChoice::Theme,
                        |s, _| s.border = BorderChoice::Theme,
                        cx,
                    ))
                    .child(self.choice(
                        "solid-border",
                        "Solid",
                        self.border == BorderChoice::Solid,
                        |s, _| s.border = BorderChoice::Solid,
                        cx,
                    ))
                    .child(self.choice(
                        "dashed-border",
                        "Dashed",
                        self.border == BorderChoice::Dashed,
                        |s, _| s.border = BorderChoice::Dashed,
                        cx,
                    ))
                    .child(self.choice(
                        "gradient-border",
                        "Gradient",
                        self.border == BorderChoice::Gradient,
                        |s, _| s.border = BorderChoice::Gradient,
                        cx,
                    ))
                    .child(self.choice(
                        "flow-border",
                        "Press to flow",
                        self.border == BorderChoice::Flow,
                        |s, _| s.border = BorderChoice::Flow,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.choice(
                        "pill",
                        "Pill shape",
                        self.pill,
                        |s, _| s.pill = !s.pill,
                        cx,
                    ))
                    .child(self.choice(
                        "disable",
                        "Disabled",
                        self.disabled,
                        |s, _| s.disabled = !s.disabled,
                        cx,
                    ))
                    .child(self.choice(
                        "mode",
                        "Dark theme",
                        dark,
                        |s, cx| {
                            s.theme.update(cx, |theme, cx| {
                                let mode = if theme.store().mode() == "dark" {
                                    "light"
                                } else {
                                    "dark"
                                };
                                theme.set_mode(mode, cx).unwrap();
                            });
                        },
                        cx,
                    )),
            )
            .child(
                div()
                    .relative()
                    .h(px(200.))
                    .flex_shrink_0()
                    .w_full()
                    .rounded(px(16.))
                    .bg(if dark { rgb(0x263441) } else { rgb(0xd0e1e5) })
                    .child(
                        div()
                            .absolute()
                            .left(px(105.))
                            .top(px(20.))
                            .size(px(150.))
                            .rounded_full()
                            .bg(if dark { rgb(0x654478) } else { rgb(0xc4abd6) }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(295.))
                            .top(px(45.))
                            .w(px(170.))
                            .h(px(120.))
                            .rounded(px(24.))
                            .bg(if dark { rgb(0x28665e) } else { rgb(0x80b9a8) }),
                    )
                    .child(
                        div()
                            .relative()
                            .p_6()
                            .h_full()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_6()
                            .children(self.previews.iter().cloned())
                            .child(self.lens.clone()),
                    ),
            )
            .child(format!(
                "Activations: {} · Hold a preview button to inspect its pressed state",
                self.activations
            ))
            .child(
                div()
                    .text_size(px(13.))
                    .child("Surface-free controls · text and link retain their flat appearance"),
            )
            .child(div().flex().gap_4().children(self.flat.iter().cloned()))
    }
}
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.), px(850.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                let mut theme = default_theme();
                let tokens = &mut theme.style_sets.get_mut("neutral").unwrap().tokens;
                for (name, light, dark) in [
                    ("azure_start", 0x1749dc, 0x2054e2),
                    ("azure_end", 0x007eae, 0x0086b6),
                    ("jade_start", 0x006c50, 0x007459),
                    ("jade_end", 0x00877c, 0x008c82),
                    ("violet_start", 0x6324d6, 0x7233dd),
                    ("violet_end", 0x9650bd, 0x9c56c3),
                ] {
                    tokens.insert(
                        name.into(),
                        gpui_fusion::theme::TokenDefinition::Color(
                            gpui_fusion::theme::ModeValues::constant(rgb(light))
                                .mode("dark", rgb(dark)),
                        ),
                    );
                }
                let theme =
                    ThemeRuntime::install(ThemeStore::new(theme, "light").unwrap(), "neutral", cx)
                        .unwrap();
                cx.new(|cx| {
                    let owner = cx.weak_entity();
                    let mut make = |label, variant: Option<&str>| {
                        let owner = owner.clone();
                        cx.new(|cx| {
                            let button = Button::new(label, window, cx).on_click(move |_, cx| {
                                let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                    view.activations += 1;
                                    cx.notify();
                                });
                            });
                            if let Some(variant) = variant {
                                if variant == "glass" {
                                    button.material_mode(gpui_fusion::style::MaterialMode::Glass)
                                } else {
                                    button.variant(variant)
                                }
                            } else {
                                button
                            }
                        })
                    };
                    let previews = vec![
                        make("Primary", Some("primary")),
                        make("Default", None),
                        make("Dashed", Some("dashed")),
                    ];
                    let flat = vec![make("Text", Some("text")), make("Link", Some("link"))];
                    let lens = make("Optical", Some("glass"));
                    lens.update(cx, |button, cx| {
                        button.set_style(
                            StylePatch::default()
                                .h(px(40.))
                                .px(px(20.))
                                .rounded(px(20.))
                                .bg(Token::<Hsla>::new("surface").opacity(0.08))
                                .material(glass(MaterialChoice::Optical, false))
                                .pressed(|s| s.material(glass(MaterialChoice::Optical, true))),
                            cx,
                        )
                    });
                    let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                    let mut gallery = Gallery {
                        previews,
                        flat,
                        lens,
                        theme,
                        _subscription: subscription,
                        material: MaterialChoice::Plain,
                        fill: FillChoice::Theme,
                        border: BorderChoice::Theme,
                        pill: false,
                        disabled: false,
                        activations: 0,
                        palette: 0,
                    };
                    gallery.apply(cx);
                    gallery
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
