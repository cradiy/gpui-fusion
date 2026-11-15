use gpui::{prelude::*, *};
use gpui_fusion::{
    components::button::Button,
    components::slider::Slider,
    components::switch::Switch,
    style::{MaterialMode, Styled as FusionStyled, Token},
    theme::ResolveContext,
    theme::catalog::{ThemeRepository, ThemeSource},
    theme::runtime::ThemeRuntime,
    theme::session::ThemeSession,
};
use std::path::PathBuf;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    buttons: Vec<Entity<Button>>,
    switches: Vec<Entity<Switch>>,
    sliders: Vec<Entity<Slider>>,
    directory: PathBuf,
    session: Entity<ThemeSession>,
    choices: Vec<(String, ThemeSource)>,
    _session_subscription: Subscription,
    message: String,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theme.read(cx).store().mode() == "dark";
        let glass = self.theme.read(cx).material_mode() == MaterialMode::Glass;
        let runtime = self.theme.read(cx);
        let context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        let panel = color("surface");
        let background = color("muted_surface");
        let foreground = color("foreground");
        let control = |id, label| {
            div()
                .id(id)
                .focusable()
                .cursor_pointer()
                .px_4()
                .py_2()
                .rounded_lg()
                .bg(panel)
                .child(label)
        };
        div()
            .id("theme-gallery")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(background)
            .text_color(foreground)
            .child(div().text_size(px(24.)).child("Theme & materials"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        control("mode", if dark { "Light theme" } else { "Dark theme" }).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.theme.update(cx, |theme, cx| {
                                    let next = if theme.store().mode() == "dark" {
                                        "light"
                                    } else {
                                        "dark"
                                    };
                                    theme.set_mode(next, cx).unwrap();
                                });
                            }),
                        ),
                    )
                    .child(
                        control(
                            "material",
                            if glass {
                                "Material: glass"
                            } else {
                                "Material: plain"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.theme.update(cx, |theme, cx| {
                                let next = if theme.material_mode() == MaterialMode::Glass {
                                    MaterialMode::Plain
                                } else {
                                    MaterialMode::Glass
                                };
                                theme.set_material_mode(next, cx);
                            });
                        })),
                    )
                    .child(
                        control("follow", "Follow configuration").on_click(cx.listener(
                            |this, _, _, cx| {
                                let result = this.session.update(cx, |session, cx| {
                                    session.set_source(
                                        ThemeSource::Follow {
                                            app_id: APP_ID.into(),
                                        },
                                        cx,
                                    )
                                });
                                this.message =
                                    result.err().map(|e| e.to_string()).unwrap_or_default();
                                cx.notify();
                            },
                        )),
                    )
                    .child(control("reload", "Reload selection").on_click(cx.listener(
                        |this, _, _, cx| {
                            let result = this.session.update(cx, |session, cx| session.reload(cx));
                            this.message = result.err().map(|e| e.to_string()).unwrap_or_default();
                            cx.notify();
                        },
                    )))
                    .child({
                        #[cfg(feature = "theme-watch")]
                        {
                            control(
                                "watch",
                                if self.session.read(cx).is_watching() {
                                    "Stop watching"
                                } else {
                                    "Watch files"
                                },
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let result = this.session.update(cx, |session, cx| {
                                    if session.is_watching() {
                                        session.stop_watching();
                                        Ok(())
                                    } else {
                                        session.start_watching(cx)
                                    }
                                });
                                this.message =
                                    result.err().map(|e| e.to_string()).unwrap_or_default();
                                cx.notify();
                            }))
                            .into_any_element()
                        }
                        #[cfg(not(feature = "theme-watch"))]
                        {
                            div().child("File watching is disabled").into_any_element()
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(
                        self.choices
                            .iter()
                            .enumerate()
                            .map(|(index, (name, source))| {
                                let source = source.clone();
                                div()
                                    .id(("theme-choice", index))
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(panel)
                                    .cursor_pointer()
                                    .child(name.clone())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let result = this.session.update(cx, |session, cx| {
                                            session.set_source(source.clone(), cx)
                                        });
                                        this.message =
                                            result.err().map(|e| e.to_string()).unwrap_or_default();
                                        cx.notify();
                                    }))
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children((0..3).map(|index| {
                        div()
                            .w(px(260.))
                            .p_5()
                            .rounded_xl()
                            .bg(panel)
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(["Follow theme", "Always plain", "Always glass"][index])
                            .child(
                                div()
                                    .h(px(56.))
                                    .flex()
                                    .items_center()
                                    .child(self.buttons[index].clone()),
                            )
                            .child(
                                div()
                                    .h(px(56.))
                                    .flex()
                                    .items_center()
                                    .child(self.switches[index].clone()),
                            )
                            .child(
                                div()
                                    .h(px(56.))
                                    .flex()
                                    .items_center()
                                    .child(self.sliders[index].clone()),
                            )
                    })),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .opacity(0.7)
                    .child(format!("Theme directory: {}", self.directory.display())),
            )
            .child(div().text_size(px(13.)).child(self.message.clone()))
            .child(
                div().text_size(px(13.)).child(
                    self.session
                        .read(cx)
                        .error()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                ),
            )
    }
}
const APP_ID: &str = "org.gpui.fusion.themes";

