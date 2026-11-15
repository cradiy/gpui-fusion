use gpui::{prelude::*, *};
use gpui_fusion::{
    default_theme,
    runtime::ThemeRuntime,
    slider::{Slider, SliderRange},
    style::{StylePatch, Styled as FusionStyled, Token},
    theme::ThemeStore,
};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    sliders: Vec<Entity<Slider>>,
    values: Vec<f64>,
    disabled: bool,
    border: usize,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.theme.read(cx).store().mode() == "dark";
        let panel = if dark { rgb(0x23262c) } else { rgb(0xffffff) };
        div()
            .id("slider-gallery")
            .overflow_y_scroll()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(if dark { rgb(0x17191e) } else { rgb(0xf5f6f8) })
            .text_color(if dark { rgb(0xf1f3f7) } else { rgb(0x242833) })
            .child(div().text_size(px(24.)).child("Sliders"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
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
                            .id("disabled")
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
                                for slider in &this.sliders {
                                    slider.update(cx, |s, cx| s.set_disabled(this.disabled, cx));
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("border")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(panel)
                            .child(["Border: theme", "Border: none", "Border: solid"][self.border])
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.border = (this.border + 1) % 3;
                                for slider in &this.sliders {
                                    let style = StylePatch::default().w(px(400.));
                                    let style = match this.border {
                                        1 => style.part(
                                            gpui_fusion::style::PartName("thumb".into()),
                                            |s| s.border(px(0.)),
                                        ),
                                        2 => style.part(
                                            gpui_fusion::style::PartName("thumb".into()),
                                            |s| {
                                                s.border(px(2.))
                                                    .border_color(Token::<Hsla>::new("primary"))
                                            },
                                        ),
                                        _ => style,
                                    };
                                    slider.update(cx, |s, cx| s.set_style(style, cx));
                                }
                                cx.notify();
                            })),
                    ),
            )
            .children(self.sliders.iter().enumerate().map(|(index, slider)| {
                div()
                    .p_6()
                    .rounded_xl()
                    .bg(panel)
                    .flex()
                    .flex_col()
                    .gap_5()
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                ["Standard", "Glass", "Stepped · standard", "Stepped · glass"]
                                    [index],
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .opacity(0.65)
                                    .child(format!("{:.1}", self.values[index])),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(div().text_size(px(13.)).opacity(0.6).child(if index < 2 {
                                "0"
                            } else {
                                "−20"
                            }))
                            .child(slider.clone())
                            .child(div().text_size(px(13.)).opacity(0.6).child(if index < 2 {
                                "100"
                            } else {
                                "20"
                            })),
                    )
            }))
            .child(div().text_size(px(13.)).opacity(0.6).child(
                "Drag or click the track · Arrow keys to adjust · Home / End · Page Up / Down",
            ))
    }
}
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(680.), px(760.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                let theme = cx
                    .new(|_| ThemeRuntime::new(ThemeStore::new(default_theme(), "light").unwrap()));
                cx.new(|cx| {
                    let owner = cx.weak_entity();
                    let values = vec![38., 62., -5., 10.];
                    let sliders = values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            let owner = owner.clone();
                            cx.new(|cx| {
                                let slider = Slider::new(
                                    "Level",
                                    *value,
                                    theme.clone(),
                                    "neutral",
                                    window,
                                    cx,
                                )
                                .range(if index < 2 {
                                    SliderRange::new(0., 100., None).unwrap()
                                } else {
                                    SliderRange::new(-20., 20., Some(5.)).unwrap()
                                })
                                .w(px(400.))
                                .on_change(move |value, _, cx| {
                                    let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                        view.values[index] = value;
                                        cx.notify();
                                    });
                                });
                                if index % 2 == 1 {
                                    slider.variant("glass")
                                } else {
                                    slider
                                }
                            })
                        })
                        .collect();
                    let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                    Gallery {
                        theme,
                        sliders,
                        values,
                        disabled: false,
                        border: 0,
                        _subscription: subscription,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
