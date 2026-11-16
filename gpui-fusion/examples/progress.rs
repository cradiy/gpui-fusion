use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        progress::{Progress, ProgressShape},
    },
    default_theme,
    style::{MaterialMode, Styled as FusionStyled, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    bars: Vec<Entity<Progress>>,
    rings: Vec<Entity<Progress>>,
    controls: Vec<Entity<Button>>,
    animated: bool,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let runtime = self.theme.read(cx);
        let context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        div().id("progress-gallery").size_full().overflow_y_scroll().p_8()
            .bg(color("canvas")).text_color(color("foreground")).flex().flex_col().gap_6()
            .child(div().text_2xl().child("Progress"))
            .child(div().flex().flex_wrap().gap_3().children(self.controls.iter().cloned()))
            .child(div().text_color(color("foreground_muted")).child(if runtime.material_mode() == MaterialMode::Glass { "Theme material: Glass" } else { "Theme material: Plain" }))
            .child(div().relative().p_6().rounded_xl().bg(color("surface")).flex().flex_col().gap_5()
                .child(div().absolute().right_0().top_0().bottom_0().w(px(190.)).rounded_xl().bg(color("primary_subtle")))
                .children(self.bars.iter().zip(["Processing", "Verified", "Attention", "Waiting", "Fixed plain", "Fixed glass"]).map(|(bar, label)| {
                    div().relative().flex().items_center().gap_6().child(div().w(px(105.)).text_sm().child(label)).child(bar.clone())
                })))
            .child(div().p_6().rounded_xl().bg(color("surface")).flex().flex_col().gap_5()
                .child("Ring indicators")
                .child(div().flex().flex_wrap().gap_8().children(self.rings.iter().cloned())))
            .child(div().text_sm().text_color(color("foreground_muted")).child("Progress is read-only · Increase advances determinate indicators · Toggle motion pauses loading indicators"))
    }
}
fn main() {
    gpui_platform::application().run(|cx| {
        let theme = ThemeRuntime::install(
            ThemeStore::new(default_theme(), "light").unwrap(),
            "neutral",
            cx,
        )
        .unwrap();
        theme.update(cx, |theme, cx| {
            theme.set_material_mode(MaterialMode::Glass, cx)
        });
        cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(760.), px(620.)),
                    cx,
                ))),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let owner = cx.weak_entity();
                    let bars = (0..6)
                        .map(|index| {
                            cx.new(|cx| {
                                let value = match index {
                                    1 => Some(1.),
                                    2 => Some(0.24),
                                    3 => None,
                                    _ => Some(0.46),
                                };
                                let progress = Progress::new("Task progress", value, cx)
                                    .unwrap()
                                    .show_value(true)
                                    .w(px(330.));
                                match index {
                                    1 => progress.variant("success"),
                                    2 => progress.variant("warning"),
                                    4 => progress.material_mode(MaterialMode::Plain),
                                    5 => progress.material_mode(MaterialMode::Glass),
                                    _ => progress,
                                }
                            })
                        })
                        .collect();
                    let rings = (0..4)
                        .map(|index| {
                            cx.new(|cx| {
                                let value = match index {
                                    1 => Some(1.),
                                    2 => Some(0.24),
                                    3 => None,
                                    _ => Some(0.46),
                                };
                                let progress = Progress::new("Task progress", value, cx)
                                    .unwrap()
                                    .shape(ProgressShape::Ring)
                                    .show_value(true)
                                    .w(px(72.))
                                    .h(px(72.));
                                match index {
                                    1 => progress.variant("success"),
                                    2 => progress.variant("error"),
                                    _ => progress,
                                }
                            })
                        })
                        .collect();
                    let controls = [
                        "Toggle theme",
                        "Toggle material",
                        "Increase",
                        "Toggle motion",
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        let owner = owner.clone();
                        cx.new(|cx| {
                            Button::new(label, window, cx)
                                .material_mode(MaterialMode::Plain)
                                .on_click(move |_, cx| {
                                    let _ = owner.update(cx, |g: &mut Gallery, cx| match index {
                                        0 => g.theme.update(cx, |theme, cx| {
                                            theme
                                                .set_mode(
                                                    if theme.store().mode() == "light" {
                                                        "dark"
                                                    } else {
                                                        "light"
                                                    },
                                                    cx,
                                                )
                                                .unwrap();
                                        }),
                                        1 => g.theme.update(cx, |theme, cx| {
                                            theme.set_material_mode(
                                                if theme.material_mode() == MaterialMode::Plain {
                                                    MaterialMode::Glass
                                                } else {
                                                    MaterialMode::Plain
                                                },
                                                cx,
                                            )
                                        }),
                                        2 => {
                                            for progress in g.bars.iter().chain(g.rings.iter()) {
                                                progress.update(cx, |p, cx| {
                                                    if let Some(value) = p.value() {
                                                        p.set_value(
                                                            Some(if value >= 1. {
                                                                0.
                                                            } else {
                                                                value + 0.1
                                                            }),
                                                            cx,
                                                        )
                                                        .unwrap();
                                                    }
                                                });
                                            }
                                        }
                                        _ => {
                                            g.animated = !g.animated;
                                            for progress in g.bars.iter().chain(g.rings.iter()) {
                                                progress.update(cx, |p, cx| {
                                                    p.set_animated(g.animated, cx)
                                                });
                                            }
                                        }
                                    });
                                })
                        })
                    })
                    .collect();
                    let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                    Gallery {
                        theme,
                        bars,
                        rings,
                        controls,
                        animated: true,
                        _subscription: subscription,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
