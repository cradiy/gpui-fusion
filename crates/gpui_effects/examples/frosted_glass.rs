use gpui::{
    App, Bounds, Context, CursorStyle, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, Window, WindowBounds, WindowOptions, div,
    linear_color_stop, multi_linear_gradient, point, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{LiquidGlass, LiquidGlassMerge, LiquidGlassStyle};
use gpui_platform::application;

const FIELD_LEFT: f32 = 70.0;
const FIELD_TOP: f32 = 90.0;
const FIELD_WIDTH: f32 = 960.0;
const FIELD_HEIGHT: f32 = 560.0;
const BAR_X: f32 = 410.0;
const BAR_Y: f32 = 370.0;
const BAR_WIDTH: f32 = 340.0;
const CONTROL_SIZE: f32 = 96.0;

struct FrostedGlassDemo {
    light: bool,
    dragging: bool,
    control_center: Point<Pixels>,
    drag_offset: Point<Pixels>,
}

impl FrostedGlassDemo {
    fn new() -> Self {
        Self {
            light: false,
            dragging: false,
            control_center: point(px(630.0), px(BAR_Y)),
            drag_offset: Point::default(),
        }
    }

    fn begin_drag(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        self.dragging = true;
        let global_center = point(
            px(FIELD_LEFT) + self.control_center.x,
            px(FIELD_TOP) + self.control_center.y,
        );
        self.drag_offset = pointer - global_center;
        cx.notify();
    }

    fn update_drag(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        self.control_center = point(
            (pointer.x - px(FIELD_LEFT) - self.drag_offset.x)
                .clamp(px(CONTROL_SIZE * 0.5), px(FIELD_WIDTH - CONTROL_SIZE * 0.5)),
            (pointer.y - px(FIELD_TOP) - self.drag_offset.y).clamp(
                px(CONTROL_SIZE * 0.5),
                px(FIELD_HEIGHT - CONTROL_SIZE * 0.5),
            ),
        );
        cx.notify();
    }

    fn appearance(&self) -> LiquidGlassStyle {
        if self.light {
            LiquidGlassStyle::frosted()
        } else {
            LiquidGlassStyle::dark()
        }
    }

    fn background() -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .bg(multi_linear_gradient(
                135.0,
                [
                    linear_color_stop(rgb(0x0b1220), 0.0),
                    linear_color_stop(rgb(0x25314c), 0.52),
                    linear_color_stop(rgb(0x111827), 1.0),
                ],
            ))
            .child(
                div()
                    .absolute()
                    .left(px(-90.0))
                    .top(px(-160.0))
                    .size(px(520.0))
                    .rounded_full()
                    .bg(multi_linear_gradient(
                        128.0,
                        [
                            linear_color_stop(rgba(0xff826acc), 0.0),
                            linear_color_stop(rgba(0xb43b8db0), 1.0),
                        ],
                    )),
            )
            .child(
                div()
                    .absolute()
                    .right(px(-120.0))
                    .bottom(px(-190.0))
                    .size(px(560.0))
                    .rounded_full()
                    .bg(multi_linear_gradient(
                        42.0,
                        [
                            linear_color_stop(rgba(0x2764e8d8), 0.0),
                            linear_color_stop(rgba(0x27d4c6a8), 1.0),
                        ],
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left(px(105.0))
                    .top(px(52.0))
                    .text_size(px(54.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgba(0xf5f8ffff))
                    .child("FROSTED SURFACES"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(108.0))
                    .top(px(122.0))
                    .text_lg()
                    .text_color(rgba(0xdbe8f4bb))
                    .child("Drag the round control away, then bring it back to merge."),
            )
            .child(
                div()
                    .absolute()
                    .left(px(120.0))
                    .top(px(420.0))
                    .text_size(px(58.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgba(0xf7fbff9c))
                    .child("TEXT BEHIND THE GLASS"),
            )
            .children((0..12).map(|row| {
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(row as f32 * 62.0))
                    .h(px(1.0))
                    .bg(rgba(0xffffff0c))
            }))
    }

    fn mode_switch(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.light;
        div()
            .absolute()
            .right(px(42.0))
            .top(px(38.0))
            .p(px(8.0))
            .rounded(px(18.0))
            .bg(rgba(0x091221dd))
            .flex()
            .gap(px(6.0))
            .children([(false, "Dark"), (true, "Light")].map(|(value, label)| {
                let selected = value == light;
                div()
                    .id(label)
                    .px(px(16.0))
                    .h(px(38.0))
                    .rounded(px(12.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(if selected {
                        rgba(0xe8f1f8ff)
                    } else {
                        rgba(0xffffff10)
                    })
                    .text_color(if selected {
                        rgba(0x142033ff)
                    } else {
                        rgba(0xe9f2f8cc)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.light = value;
                        cx.notify();
                    }))
                    .child(label)
            }))
    }

    fn glass(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut material = self.appearance();
        material.shape.merge = Some(LiquidGlassMerge {
            offset: self.control_center - point(px(BAR_X), px(BAR_Y)),
            radius: px(CONTROL_SIZE * 0.5),
            smoothness: px(44.),
        });
        let control_left = self.control_center.x - px(CONTROL_SIZE * 0.5);
        let control_top = self.control_center.y - px(CONTROL_SIZE * 0.5);

        div()
            .absolute()
            .left(px(FIELD_LEFT))
            .top(px(FIELD_TOP))
            .w(px(FIELD_WIDTH))
            .h(px(FIELD_HEIGHT))
            .child(
                LiquidGlass::with_style(material)
                    .absolute()
                    .left(px(BAR_X - BAR_WIDTH * 0.5))
                    .top(px(BAR_Y - CONTROL_SIZE * 0.5))
                    .w(px(BAR_WIDTH))
                    .h(px(CONTROL_SIZE))
                    .rounded(px(CONTROL_SIZE * 0.5)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(BAR_X - BAR_WIDTH * 0.5))
                    .top(px(BAR_Y - CONTROL_SIZE * 0.5))
                    .w(px(BAR_WIDTH))
                    .h(px(CONTROL_SIZE))
                    .px(px(30.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .font_weight(FontWeight::SEMIBOLD)
                    .children(["Files", "Edit", "View"]),
            )
            .child(
                div()
                    .id("draggable-frosted-control")
                    .absolute()
                    .left(control_left)
                    .top(control_top)
                    .size(px(CONTROL_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(34.0))
                    .font_weight(FontWeight::NORMAL)
                    .cursor(if self.dragging {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::OpenHand
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.begin_drag(event.position, cx);
                        }),
                    )
                    .child("+"),
            )
    }

    fn layout_panel(&self) -> impl IntoElement {
        LiquidGlass::with_style(self.appearance())
            .absolute()
            .right(px(44.0))
            .top(px(150.0))
            .w(px(330.0))
            .p(px(22.0))
            .rounded(px(24.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("DIV-EQUIVALENT PANEL"),
            )
            .child(
                div().text_sm().opacity(0.74).child(
                    "Width, padding, flex, gap, radius, and children use normal GPUI layout.",
                ),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .flex()
                    .gap(px(8.0))
                    .children(["Accept", "Cancel"].map(|label| {
                        div()
                            .px(px(14.0))
                            .h(px(34.0))
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(rgba(0xffffff35))
                            .flex()
                            .items_center()
                            .child(label)
                    })),
            )
    }
}

impl Render for FrostedGlassDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .text_color(if self.light {
                rgb(0x1f2937)
            } else {
                rgb(0xf8fafc)
            })
            .overflow_hidden()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging && event.dragging() {
                    this.update_drag(event.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    this.dragging = false;
                    cx.notify();
                }),
            )
            .child(Self::background())
            .child(self.glass(cx))
            .child(self.layout_panel())
            .child(self.mode_switch(cx))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.0), px(720.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| FrostedGlassDemo::new()),
        )
        .expect("failed to open frosted-glass demo");
        cx.activate(true);
    });
}
