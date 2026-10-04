use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{GrainOptions, surface_grain};
use gpui_platform::application;

struct Preview {
    enabled: bool,
    light: bool,
    options: GrainOptions,
}

fn button(id: &'static str, label: impl Into<gpui::SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(rgb(0x202736))
        .text_size(px(13.))
        .text_color(rgb(0xecf1fc))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x303a4c)))
        .child(label.into())
}

impl Preview {
    fn card(&self, textured: bool, gradient: bool) -> Div {
        let base = if self.light { 0xe8e2d8 } else { 0x202b38 };
        let ink = if self.light { 0x27313d } else { 0xecf1fc };
        let muted = if self.light { 0x65707b } else { 0xa5b4c9 };
        let options = GrainOptions {
            strength: if textured && self.enabled {
                self.options.strength
            } else {
                0.
            },
            ..self.options
        };
        surface_grain(options)
            .w_full()
            .h(px(238.))
            .flex_shrink_0()
            .rounded(px(24.))
            .p(px(28.))
            .bg(rgb(base))
            .text_color(rgb(ink))
            .when(gradient, |s| {
                s.bg(linear_gradient(
                    125.,
                    linear_color_stop(rgb(if self.light { 0xe6c4bc } else { 0x4a3e65 }), 0.),
                    linear_color_stop(rgb(if self.light { 0xb5c9d1 } else { 0x244c57 }), 1.),
                ))
            })
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(muted))
                    .child(if gradient {
                        "02 / COLOR STUDY"
                    } else {
                        "01 / EVERYDAY SURFACE"
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().text_size(px(28.)).child(if gradient {
                        "A softer spectrum."
                    } else {
                        "Room to breathe."
                    }))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(muted))
                            .child("Small details. A different feeling."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(12.)).child("Explore  →"))
                    .child(div().size(px(8.)).rounded_full().bg(rgba(0xffffff88))),
            )
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("SURFACE GRAIN"),
            )
            .child(div().text_size(px(38.)).child("A little texture."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Quiet surfaces. Crisp content. Compare them side by side."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "enabled",
                            if self.enabled {
                                "Grain: On"
                            } else {
                                "Grain: Off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.enabled = !this.enabled;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "strength",
                            format!("Strength: {:.0}%", self.options.strength * 100.),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.strength = if this.options.strength < 0.1 {
                                0.12
                            } else if this.options.strength < 0.2 {
                                0.24
                            } else {
                                0.06
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "size",
                            format!("Size: {:.2}px", f32::from(self.options.size)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.size = if this.options.size < px(1.) {
                                px(1.25)
                            } else if this.options.size < px(2.) {
                                px(2.)
                            } else {
                                px(0.75)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "color",
                            if self.options.colored {
                                "Colored"
                            } else {
                                "Monochrome"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.colored = !this.options.colored;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "theme",
                            if self.light {
                                "Light surface"
                            } else {
                                "Dark surface"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.light = !this.light;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .children([false, true].map(|textured| {
                        div()
                            .flex_1()
                            .min_w(px(290.))
                            .flex()
                            .flex_col()
                            .gap(px(16.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(div().size(px(6.)).rounded_full().bg(rgb(if textured {
                                        0xb8c4ee
                                    } else {
                                        0x66748a
                                    })))
                                    .child(
                                        div().text_size(px(12.)).text_color(rgb(0xa5b4c9)).child(
                                            if textured { "WITH GRAIN" } else { "ORIGINAL" },
                                        ),
                                    ),
                            )
                            .child(self.card(textured, false))
                            .child(self.card(textured, true))
                    })),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(830.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Surface grain");
                cx.new(|_| Preview {
                    enabled: true,
                    light: false,
                    options: GrainOptions::default(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