fn main() {
    let repository = match ThemeRepository::discover() {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let mut choices = Vec::new();
    if let Ok(catalog) = repository.global_catalog() {
        for (id, entry) in &catalog.index().themes {
            choices.push((
                format!("Global: {}", entry.name),
                ThemeSource::Global {
                    theme_id: id.clone(),
                },
            ));
        }
    }
    if let Ok(catalog) = repository.application_catalog(APP_ID) {
        for (id, entry) in &catalog.index().themes {
            choices.push((
                format!("Application: {}", entry.name),
                ThemeSource::Application {
                    app_id: APP_ID.into(),
                    theme_id: id.clone(),
                },
            ));
        }
    }
    gpui_platform::application().run(move |cx| {
        let session = match ThemeSession::install(
            repository.clone(),
            ThemeSource::Follow {
                app_id: APP_ID.into(),
            },
            cx,
        ) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("{error}");
                cx.quit();
                return;
            }
        };
        #[cfg(feature = "theme-watch")]
        session
            .update(cx, |session, cx| session.start_watching(cx))
            .unwrap();
        let theme = session.read(cx).runtime().clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(940.), px(560.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let policies = [None, Some(MaterialMode::Plain), Some(MaterialMode::Glass)];
                    let buttons = policies
                        .iter()
                        .map(|policy| {
                            cx.new(|cx| {
                                let button = Button::new("Apply", window, cx);
                                if let Some(mode) = policy {
                                    button.material_mode(*mode)
                                } else {
                                    button
                                }
                            })
                        })
                        .collect();
                    let switches = policies
                        .iter()
                        .map(|policy| {
                            cx.new(|cx| {
                                let switch = Switch::new("Updates", false, window, cx);
                                if let Some(mode) = policy {
                                    switch.material_mode(*mode)
                                } else {
                                    switch
                                }
                            })
                        })
                        .collect();
                    let sliders = policies
                        .iter()
                        .map(|policy| {
                            cx.new(|cx| {
                                let slider = Slider::new("Level", 44., window, cx).w(px(210.));
                                if let Some(mode) = policy {
                                    slider.material_mode(*mode)
                                } else {
                                    slider
                                }
                            })
                        })
                        .collect();
                    let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                    Gallery {
                        theme: theme.clone(),
                        buttons,
                        switches,
                        sliders,
                        directory: repository.root().to_owned(),
                        session: session.clone(),
                        choices: choices.clone(),
                        _session_subscription: cx.observe(&session, |_, _, cx| cx.notify()),
                        message: String::new(),
                        _subscription: subscription,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
