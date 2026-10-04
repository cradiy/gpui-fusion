use std::sync::Arc;

use gpui::{
    App, Bounds, Context, Div, Image, ImageFormat, ImageSource, ObjectFit, Render, Stateful,
    Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use gpui_effects::{HalftoneOptions, subtree_halftone};
use gpui_platform::application;

const PALETTES: [(&str, u32, u32); 3] = [
    ("Midnight / Cream", 0x243149, 0xf2e6cf),
    ("Terracotta / Sand", 0x8a382b, 0xf1d6b0),
    ("Black / White", 0x171717, 0xf3f3ef),
];

struct Preview {
    image: ImageSource,
    options: HalftoneOptions,
    enabled: bool,
    palette: usize,
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
    fn card(&self, print: bool) -> Div {
        let image = img(self.image.clone())
            .w_full()
            .h(px(330.))
            .object_fit(ObjectFit::Cover);
        let artwork = if print {
            subtree_halftone(
                image,
                HalftoneOptions {
                    strength: if self.enabled {
                        self.options.strength
                    } else {
                        0.
                    },
                    ..self.options
                },
            )
            .into_any_element()
        } else {
            image.into_any_element()
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
                    .child(if print {
                        "02 / PRINT STUDY"
                    } else {
                        "01 / ORIGINAL"
                    }),
            )
            .child(
                div()
                    .w_full()
                    .rounded(px(24.))
                    .overflow_hidden()
                    .bg(rgb(0x192230))
                    .child(artwork)
                    .child(
                        div()
                            .p(px(26.))
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(0x929fbc))
                                    .child("AFTER HOURS / VOL. 04"),
                            )
                            .child(div().text_size(px(28.)).child("Midnight garden"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0xa5b4c9))
                                    .child("A collection of quiet moments."),
                            )
                            .child(
                                div()
                                    .mt(px(12.))
                                    .flex()
                                    .justify_between()
                                    .text_size(px(12.))
                                    .child("12 tracks · 48 min")
                                    .child("Explore  →"),
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
                    .child("HALFTONE"),
            )
            .child(div().text_size(px(38.)).child("Made of little dots."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("From luminous color to ink on paper."),
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
                                "Print: On"
                            } else {
                                "Print: Off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.enabled = !this.enabled;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "spacing",
                            format!("Spacing: {:.0}px", f32::from(self.options.spacing)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.spacing = if this.options.spacing < px(6.) {
                                px(6.)
                            } else if this.options.spacing < px(10.) {
                                px(10.)
                            } else {
                                px(4.)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        button("angle", format!("Angle: {:.0}°", self.options.angle)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.options.angle = if this.options.angle < 30. {
                                    30.
                                } else if this.options.angle < 45. {
                                    45.
                                } else {
                                    0.
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(
                            "mix",
                            format!("Print mix: {:.0}%", self.options.strength * 100.),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.options.strength =
                                if this.options.strength > 0.5 { 0.5 } else { 1. };
                            cx.notify();
                        })),
                    )
                    .child(
                        button("palette", PALETTES[self.palette].0).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.palette = (this.palette + 1) % PALETTES.len();
                                this.options.ink = rgb(PALETTES[this.palette].1);
                                this.options.paper = rgb(PALETTES[this.palette].2);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .child(self.card(false))
                    .child(self.card(true)),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(850.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Halftone");
                cx.new(|_| Preview {
                    image: Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        include_bytes!("album-cover.svg").to_vec(),
                    ))
                    .into(),
                    options: HalftoneOptions::default(),
                    enabled: true,
                    palette: 0,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
