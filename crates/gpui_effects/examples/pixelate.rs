use std::{sync::Arc, time::Instant};

use gpui::{
    App, Bounds, Context, Div, Image, ImageFormat, ImageSource, MouseButton, ObjectFit, Render,
    Stateful, Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use gpui_effects::{PixelateOptions, subtree_pixelate};
use gpui_platform::application;

struct Preview {
    image: ImageSource,
    cell_size: f32,
    strength: f32,
    held: bool,
    progress: f32,
    last_frame: Instant,
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
    fn hold(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.held != held {
            self.held = held;
            self.last_frame = Instant::now();
            cx.notify();
        }
    }

    fn card(&self, index: usize) -> Div {
        let source = img(self.image.clone())
            .w_full()
            .h(px(300.))
            .object_fit(ObjectFit::Cover);
        let progress = if index == 1 { 1. } else { self.progress };
        let eased = progress * progress * (3. - 2. * progress);
        let artwork = if index == 0 {
            source.into_any_element()
        } else {
            subtree_pixelate(
                source,
                PixelateOptions {
                    cell_size: px(1. + (self.cell_size - 1.) * eased),
                    strength: self.strength * eased,
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
            .child(div().text_size(px(11.)).text_color(rgb(0xa5b4c9)).child(
                [
                    "01 / ORIGINAL",
                    "02 / PIXEL STUDY",
                    "03 / HOLD TO TRANSFORM",
                ][index],
            ))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !window.is_window_active() {
            self.held = false;
        }
        let now = Instant::now();
        let target = if self.held { 1. } else { 0. };
        let step = now.duration_since(self.last_frame).as_secs_f32().min(0.05) / 0.6;
        self.progress += (target - self.progress).clamp(-step, step);
        self.last_frame = now;
        if self.progress != target {
            window.request_animation_frame();
        }
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
                    .child("PIXELATE"),
            )
            .child(
                div()
                    .text_size(px(38.))
                    .child("Less detail. A different feeling."),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Hold to turn color into blocks. Release to bring it back."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "hold",
                            if self.held {
                                "Release to restore"
                            } else {
                                "Hold to pixelate"
                            },
                        )
                        .bg(rgb(if self.held { 0x405b80 } else { 0x2e4260 }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.hold(true, cx)),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.hold(false, cx)),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.hold(false, cx)),
                        ),
                    )
                    .child(
                        button("size", format!("Cell size: {:.0}px", self.cell_size)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.cell_size = match this.cell_size as u32 {
                                    8 => 16.,
                                    16 => 32.,
                                    32 => 64.,
                                    _ => 8.,
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("strength", format!("Mix: {:.0}%", self.strength * 100.)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.strength = if this.strength > 0.5 { 0.5 } else { 1. };
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
                    .children((0..3).map(|i| self.card(i))),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.hold(false, cx);
                }
            }))
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
                window.set_window_title("Pixelate");
                cx.new(|_| Preview {
                    image: Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        include_bytes!("album-cover.svg").to_vec(),
                    ))
                    .into(),
                    cell_size: 16.,
                    strength: 1.,
                    held: false,
                    progress: 0.,
                    last_frame: Instant::now(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
