use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        radio::{RadioGroup, RadioOption, RadioPart},
    },
    default_theme,
    style::{MaterialMode, State, Styled as FusionStyled, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    groups: Vec<Entity<RadioGroup>>,
    theme_button: Entity<Button>,
    material_button: Entity<Button>,
    status: SharedString,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let runtime = self.theme.read(cx);
        let store = runtime.store();
        let context =
            ResolveContext::new(store.theme(), store.mode(), runtime.style_set()).unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        let titles = [
            "Archive retention",
            "Verification · horizontal",
            "Compression · fixed plain",
            "Notification · fixed glass",
            "Custom indicator and dot",
            "Managed setting",
        ];
        div()
            .id("radio-gallery")
            .on_key_down(|event, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                {
                    if modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            })
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_2xl().child("Radio groups"))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(self.theme_button.clone())
                    .child(self.material_button.clone()),
            )
            .child(div().text_color(color("foreground_secondary")).child(
                if runtime.material_mode() == MaterialMode::Glass {
                    "Theme material: Glass"
                } else {
                    "Theme material: Plain"
                },
            ))
            .children(
                self.groups
                    .iter()
                    .zip(titles)
                    .enumerate()
                    .map(|(index, (group, title))| {
                        div()
                            .relative()
                            .p_5()
                            .rounded_xl()
                            .bg(color("surface"))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .when(index == 0 || index == 3, |s| {
                                s.child(
                                    div()
                                        .absolute()
                                        .left(px(45.))
                                        .top_0()
                                        .bottom_0()
                                        .w(px(110.))
                                        .bg(color("primary_subtle")),
                                )
                            })
                            .child(
                                div()
                                    .relative()
                                    .text_sm()
                                    .text_color(color("foreground_secondary"))
                                    .child(title),
                            )
                            .child(group.clone())
                    }),
            )
            .child(div().text_sm().child(self.status.clone()))
            .child(
                div()
                    .text_sm()
                    .text_color(color("foreground_muted"))
                    .child("Click a circle · Tab between groups · Arrow keys select · Home / End"),
            )
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
        cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(680.), px(850.)),
                    cx,
                ))),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let owner = cx.weak_entity();
                    let theme_button = cx.new(|cx| {
                        let theme = theme.clone();
                        Button::new("Toggle theme", window, cx).on_click(move |_, cx| {
                            theme.update(cx, |theme, cx| {
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
                            });
                        })
                    });
                    let material_button = cx.new(|cx| {
                        let theme = theme.clone();
                        Button::new("Toggle material", window, cx).on_click(move |_, cx| {
                            theme.update(cx, |theme, cx| {
                                theme.set_material_mode(
                                    if theme.material_mode() == MaterialMode::Plain {
                                        MaterialMode::Glass
                                    } else {
                                        MaterialMode::Plain
                                    },
                                    cx,
                                );
                            });
                        })
                    });
                    let configurations = [
                        (
                            "retention",
                            vec![
                                RadioOption::new("month", "One month"),
                                RadioOption::new("year", "One year"),
                                RadioOption::new("forever", "Keep indefinitely"),
                            ],
                        ),
                        (
                            "verification",
                            vec![
                                RadioOption::new("quick", "Quick"),
                                RadioOption::new("full", "Full"),
                                RadioOption::new("remote", "Remote").disabled(true),
                            ],
                        ),
                        (
                            "compression",
                            vec![
                                RadioOption::new("none", "None"),
                                RadioOption::new("fast", "Fast"),
                                RadioOption::new("dense", "Dense"),
                            ],
                        ),
                        (
                            "notification",
                            vec![
                                RadioOption::new("silent", "Silent"),
                                RadioOption::new("failures", "Failures"),
                                RadioOption::new("all", "All events"),
                            ],
                        ),
                        (
                            "priority",
                            vec![
                                RadioOption::new("low", "Low"),
                                RadioOption::new("normal", "Normal"),
                                RadioOption::new("high", "High"),
                            ],
                        ),
                        (
                            "policy",
                            vec![
                                RadioOption::new("automatic", "Automatic"),
                                RadioOption::new("manual", "Manual"),
                            ],
                        ),
                    ];
                    let groups = configurations
                        .into_iter()
                        .enumerate()
                        .map(|(index, (label, options))| {
                            let owner = owner.clone();
                            cx.new(|cx| {
                                let group = RadioGroup::new(label, options, window, cx)
                                    .unwrap()
                                    .horizontal(index != 0)
                                    .disabled(index == 5)
                                    .gap(px(20.))
                                    .on_change(move |value, _, cx| {
                                        let _ = owner.update(cx, |gallery: &mut Gallery, cx| {
                                            gallery.status = format!("{label}: {value}").into();
                                            cx.notify();
                                        });
                                    });
                                match index {
                                    2 => group.material_mode(MaterialMode::Plain),
                                    3 => group.material_mode(MaterialMode::Glass),
                                    4 => group
                                        .part(RadioPart::Indicator, |s| {
                                            s.state(State::Selected, |s| {
                                                s.bg(Token::<Hsla>::new("accent"))
                                                    .border_color(Token::<Hsla>::new("accent"))
                                            })
                                        })
                                        .part(RadioPart::Dot, |s| {
                                            s.w(px(6.))
                                                .h(px(6.))
                                                .bg(Token::<Hsla>::new("on_primary"))
                                        }),
                                    _ => group,
                                }
                            })
                        })
                        .collect();
                    let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                    Gallery {
                        theme,
                        groups,
                        theme_button,
                        material_button,
                        status: "Ready".into(),
                        _subscription: subscription,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
