use gpui::{prelude::*, *};
use gpui_fusion::{
    components::segmented::{Segment, Segmented},
    default_theme,
    style::{MaterialMode, Styled as FusionStyled, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    controls: Vec<Entity<Segmented>>,
    disabled: bool,
    status: SharedString,
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
        let surface = color("surface");
        let dark = theme.store().mode() == "dark";
        let glass = theme.material_mode() == MaterialMode::Glass;
        let action = |id, label| {
            div()
                .id(id)
                .px_4()
                .py_2()
                .rounded_lg()
                .bg(surface)
                .cursor_pointer()
                .child(label)
        };
        div()
            .id("segmented-gallery")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .child(
                div()
                    .text_size(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Segmented"),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        action("theme", if dark { "Light theme" } else { "Dark theme" }).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.theme.update(cx, |theme, cx| {
                                    theme
                                        .set_mode(
                                            if theme.store().mode() == "dark" {
                                                "light"
                                            } else {
                                                "dark"
                                            },
                                            cx,
                                        )
                                        .unwrap();
                                });
                            }),
                        ),
                    )
                    .child(
                        action(
                            "material",
                            if glass {
                                "Material: glass"
                            } else {
                                "Material: plain"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.theme.update(cx, |theme, cx| {
                                theme.set_material_mode(
                                    if theme.material_mode() == MaterialMode::Glass {
                                        MaterialMode::Plain
                                    } else {
                                        MaterialMode::Glass
                                    },
                                    cx,
                                );
                            });
                        })),
                    )
                    .child(
                        action("disabled", if self.disabled { "Enable" } else { "Disable" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.disabled = !this.disabled;
                                for control in &this.controls {
                                    control.update(cx, |control, cx| {
                                        control.set_disabled(this.disabled, cx)
                                    });
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .p_6()
                    .rounded_xl()
                    .bg(surface)
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(color("foreground_muted"))
                            .child("Follow theme material"),
                    )
                    .children(self.controls[..3].iter().cloned()),
            )
            .child(
                div()
                    .p_6()
                    .rounded_xl()
                    .bg(surface)
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(color("foreground_muted"))
                            .child("Fixed materials"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_6()
                            .items_center()
                            .child(div().w(px(80.)).child("Plain"))
                            .child(self.controls[3].clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_6()
                            .items_center()
                            .child(div().w(px(80.)).child("Glass"))
                            .child(self.controls[4].clone()),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(color("foreground_secondary"))
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(color("foreground_muted"))
                    .child("Click or drag to select · Arrow keys, Home and End · Escape to cancel"),
            )
    }
}
fn main() {
    gpui_platform::application()
        .with_assets(IconAssets)
        .run(|cx| {
            let mut theme = default_theme();
            theme.material = MaterialMode::Glass;
            let theme =
                ThemeRuntime::install(ThemeStore::new(theme, "dark").unwrap(), "neutral", cx)
                    .unwrap();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(780.), px(620.)),
                        cx,
                    ))),
                    titlebar: None,
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let owner = cx.weak_entity();
                        let mut controls = Vec::new();
                        for index in 0..5 {
                            let owner = owner.clone();
                            let items = match index {
                                0 => vec![
                                    Segment::new("full", "Original"),
                                    Segment::new("balanced", "Balanced"),
                                    Segment::new("compact", "Compact"),
                                ],
                                1 => vec![
                                    Segment::new("incoming", "Incoming")
                                        .icon(GpuiIcons::LucideChevronLeft.path()),
                                    Segment::new("outgoing", "Outgoing")
                                        .icon(GpuiIcons::LucideChevronRight.path()),
                                    Segment::new("paused", "Paused")
                                        .icon(GpuiIcons::LucideX.path())
                                        .disabled(true),
                                ],
                                2 => vec![
                                    Segment::new("hints", "Hints")
                                        .icon_only(GpuiIcons::LucideLightbulb.path()),
                                    Segment::new("color", "Color")
                                        .icon_only(GpuiIcons::LucidePipette.path()),
                                    Segment::new("details", "Details")
                                        .icon_only(GpuiIcons::LucideInfo.path()),
                                ],
                                _ => vec![
                                    Segment::new("local", "Local"),
                                    Segment::new("remote", "Remote"),
                                ],
                            };
                            controls.push(cx.new(|cx| {
                                let control = Segmented::new("Output options", items, window, cx)
                                    .unwrap()
                                    .w(px(if index == 2 {
                                        156.
                                    } else if index >= 3 {
                                        240.
                                    } else {
                                        440.
                                    }))
                                    .on_change(move |value, _, cx| {
                                        let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                            view.status = format!("Selected: {value}").into();
                                            cx.notify();
                                        });
                                    });
                                match index {
                                    3 => control.material_mode(MaterialMode::Plain),
                                    4 => control.material_mode(MaterialMode::Glass),
                                    _ => control,
                                }
                            }));
                        }
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            controls,
                            disabled: false,
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
