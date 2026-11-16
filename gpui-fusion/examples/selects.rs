use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        select::{Select, SelectOption, SelectPart},
    },
    default_theme,
    style::{MaterialMode, Styled as FusionStyled, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    controls: Vec<Entity<Select>>,
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
            "Image format",
            "Color profile · searchable",
            "Fixed plain material",
            "Fixed glass material",
            "Managed format",
        ];
        div()
            .id("select-gallery")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .on_key_down(|event, window, cx| {
                let m = event.keystroke.modifiers;
                if event.keystroke.key == "tab" && !m.control && !m.alt && !m.platform {
                    if m.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            })
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_2xl().child("Select fields"))
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
            .children(self.controls.iter().zip(titles).enumerate().map(
                |(index, (control, title))| {
                    div()
                        .relative()
                        .rounded_xl()
                        .p_6()
                        .bg(color("surface"))
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_3()
                        .when(index == 0 || index == 3, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .right(px(80.))
                                    .top_0()
                                    .bottom_0()
                                    .w(px(160.))
                                    .rounded_xl()
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
                        .child(control.clone())
                },
            ))
            .child(div().text_sm().child(self.status.clone()))
            .child(
                div().text_sm().text_color(color("foreground_muted")).child(
                    "Arrow keys navigate · Enter selects · Escape dismisses · Tab moves focus",
                ),
            )
    }
}
fn formats() -> [SelectOption; 4] {
    [
        SelectOption::new("png", "PNG").icon(GpuiIcons::LucideImage.path()),
        SelectOption::new("jpeg", "JPEG"),
        SelectOption::new("avif", "AVIF"),
        SelectOption::new("heif", "HEIF").disabled(true),
    ]
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
                        size(px(700.), px(830.)),
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
                                theme.update(cx, |t, cx| {
                                    t.set_mode(
                                        if t.store().mode() == "light" {
                                            "dark"
                                        } else {
                                            "light"
                                        },
                                        cx,
                                    )
                                    .unwrap();
                                })
                            })
                        });
                        let material_button = cx.new(|cx| {
                            let theme = theme.clone();
                            Button::new("Toggle material", window, cx).on_click(move |_, cx| {
                                theme.update(cx, |t, cx| {
                                    t.set_material_mode(
                                        if t.material_mode() == MaterialMode::Plain {
                                            MaterialMode::Glass
                                        } else {
                                            MaterialMode::Plain
                                        },
                                        cx,
                                    );
                                })
                            })
                        });
                        let controls = (0..5)
                            .map(|index| {
                                let owner = owner.clone();
                                cx.new(|cx| {
                                    let options = if index == 1 {
                                        (0..10000)
                                            .map(|i| {
                                                SelectOption::new(
                                                    format!("profile-{i}"),
                                                    format!("Calibration profile {i:05}"),
                                                )
                                                .keywords(if i % 2 == 0 {
                                                    "display"
                                                } else {
                                                    "print"
                                                })
                                            })
                                            .collect()
                                    } else {
                                        formats().to_vec()
                                    };
                                    let mut select = Select::new(
                                        if index == 1 {
                                            "Color profile"
                                        } else {
                                            "Image format"
                                        },
                                        options,
                                        window,
                                        cx,
                                    )
                                    .unwrap()
                                    .w(px(320.))
                                    .searchable(index == 1)
                                    .clearable(true)
                                    .disabled(index == 4)
                                    .part(SelectPart::Panel, |s| s.h(px(300.)))
                                    .on_change(move |value, _, cx| {
                                        let _ = owner.update(cx, |g: &mut Gallery, cx| {
                                            g.status = value
                                                .map(|v| format!("Selected: {v}"))
                                                .unwrap_or_else(|| "Selection cleared".into())
                                                .into();
                                            cx.notify();
                                        });
                                    });
                                    if index != 1 {
                                        select.set_selected(Some("png"), cx).unwrap();
                                    }
                                    match index {
                                        2 => select.material_mode(MaterialMode::Plain),
                                        3 => select.material_mode(MaterialMode::Glass),
                                        _ => select,
                                    }
                                })
                            })
                            .collect();
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            controls,
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
