use std::sync::Arc;

use gpui::{
    App, Bounds, Context, Div, Image, ImageFormat, ImageSource, ObjectFit, Render, Stateful,
    Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use gpui_effects::{
    ChromaticAberrationMode, ChromaticAberrationOptions, subtree_chromatic_aberration,
};
use gpui_platform::application;

struct Preview {
    image: ImageSource,
    amount: f32,
    angle: f32,
    enabled: bool,
    graphic: bool,
}

fn button(id: &'static str, label: impl Into<gpui::SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(rgb(0x202736))
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x303a4c)))
        .child(label.into())
}

impl Preview {
    fn artwork(&self) -> gpui::AnyElement {
        if !self.graphic {
            return img(self.image.clone())
                .w_full()
                .h(px(300.))
                .object_fit(ObjectFit::Cover)
                .into_any_element();
        }
        div()
            .w_full()
            .h(px(300.))
            .bg(rgb(0x111927))
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap(px(20.))
            .child(
                div()
                    .size(px(100.))
                    .rounded_full()
                    .border_8()
                    .border_color(rgb(0xf3efe6)),
            )
            .child(div().text_size(px(38.)).child("AFTER / 04"))
            .child(div().w(px(170.)).h(px(5.)).bg(rgb(0xf3efe6)))
            .into_any_element()
    }

    fn card(&self, index: usize) -> Div {
        let artwork = self.artwork();
        let artwork = if index == 0 {
            artwork
        } else {
            subtree_chromatic_aberration(
                artwork,
                ChromaticAberrationOptions {
                    amount: px(if self.enabled { self.amount } else { 0. }),
                    mode: if index == 1 {
                        ChromaticAberrationMode::Radial
                    } else {
                        ChromaticAberrationMode::Directional { angle: self.angle }
                    },
                },
            )
            .into_any_element()
        };
        div()
            .flex_1()
            .min_w(px(230.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0xa5b4c9))
                    .child(["01 / ORIGINAL", "02 / RADIAL", "03 / DIRECTIONAL"][index]),
            )
            .child(
                div()
                    .rounded(px(22.))
                    .overflow_hidden()
                    .bg(rgb(0x192230))
                    .child(artwork)
                    .child(
                        div()
                            .p(px(22.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(div().text_size(px(22.)).child("Midnight garden"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0xa5b4c9))
                                    .child("A collection of quiet moments."),
                            )
                            .child(div().mt(px(10.)).text_size(px(12.)).child("Explore  →")),
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
                    .child("CHROMATIC ABERRATION"),
            )
            .child(div().text_size(px(38.)).child("A little out of alignment."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Color at the edges. Clarity where it matters."),
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
                                "Effect: On"
                            } else {
                                "Effect: Off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.enabled = !this.enabled;
                            cx.notify();
                        })),
                    )
                    .child(
                        button("amount", format!("Offset: {:.0}px", self.amount)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.amount = match this.amount as u32 {
                                    2 => 4.,
                                    4 => 8.,
                                    _ => 2.,
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("angle", format!("Direction: {:.0}°", self.angle)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.angle = (this.angle + 45.) % 180.;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(
                            "artwork",
                            if self.graphic {
                                "Artwork: Graphic"
                            } else {
                                "Artwork: Cover"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.graphic = !this.graphic;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .children((0..3).map(|i| self.card(i))),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(820.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Chromatic aberration");
                cx.new(|_| Preview {
                    image: Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        include_bytes!("album-cover.svg").to_vec(),
                    ))
                    .into(),
                    amount: 4.,
                    angle: 0.,
                    enabled: true,
                    graphic: false,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
