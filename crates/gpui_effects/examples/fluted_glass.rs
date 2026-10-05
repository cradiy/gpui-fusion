use std::sync::Arc;

use gpui::{
    App, Bounds, Context, Div, Image, ImageFormat, ImageSource, ObjectFit, Render, Stateful,
    Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{FlutedGlassOptions, fluted_glass};
use gpui_platform::application;

struct Preview {
    image: ImageSource,
    options: FlutedGlassOptions,
    grid: bool,
    clicks: usize,
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
    fn study(&self, ribbed: bool, cx: &mut Context<Self>) -> Div {
        let optics = FlutedGlassOptions {
            refraction: if ribbed {
                self.options.refraction
            } else {
                px(0.)
            },
            ..self.options
        };
        div()
            .flex_1()
            .min_w(px(290.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0xa5b4c9))
                    .child(if ribbed {
                        "02 / FLUTED GLASS"
                    } else {
                        "01 / FLAT GLASS"
                    }),
            )
            .child(
                div()
                    .relative()
                    .h(px(480.))
                    .w_full()
                    .overflow_hidden()
                    .rounded(px(24.))
                    .bg(rgb(0x202c3c))
                    .child(
                        img(self.image.clone())
                            .absolute()
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(div().absolute().inset_0().bg(rgba(0x08111b55)))
                    .when(self.grid, |s| {
                        s.children((0..16).map(|i| {
                            div()
                                .absolute()
                                .left(px(i as f32 * 32.))
                                .top_0()
                                .bottom_0()
                                .w(px(3.))
                                .bg(rgba(0xffffffaa))
                        }))
                        .children((0..16).map(|i| {
                            div()
                                .absolute()
                                .top(px(i as f32 * 32.))
                                .left_0()
                                .right_0()
                                .h(px(3.))
                                .bg(rgba(0xffffffaa))
                        }))
                    })
                    .child(
                        fluted_glass(optics)
                            .absolute()
                            .left(px(24.))
                            .right(px(24.))
                            .top(px(52.))
                            .bottom(px(36.))
                            .rounded(px(22.))
                            .p(px(24.))
                            .border_1()
                            .border_color(rgba(0xffffff44))
                            .flex()
                            .flex_col()
                            .justify_between()
                            .child(div().text_size(px(11.)).child("MATERIAL / 03"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.))
                                    .child(div().text_size(px(30.)).child("A different lens."))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .child("Light bends. The details stay."),
                                    ),
                            )
                            .child(
                                div()
                                    .id(if ribbed {
                                        "ribbed-action"
                                    } else {
                                        "flat-action"
                                    })
                                    .py(px(12.))
                                    .px(px(16.))
                                    .rounded(px(12.))
                                    .bg(rgba(0x0c152de0))
                                    .text_size(px(13.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x202e48)))
                                    .child(if self.clicks == 0 {
                                        "Take a closer look  →".to_owned()
                                    } else {
                                        format!("Clicked {} times  →", self.clicks)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.clicks += 1;
                                        cx.notify();
                                    })),
                            ),
                    ),
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
                    .child("FLUTED GLASS"),
            )
            .child(div().text_size(px(38.)).child("Light, through a rhythm."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("The same backdrop. Two ways of seeing it."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "spacing",
                            format!("Spacing: {:.0}px", f32::from(self.options.spacing)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.spacing = if this.options.spacing < px(24.) {
                                px(24.)
                            } else if this.options.spacing < px(40.) {
                                px(40.)
                            } else {
                                px(14.)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button("angle", format!("Direction: {:.0}°", self.options.angle)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.options.angle = if this.options.angle < 45. {
                                    45.
                                } else if this.options.angle < 90. {
                                    90.
                                } else {
                                    0.
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(
                            "refraction",
                            format!("Refraction: {:.0}px", f32::from(self.options.refraction)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.refraction = if this.options.refraction < px(6.) {
                                px(6.)
                            } else if this.options.refraction < px(10.) {
                                px(10.)
                            } else {
                                px(0.)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "blur",
                            format!("Blur: {:.0}px", f32::from(self.options.blur_radius)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.blur_radius = if this.options.blur_radius < px(3.) {
                                px(3.)
                            } else if this.options.blur_radius < px(8.) {
                                px(8.)
                            } else {
                                px(0.)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button("grid", if self.grid { "Grid: On" } else { "Grid: Off" }).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.grid = !this.grid;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .child(self.study(false, cx))
                    .child(self.study(true, cx)),
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
                window.set_window_title("Fluted glass");
                cx.new(|_| Preview {
                    image: Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        include_bytes!("album-cover.svg").to_vec(),
                    ))
                    .into(),
                    options: FlutedGlassOptions::default(),
                    grid: false,
                    clicks: 0,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
