use gpui::{prelude::*, *};
use gpui_fusion::style::Styled as FusionStyled;
use gpui_fusion::{
    default_theme,
    runtime::ThemeRuntime,
    switch::{Switch, SwitchContent, SwitchPart},
    theme::ThemeStore,
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    switches: Vec<Entity<Switch>>,
    disabled: bool,
    changes: usize,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theme.read(cx).store().mode() == "dark";
        let panel = if dark { rgb(0x262a30) } else { rgb(0xffffff) };
        div()
            .id("switch-gallery")
            .overflow_y_scroll()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(if dark { rgb(0x17191e) } else { rgb(0xf5f6f8) })
            .text_color(if dark { rgb(0xf1f3f7) } else { rgb(0x242833) })
            .child(div().text_size(px(24.)).child("Switches"))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .id("theme")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(panel)
                            .child(if dark { "Light theme" } else { "Dark theme" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.theme.update(cx, |t, cx| {
                                    let mode = if t.store().mode() == "dark" {
                                        "light"
                                    } else {
                                        "dark"
                                    };
                                    t.set_mode(mode, cx).unwrap();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("disable")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(panel)
                            .child(if self.disabled {
                                "Enable controls"
                            } else {
                                "Disable controls"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.disabled = !this.disabled;
                                for switch in &this.switches {
                                    switch.update(cx, |s, cx| s.set_disabled(this.disabled, cx));
                                }
                                cx.notify();
                            })),
                    ),
            )
            .children(self.switches.iter().enumerate().map(|(index, switch)| {
                div()
                    .p_6()
                    .flex_shrink_0()
                    .rounded_xl()
                    .bg(panel)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                [
                                    "Standard",
                                    "Glass",
                                    "Text · standard",
                                    "Text · glass",
                                    "Icons · glass",
                                ][index],
                            )
                            .child(div().text_size(px(13.)).opacity(0.6).child(
                                if index == 0 || index == 2 {
                                    "Solid track and thumb"
                                } else {
                                    "Frosted at rest · hold or drag to reveal the lens"
                                },
                            )),
                    )
                    .child(switch.clone())
            }))
            .child(
                div()
                    .text_size(px(13.))
                    .opacity(0.65)
                    .child(format!("Changes: {}", self.changes)),
            )
    }
}
fn main() {
    gpui_platform::application()
        .with_assets(IconAssets)
        .run(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(700.), px(780.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    let theme = cx.new(|_| {
                        ThemeRuntime::new(ThemeStore::new(default_theme(), "light").unwrap())
                    });
                    cx.new(|cx| {
                        let owner = cx.weak_entity();
                        let switches = [None, Some("glass"), None, Some("glass"), Some("glass")]
                            .into_iter()
                            .enumerate()
                            .map(|(index, variant)| {
                                let owner = owner.clone();
                                cx.new(|cx| {
                                    let control = Switch::new(
                                        "Enable updates",
                                        false,
                                        theme.clone(),
                                        "neutral",
                                        window,
                                        cx,
                                    )
                                    .on_change(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                                view.changes += 1;
                                                cx.notify();
                                            });
                                        },
                                    );
                                    let control = match index {
                                        2 | 3 => control
                                            .content(
                                                SwitchContent::Text("OFF".into()),
                                                SwitchContent::Text("ON".into()),
                                            )
                                            .w(px(78.))
                                            .part(SwitchPart::Thumb, |s| {
                                                s.w(px(38.)).text_size(px(10.))
                                            }),
                                        4 => control.content(
                                            SwitchContent::Icon(GpuiIcons::LucideX.path().into()),
                                            SwitchContent::Icon(
                                                GpuiIcons::LucideCheck.path().into(),
                                            ),
                                        ),
                                        _ => control,
                                    };
                                    if let Some(variant) = variant {
                                        control.variant(variant)
                                    } else {
                                        control
                                    }
                                })
                            })
                            .collect();
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            switches,
                            disabled: false,
                            changes: 0,
                            _subscription: subscription,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
