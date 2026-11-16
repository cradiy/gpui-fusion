use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        badge::{Badge, BadgeContent, Badged},
        button::Button,
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    toolbar: Vec<Entity<Button>>,
    count: u64,
    dismissals: usize,
    counts: Vec<Entity<Badge>>,
    states: Vec<Vec<Entity<Badge>>>,
    hosts: Vec<Entity<Button>>,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme.read(cx);
        let context = ResolveContext::new(
            theme.store().theme(),
            theme.store().mode(),
            theme.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        div()
            .id("badges-gallery")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .child(div().text_2xl().child("Badges"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(self.toolbar.iter().cloned()),
            )
            .child(format!(
                "Pending reviews: {} · Dismissed: {} · Theme material: {:?}",
                self.count, self.dismissals,
                theme.material_mode()
            ))
            .children(
                ["Follow theme", "Fixed plain", "Fixed glass"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, title)| {
                        div()
                            .rounded_xl()
                            .p_6()
                            .bg(color("surface"))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(title)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_8()
                                    .child(Badged::new(
                                        self.hosts[index].clone(),
                                        self.counts[index].clone(),
                                    ))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(color("foreground_secondary"))
                                            .child("Counts above 99 display as 99+"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_wrap()
                                    .gap_4()
                                    .children(self.states[index].iter().cloned()),
                            )
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(color("foreground_secondary"))
                    .child(
                        "Drag a count to dismiss · Release nearby to spring back · Restore badges to try again",
                    ),
            )
    }
}
fn main() {
    gpui_platform::application()
        .with_assets(IconAssets)
        .run(|cx| {
            let theme = ThemeRuntime::install(
                ThemeStore::new(default_theme(), "light").unwrap(),
                "neutral",
                cx,
            )
            .unwrap();
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(780.), px(800.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let owner: WeakEntity<Gallery> = cx.weak_entity();
                        let counts = (0..3)
                            .map(|index| {
                                let owner = owner.clone();
                                cx.new(|cx| {
                                    let badge =
                                        Badge::new("Pending reviews", BadgeContent::Count(7), cx)
                                            .variant("error")
                                            .dismissible(true)
                                            .on_dismiss(move |_, cx| {
                                                let _ = owner.update(cx, |gallery, cx| {
                                                    gallery.dismissals += 1;
                                                    cx.notify();
                                                });
                                            });
                                    match index {
                                        1 => badge.material_mode(MaterialMode::Plain),
                                        2 => badge.material_mode(MaterialMode::Glass),
                                        _ => badge,
                                    }
                                })
                            })
                            .collect::<Vec<_>>();
                        let states = (0..3)
                            .map(|index| {
                                let mut badges = [
                                    ("Ready", "success"),
                                    ("Scheduled", "info"),
                                    ("Needs review", "warning"),
                                    ("Expired", "error"),
                                    ("Draft", "neutral"),
                                ]
                                .into_iter()
                                .map(|(label, variant)| {
                                    cx.new(|cx| {
                                        let badge =
                                            Badge::new(label, BadgeContent::Text(label.into()), cx)
                                                .variant(variant);
                                        match index {
                                            1 => badge.material_mode(MaterialMode::Plain),
                                            2 => badge.material_mode(MaterialMode::Glass),
                                            _ => badge,
                                        }
                                    })
                                })
                                .collect::<Vec<_>>();
                                badges.push(cx.new(|cx| {
                                    let badge = Badge::new("Connected", BadgeContent::Dot, cx)
                                        .variant("success");
                                    match index {
                                        1 => badge.material_mode(MaterialMode::Plain),
                                        2 => badge.material_mode(MaterialMode::Glass),
                                        _ => badge,
                                    }
                                }));
                                badges
                            })
                            .collect();
                        let owner: WeakEntity<Gallery> = cx.weak_entity();
                        let hosts = (0..3)
                            .map(|_| {
                                let owner = owner.clone();
                                cx.new(|cx| {
                                    Button::new("Review queue", window, cx).on_click(
                                        move |_, cx| {
                                            let _ = owner.update(cx, |gallery, cx| {
                                                gallery.count = gallery.count.saturating_sub(1);
                                                for badge in &gallery.counts {
                                                    badge.update(cx, |b, cx| {
                                                        b.set_content(
                                                            BadgeContent::Count(gallery.count),
                                                            cx,
                                                        )
                                                    });
                                                }
                                                cx.notify();
                                            });
                                        },
                                    )
                                })
                            })
                            .collect();
                        let toolbar = [
                            "Toggle theme",
                            "Toggle material",
                            "Add reviews",
                            "Clear",
                            "Restore badges",
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            let owner = owner.clone();
                            let theme = theme.clone();
                            cx.new(|cx| {
                                Button::new(label, window, cx)
                                    .material_mode(MaterialMode::Plain)
                                    .on_click(move |window, cx| {
                                        if index < 2 {
                                            theme.update(cx, |t, cx| {
                                                if index == 0 {
                                                    t.set_mode(
                                                        if t.store().mode() == "light" {
                                                            "dark"
                                                        } else {
                                                            "light"
                                                        },
                                                        cx,
                                                    )
                                                    .unwrap();
                                                } else {
                                                    t.set_material_mode(
                                                        if t.material_mode() == MaterialMode::Plain
                                                        {
                                                            MaterialMode::Glass
                                                        } else {
                                                            MaterialMode::Plain
                                                        },
                                                        cx,
                                                    );
                                                }
                                            });
                                        } else {
                                            let _ = owner.update(cx, |gallery, cx| {
                                                if index == 4 {
                                                    for badge in &gallery.counts {
                                                        badge.update(cx, |b, cx| {
                                                            b.reset(window, cx)
                                                        });
                                                    }
                                                    return;
                                                }
                                                gallery.count = if index == 2 {
                                                    gallery.count.saturating_add(25)
                                                } else {
                                                    0
                                                };
                                                for badge in &gallery.counts {
                                                    badge.update(cx, |b, cx| {
                                                        b.set_content(
                                                            BadgeContent::Count(gallery.count),
                                                            cx,
                                                        )
                                                    });
                                                }
                                                cx.notify();
                                            });
                                        }
                                    })
                            })
                        })
                        .collect();
                        Gallery {
                            _subscription: cx.observe(&theme, |_, _, cx| cx.notify()),
                            theme,
                            toolbar,
                            count: 7,
                            dismissals: 0,
                            counts,
                            states,
                            hosts,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
