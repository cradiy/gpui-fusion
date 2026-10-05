use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    App, Bounds, Context, Div, Image, ImageFormat, ImageSource, MouseButton, MouseMoveEvent,
    ObjectFit, Pixels, Render, Stateful, Window, WindowBounds, WindowOptions, canvas, div, img,
    linear_color_stop, prelude::*, px, relative, rgb, size,
};
use gpui_effects::{GradientMapPalette, subtree_gradient_map};
use gpui_platform::application;

const COLORS: [u32; 8] = [
    0x18223c, 0x604491, 0xb75998, 0xe7979a, 0xf6dfb1, 0x80c8bf, 0x5d8ada, 0xffffff,
];

fn palette() -> GradientMapPalette {
    GradientMapPalette::new(
        COLORS[..5]
            .iter()
            .enumerate()
            .map(|(i, color)| linear_color_stop(rgb(*color), i as f32 / 4.))
            .collect::<Vec<_>>(),
    )
}

struct Preview {
    image: ImageSource,
    palette: GradientMapPalette,
    selected: usize,
    dragging: bool,
    ramp_bounds: Rc<Cell<Bounds<Pixels>>>,
    strength: f32,
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
    fn move_stop(&mut self, x: Pixels) {
        let bounds = self.ramp_bounds.get();
        if bounds.size.width <= px(0.) {
            return;
        }
        let stops = self.palette.stops();
        let mut stop = stops[self.selected];
        let low = if self.selected == 0 {
            0.
        } else {
            stops[self.selected - 1].percentage
        };
        let high = stops.get(self.selected + 1).map_or(1., |s| s.percentage);
        stop.percentage =
            (f32::from(x - bounds.origin.x) / f32::from(bounds.size.width)).clamp(low, high);
        self.palette.set_stop(self.selected, stop);
    }

    fn card(&self, mode: usize) -> Div {
        let source = img(self.image.clone())
            .w_full()
            .h(px(245.))
            .object_fit(ObjectFit::Cover);
        let image = match mode {
            0 => source.into_any_element(),
            1 => subtree_gradient_map(
                source,
                GradientMapPalette::new([
                    linear_color_stop(rgb(0x18223c), 0.),
                    linear_color_stop(rgb(0xf6dfb1), 1.),
                ]),
            )
            .strength(self.strength)
            .into_any_element(),
            _ => subtree_gradient_map(source, self.palette.clone())
                .strength(self.strength)
                .into_any_element(),
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
                    .child(["01 / ORIGINAL", "02 / DUOTONE", "03 / YOUR PALETTE"][mode]),
            )
            .child(
                div()
                    .rounded(px(22.))
                    .overflow_hidden()
                    .bg(rgb(0x192230))
                    .child(image)
                    .child(
                        div()
                            .p(px(22.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(div().text_size(px(23.)).child("Midnight garden"))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(0xa5b4c9))
                                    .child("Same light. A new mood."),
                            ),
                    ),
            )
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ramp_bounds = self.ramp_bounds.clone();
        let stop = self.palette.stops()[self.selected];
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .p(px(32.))
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .flex()
            .flex_col()
            .gap(px(22.))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    if event.dragging() {
                        this.move_stop(event.position.x);
                        cx.notify();
                    } else {
                        this.dragging = false;
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("GRADIENT MAP"),
            )
            .child(div().text_size(px(38.)).child("Give light a new palette."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Drag a stop. Pick a color. Watch the image change."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button("mix", format!("Mix: {:.0}%", self.strength * 100.)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.strength = if this.strength > 0.5 {
                                    0.5
                                } else if this.strength > 0. {
                                    0.
                                } else {
                                    1.
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("add", "Add stop").on_click(cx.listener(|this, _, _, cx| {
                            let mut stops = this.palette.stops().to_vec();
                            let index = stops
                                .windows(2)
                                .enumerate()
                                .max_by(|(_, a), (_, b)| {
                                    (a[1].percentage - a[0].percentage)
                                        .total_cmp(&(b[1].percentage - b[0].percentage))
                                })
                                .unwrap()
                                .0;
                            let position =
                                (stops[index].percentage + stops[index + 1].percentage) * 0.5;
                            stops.insert(
                                index + 1,
                                linear_color_stop(
                                    rgb(COLORS[stops.len() % COLORS.len()]),
                                    position,
                                ),
                            );
                            this.selected = index + 1;
                            this.palette = GradientMapPalette::new(stops);
                            this.dragging = false;
                            cx.notify();
                        })),
                    )
                    .child(button("remove", "Remove stop").on_click(cx.listener(
                        |this, _, _, cx| {
                            if this.palette.stops().len() > 2 {
                                let mut stops = this.palette.stops().to_vec();
                                stops.remove(this.selected);
                                this.selected = this.selected.min(stops.len() - 1);
                                this.palette = GradientMapPalette::new(stops);
                                this.dragging = false;
                                cx.notify();
                            }
                        },
                    )))
                    .child(
                        button("reset", "Reset").on_click(cx.listener(|this, _, _, cx| {
                            this.palette = palette();
                            this.selected = 2;
                            this.strength = 1.;
                            this.dragging = false;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .rounded(px(18.))
                    .bg(rgb(0x171e2b))
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(12.))
                            .text_color(rgb(0xa5b4c9))
                            .child("SHADOWS")
                            .child(format!("{} STOPS", self.palette.stops().len()))
                            .child("HIGHLIGHTS"),
                    )
                    .child(
                        div()
                            .relative()
                            .h(px(40.))
                            .w_full()
                            .rounded(px(8.))
                            .bg(self.palette.background())
                            .child(
                                canvas(
                                    move |bounds, _, _| ramp_bounds.set(bounds),
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .inset_0(),
                            )
                            .children(self.palette.stops().iter().enumerate().map(
                                |(index, stop)| {
                                    div()
                                        .id(("stop", index))
                                        .absolute()
                                        .left(relative(stop.percentage))
                                        .top(px(8.))
                                        .ml(px(-12.))
                                        .size(px(24.))
                                        .rounded_full()
                                        .border_2()
                                        .border_color(rgb(0xffffff))
                                        .bg(stop.color)
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(0xffffff))
                                        .text_size(px(10.))
                                        .when(index == self.selected, |s| s.child("●"))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.selected = index;
                                                this.dragging = true;
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(10.))
                            .items_center()
                            .child(div().text_size(px(12.)).text_color(rgb(0xa5b4c9)).child(
                                format!(
                                    "Stop {} · {:.0}%",
                                    self.selected + 1,
                                    stop.percentage * 100.
                                ),
                            ))
                            .children(COLORS.into_iter().enumerate().map(|(index, color)| {
                                div()
                                    .id(("color", index))
                                    .size(px(26.))
                                    .rounded_full()
                                    .bg(rgb(color))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut stop = this.palette.stops()[this.selected];
                                        stop.color = rgb(color).into();
                                        this.palette.set_stop(this.selected, stop);
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(22.))
                    .children((0..3).map(|mode| self.card(mode))),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.), px(890.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Gradient map");
                cx.new(|_| Preview {
                    image: Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        include_bytes!("album-cover.svg").to_vec(),
                    ))
                    .into(),
                    palette: palette(),
                    selected: 2,
                    dragging: false,
                    ramp_bounds: Rc::new(Cell::new(Bounds::default())),
                    strength: 1.,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
