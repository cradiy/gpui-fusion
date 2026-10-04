use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Bounds, Context, MouseButton, MouseMoveEvent, Pixels, Point, Render, Window, WindowBounds,
    WindowOptions, canvas, div, point, prelude::*, px, relative, rgb, rgba, size,
};
use gpui_effects::{GradientPoint, point_gradient};
use gpui_platform::application;

const PALETTE: [u32; 8] = [
    0xffb178, 0xf36c98, 0xb781ed, 0x6053cc, 0x629bff, 0x91cce5, 0x8ad6b3, 0xf5dfad,
];

fn initial_points() -> [GradientPoint; 4] {
    [
        GradientPoint::new(point(0.15, 0.2), rgb(PALETTE[0])),
        GradientPoint::new(point(0.8, 0.15), rgb(PALETTE[1])),
        GradientPoint::new(point(0.25, 0.85), rgb(PALETTE[3])),
        GradientPoint::new(point(0.85, 0.8), rgb(PALETTE[5])),
    ]
}

struct Preview {
    points: [GradientPoint; 4],
    selected: usize,
    dragging: bool,
    handles: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Preview {
    fn new() -> Self {
        Self {
            points: initial_points(),
            selected: 0,
            dragging: false,
            handles: true,
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    fn move_point(&mut self, position: Point<Pixels>) {
        let bounds = self.bounds.get();
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let local = position - bounds.origin;
        self.points[self.selected].position = point(
            (f32::from(local.x) / f32::from(bounds.size.width)).clamp(0., 1.),
            (f32::from(local.y) / f32::from(bounds.size.height)).clamp(0., 1.),
        );
    }
}

fn button(id: &'static str, label: impl Into<gpui::SharedString>) -> gpui::Stateful<gpui::Div> {
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

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.bounds.clone();
        let selected = self.points[self.selected];
        let surface = point_gradient(self.points)
            .id("gradient-canvas")
            .relative()
            .w_full()
            .h(px(440.))
            .rounded(px(24.))
            .bg(rgb(0x17202d))
            .cursor_crosshair()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if this.handles {
                        this.dragging = true;
                        this.move_point(event.position);
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(move |region, _, _| bounds.set(region), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(32.))
                    .top(px(30.))
                    .text_size(px(12.))
                    .text_color(rgba(0xffffffbb))
                    .child("YOUR COLOR STUDY"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(32.))
                    .bottom(px(30.))
                    .right(px(32.))
                    .text_color(rgb(0xffffff))
                    .text_size(px(42.))
                    .flex()
                    .flex_col()
                    .child("A little space.")
                    .child("Endless possibility."),
            )
            .when(self.handles, |surface| {
                surface.children(self.points.iter().enumerate().map(|(index, source)| {
                    div()
                        .id(("point", index))
                        .absolute()
                        .left(relative(source.position.x))
                        .top(relative(source.position.y))
                        .ml(px(-13.))
                        .mt(px(-13.))
                        .size(px(26.))
                        .rounded_full()
                        .border_2()
                        .border_color(rgb(0xffffff))
                        .bg(source.color)
                        .shadow_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.))
                        .text_color(rgb(0x182035))
                        .cursor_pointer()
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
                }))
            });

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
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    if event.dragging() {
                        this.move_point(event.position);
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
                    .child("POINT GRADIENT"),
            )
            .child(div().text_size(px(38.)).child("Color, placed by you."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Drag a color point. Shape the blend. Make it yours."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "handles",
                            if self.handles {
                                "Hide handles"
                            } else {
                                "Show handles"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.handles = !this.handles;
                            this.dragging = false;
                            cx.notify();
                        })),
                    )
                    .child(
                        button("reset", "Reset").on_click(cx.listener(|this, _, _, cx| {
                            this.points = initial_points();
                            this.selected = 0;
                            this.dragging = false;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .items_start()
                    .child(div().flex_1().min_w(px(340.)).child(surface))
                    .child(
                        div()
                            .w(px(240.))
                            .p(px(22.))
                            .rounded(px(20.))
                            .bg(rgb(0x151a24))
                            .border_1()
                            .border_color(rgba(0xffffff12))
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            .child(div().text_size(px(16.)).child("Color points"))
                            .child(div().flex().gap(px(10.)).children(
                                self.points.iter().enumerate().map(|(index, source)| {
                                    div()
                                        .id(("select", index))
                                        .size(px(38.))
                                        .rounded(px(12.))
                                        .bg(source.color)
                                        .border_2()
                                        .border_color(if index == self.selected {
                                            rgba(0xffffffff)
                                        } else {
                                            rgba(0xffffff22)
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selected = index;
                                            cx.notify();
                                        }))
                                }),
                            ))
                            .child(div().text_size(px(12.)).text_color(rgb(0x929fbc)).child(
                                format!(
                                    "POINT {:02}  ·  {:.0}%, {:.0}%",
                                    self.selected + 1,
                                    selected.position.x * 100.,
                                    selected.position.y * 100.
                                ),
                            ))
                            .child(div().flex().flex_wrap().gap(px(10.)).children(
                                PALETTE.into_iter().enumerate().map(|(index, color)| {
                                    div()
                                        .id(("color", index))
                                        .size(px(35.))
                                        .rounded_full()
                                        .bg(rgb(color))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let alpha = this.points[this.selected].color.a;
                                            this.points[this.selected].color = rgb(color);
                                            this.points[this.selected].color.a = alpha;
                                            cx.notify();
                                        }))
                                }),
                            ))
                            .child(div().h(px(1.)).bg(rgba(0xffffff12)))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .child(format!("Influence  {:.0}%", selected.radius * 100.)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .child(button("smaller", "−").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            let radius = &mut this.points[this.selected].radius;
                                            *radius = (*radius - 0.1).max(0.1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(button("larger", "+").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            let radius = &mut this.points[this.selected].radius;
                                            *radius = (*radius + 0.1).min(2.);
                                            cx.notify();
                                        },
                                    ))),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .child(format!("Opacity  {:.0}%", selected.color.a * 100.)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .child(button("fainter", "−").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            let alpha = &mut this.points[this.selected].color.a;
                                            *alpha = (*alpha - 0.1).max(0.);
                                            cx.notify();
                                        },
                                    )))
                                    .child(button("stronger", "+").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            let alpha = &mut this.points[this.selected].color.a;
                                            *alpha = (*alpha + 0.1).min(1.);
                                            cx.notify();
                                        },
                                    ))),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x79879f))
                    .child("Four points. One continuous surface."),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(780.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Point gradient");
                cx.new(|_| Preview::new())
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
