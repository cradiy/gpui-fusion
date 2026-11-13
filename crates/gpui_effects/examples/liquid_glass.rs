use gpui::{
    App, Bounds, Context, CursorStyle, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent,
    Pixels, Point, Render, Window, WindowBounds, WindowOptions, div, linear_color_stop,
    multi_linear_gradient, point, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{LiquidGlass, LiquidGlassMerge, LiquidGlassStyle};
use gpui_platform::application;

const GLASS_WIDTH: f32 = 300.0;
const GLASS_HEIGHT: f32 = 220.0;
const SMALL_GLASS_WIDTH: f32 = 200.0;
const SMALL_GLASS_HEIGHT: f32 = 200.0;

#[derive(Clone, Copy)]
enum GlassTarget {
    Large,
    Small,
}

struct GlassStudy {
    appearance: LiquidGlassStyle,
    radius: f32,
    position: Point<Pixels>,
    small_position: Point<Pixels>,
    drag_offset: Option<(GlassTarget, Point<Pixels>)>,
    grid: bool,
    dark_background: bool,
    contrast_shapes: bool,
}

impl GlassStudy {
    fn new() -> Self {
        Self {
            appearance: LiquidGlassStyle {
                shape: gpui_effects::LiquidGlassShape {
                    roundness: 5.,
                    merge: Some(LiquidGlassMerge {
                        offset: point(px(0.), px(0.)),
                        radius: px(100.),
                        smoothness: px(40.),
                    }),
                },
                ..LiquidGlassStyle::optical()
            },
            radius: 88.0,
            position: point(px(480.0), px(245.0)),
            small_position: point(px(320.0), px(285.0)),
            drag_offset: None,
            grid: true,
            dark_background: false,
            contrast_shapes: true,
        }
    }

    fn controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |id: &'static str, label: String| {
            div()
                .id(id)
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(rgba(0xffffffcc))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0xffffff)))
                .child(label)
        };
        div()
            .id("material-controls")
            .absolute()
            .left(px(32.0))
            .top(px(32.0))
            .w(px(300.0))
            .max_h(gpui::relative(0.92))
            .overflow_y_scroll()
            .p_5()
            .rounded(px(20.0))
            .bg(rgba(0xf0f4f9f5))
            .text_color(rgb(0x21334a))
            .text_sm()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("LIQUID GLASS / MATERIAL STUDY"),
                    )
                    .child("Drag the glass across the background.")
                    .child(
                        button(
                            "contrast-shapes",
                            format!(
                                "Checkerboard: {}",
                                if self.contrast_shapes { "on" } else { "off" }
                            ),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.contrast_shapes = !this.contrast_shapes;
                            cx.notify();
                        })),
                    )
                    .child(
                        div().flex().gap_2().children(
                            ["Optical", "Frosted", "Clear", "Dark"]
                                .into_iter()
                                .map(|preset| {
                                    button(preset, preset.into()).on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            let shape = this.appearance.shape;
                                            this.appearance = match preset {
                                                "Optical" => LiquidGlassStyle::optical(),
                                                "Clear" => LiquidGlassStyle::clear(),
                                                "Dark" => LiquidGlassStyle::dark(),
                                                _ => LiquidGlassStyle::frosted(),
                                            };
                                            this.appearance.shape = shape;
                                            cx.notify();
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(
                        button(
                            "blur-edge",
                            format!("Blur rim: {}", self.appearance.optics.blur_edge),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.appearance.optics.blur_edge = !this.appearance.optics.blur_edge;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "merge-circle",
                            format!("Merged circle: {}", self.appearance.shape.merge.is_some()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.appearance.shape.merge = if this.appearance.shape.merge.is_some() {
                                None
                            } else {
                                Some(LiquidGlassMerge {
                                    offset: point(px(-180.), px(-100.)),
                                    radius: px(72.),
                                    smoothness: px(40.),
                                })
                            };
                            cx.notify();
                        })),
                    )
                    .children(Knob::ALL.into_iter().map(|knob| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().child(format!(
                                "{} {:.2}",
                                knob.label(),
                                knob.value(self)
                            )))
                            .child(
                                div()
                                    .id(("less", knob as usize))
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(rgba(0xffffffcc))
                                    .cursor_pointer()
                                    .child("−")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        knob.adjust(this, -1.0);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id(("more", knob as usize))
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(rgba(0xffffffcc))
                                    .cursor_pointer()
                                    .child("+")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        knob.adjust(this, 1.0);
                                        cx.notify();
                                    })),
                            )
                    }))
                    .child(
                        button(
                            "grid",
                            format!("Grid: {}", if self.grid { "on" } else { "off" }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.grid = !this.grid;
                            cx.notify();
                        })),
                    )
                    .child(
                        button("background", "Switch background".into()).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.dark_background = !this.dark_background;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        "Refraction = 0 disables bending.\nDispersion = 0 disables RGB separation.",
                    ),
            )
    }
}

#[derive(Clone, Copy)]
enum Knob {
    Blur,

    Refraction,
    Thickness,

    Dispersion,

    Radius,
    Index,
    Roundness,
    FresnelRange,
    FresnelHardness,
    FresnelStrength,
    GlareRange,
    GlareHardness,
    GlareStrength,
    GlareConvergence,
    GlareOpposite,
    GlareAngle,
    ShadowSpread,
    ShadowStrength,
}

impl Knob {
    const ALL: [Self; 18] = [
        Self::Blur,
        Self::Refraction,
        Self::Thickness,
        Self::Dispersion,
        Self::Radius,
        Self::Index,
        Self::Roundness,
        Self::FresnelRange,
        Self::FresnelHardness,
        Self::FresnelStrength,
        Self::GlareRange,
        Self::GlareHardness,
        Self::GlareStrength,
        Self::GlareConvergence,
        Self::GlareOpposite,
        Self::GlareAngle,
        Self::ShadowSpread,
        Self::ShadowStrength,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Blur => "Blur (px)",

            Self::Refraction => "Refraction (px)",
            Self::Thickness => "Thickness (px)",

            Self::Dispersion => "Dispersion",

            Self::Radius => "Corner radius (px)",
            Self::Index => "Refraction index",
            Self::Roundness => "Corner exponent",
            Self::FresnelRange => "Fresnel range",
            Self::FresnelHardness => "Fresnel hardness",
            Self::FresnelStrength => "Fresnel strength",
            Self::GlareRange => "Glare range",
            Self::GlareHardness => "Glare hardness",
            Self::GlareStrength => "Glare strength",
            Self::GlareConvergence => "Glare convergence",
            Self::GlareOpposite => "Opposite glare",
            Self::GlareAngle => "Glare angle (radians)",
            Self::ShadowSpread => "Shadow spread",
            Self::ShadowStrength => "Shadow strength",
        }
    }

    fn value(self, study: &GlassStudy) -> f32 {
        match self {
            Self::Blur => study.appearance.blur_radius.as_f32(),

            Self::Refraction => study.appearance.refraction.as_f32(),
            Self::Thickness => study.appearance.thickness.as_f32(),

            Self::Dispersion => study.appearance.dispersion,

            Self::Radius => study.radius,
            Self::Index => study.appearance.optics.refractive_index,
            Self::Roundness => study.appearance.shape.roundness,
            Self::FresnelRange => study.appearance.optics.fresnel_range.as_f32(),
            Self::FresnelHardness => study.appearance.optics.fresnel_hardness,
            Self::FresnelStrength => study.appearance.optics.fresnel_strength,
            Self::GlareRange => study.appearance.optics.glare_range.as_f32(),
            Self::GlareHardness => study.appearance.optics.glare_hardness,
            Self::GlareStrength => study.appearance.optics.glare_strength,
            Self::GlareConvergence => study.appearance.optics.glare_convergence,
            Self::GlareOpposite => study.appearance.optics.glare_opposite,
            Self::GlareAngle => study.appearance.optics.glare_angle,
            Self::ShadowSpread => study.appearance.shadow.spread.as_f32(),
            Self::ShadowStrength => study.appearance.shadow.strength,
        }
    }

    fn adjust(self, study: &mut GlassStudy, direction: f32) {
        let (step, max) = match self {
            Self::Blur => (2.0, 200.0),

            Self::Refraction => (5.0, 280.0),
            Self::Thickness => (2.0, 64.0),
            Self::Dispersion => (0.02, 1.0),
            Self::Radius => (8.0, GLASS_HEIGHT * 0.5),
            Self::Index => (0.1, 4.0),
            Self::Roundness => (0.5, 7.0),
            Self::FresnelRange => (5.0, 100.0),
            Self::FresnelHardness => (0.1, 1.0),
            Self::FresnelStrength => (0.1, 1.0),
            Self::GlareRange => (5.0, 100.0),
            Self::GlareHardness => (0.1, 1.0),
            Self::GlareStrength => (0.1, 1.2),
            Self::GlareConvergence => (0.1, 1.0),
            Self::GlareOpposite => (0.1, 1.0),
            Self::GlareAngle => (0.2, 6.2831855),
            Self::ShadowSpread => (5.0, 100.0),
            Self::ShadowStrength => (0.05, 1.0),
        };
        let min = match self {
            Self::Index => 1.0,
            Self::Roundness => 2.0,
            Self::GlareAngle => -std::f32::consts::PI,
            _ => 0.0,
        };
        let value = (self.value(study) + step * direction).clamp(min, max);
        match self {
            Self::Blur => study.appearance.blur_radius = px(value),

            Self::Refraction => study.appearance.refraction = px(value),
            Self::Thickness => study.appearance.thickness = px(value),

            Self::Dispersion => study.appearance.dispersion = value,

            Self::Radius => study.radius = value,
            Self::Index => study.appearance.optics.refractive_index = value,
            Self::Roundness => study.appearance.shape.roundness = value,
            Self::FresnelRange => study.appearance.optics.fresnel_range = px(value),
            Self::FresnelHardness => study.appearance.optics.fresnel_hardness = value,
            Self::FresnelStrength => study.appearance.optics.fresnel_strength = value,
            Self::GlareRange => study.appearance.optics.glare_range = px(value),
            Self::GlareHardness => study.appearance.optics.glare_hardness = value,
            Self::GlareStrength => study.appearance.optics.glare_strength = value,
            Self::GlareConvergence => study.appearance.optics.glare_convergence = value,
            Self::GlareOpposite => study.appearance.optics.glare_opposite = value,
            Self::GlareAngle => study.appearance.optics.glare_angle = value,
            Self::ShadowSpread => study.appearance.shadow.spread = px(value),
            Self::ShadowStrength => study.appearance.shadow.strength = value,
        }
    }
}

impl Render for GlassStudy {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut material = self.appearance;
        if let Some(circle) = material.shape.merge.as_mut() {
            circle.offset = self.small_position
                + point(px(SMALL_GLASS_WIDTH * 0.5), px(SMALL_GLASS_HEIGHT * 0.5))
                - (self.position + point(px(GLASS_WIDTH * 0.5), px(GLASS_HEIGHT * 0.5)));
            circle.radius = px(SMALL_GLASS_WIDTH * 0.5);
        }
        let viewport = window.viewport_size();
        let background = if self.dark_background {
            [rgb(0x13243a), rgb(0x354f69), rgb(0x182c3c)]
        } else {
            [rgb(0xe9d4c7), rgb(0xb7d7e0), rgb(0xa6bbdc)]
        };
        let ink = if self.dark_background {
            rgba(0xffffff48)
        } else {
            rgba(0x29425c50)
        };
        div()
            .id("glass-study")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(multi_linear_gradient(
                125.0,
                [
                    linear_color_stop(background[0], 0.0),
                    linear_color_stop(background[1], 0.5),
                    linear_color_stop(background[2], 1.0),
                ],
            ))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    if this.drag_offset.take().is_some() {
                        cx.notify();
                    }
                    return;
                }
                if let Some((target, offset)) = this.drag_offset {
                    let viewport = window.viewport_size();
                    let position = event.position - offset;
                    let (width, height, target_position) = match target {
                        GlassTarget::Large => (GLASS_WIDTH, GLASS_HEIGHT, &mut this.position),
                        GlassTarget::Small => (
                            SMALL_GLASS_WIDTH,
                            SMALL_GLASS_HEIGHT,
                            &mut this.small_position,
                        ),
                    };
                    *target_position = point(
                        position
                            .x
                            .clamp(px(0.0), (viewport.width - px(width)).max(px(0.0))),
                        position
                            .y
                            .clamp(px(0.0), (viewport.height - px(height)).max(px(0.0))),
                    );
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag_offset = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(450.0))
                    .top(px(110.0))
                    .size(px(350.0))
                    .rounded_full()
                    .bg(multi_linear_gradient(
                        30.0,
                        [
                            linear_color_stop(rgba(0xffb19eaa), 0.0),
                            linear_color_stop(rgba(0xd598c122), 1.0),
                        ],
                    )),
            )
            .child(
                div()
                    .absolute()
                    .right(px(-100.0))
                    .bottom(px(-180.0))
                    .size(px(650.0))
                    .rounded_full()
                    .bg(multi_linear_gradient(
                        60.0,
                        [
                            linear_color_stop(rgba(0x51a0bb88), 0.0),
                            linear_color_stop(rgba(0x6896e500), 1.0),
                        ],
                    )),
            )
            .when(self.grid, |root| {
                root.children((0..(viewport.width.as_f32() / 48.0) as usize).map(|index| {
                    div()
                        .absolute()
                        .left(px(index as f32 * 48.0))
                        .top_0()
                        .w(px(1.0))
                        .h_full()
                        .bg(ink)
                }))
                .children(
                    (0..(viewport.height.as_f32() / 48.0) as usize).map(|index| {
                        div()
                            .absolute()
                            .top(px(index as f32 * 48.0))
                            .left_0()
                            .h(px(1.0))
                            .w_full()
                            .bg(ink)
                    }),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px(410.0))
                    .top(px(310.0))
                    .text_size(px(104.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(ink)
                    .child("REFRACTION"),
            )
            .when(self.contrast_shapes, |layer| {
                layer.child(
                    gpui::canvas(
                        |_, _, _| (),
                        |region, _, window, _| {
                            let cell = px(24.);
                            let columns = (region.size.width / cell).ceil() as usize;
                            let rows = (region.size.height / cell).ceil() as usize;
                            window.paint_quad(gpui::fill(region, rgb(0xffffff)));
                            for row in 0..rows {
                                for column in 0..columns {
                                    if (row + column) % 2 == 1 {
                                        window.paint_quad(gpui::fill(
                                            Bounds::new(
                                                region.origin
                                                    + point(
                                                        cell * column as f32,
                                                        cell * row as f32,
                                                    ),
                                                size(cell, cell),
                                            ),
                                            rgb(0xbfbfbf),
                                        ));
                                    }
                                }
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
            .child(
                LiquidGlass::with_style(material)
                    .id("test-glass")
                    .absolute()
                    .left(self.position.x)
                    .top(self.position.y)
                    .w(px(GLASS_WIDTH))
                    .h(px(GLASS_HEIGHT))
                    .rounded(px(self.radius))
                    .p_8()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_3()
                    .text_color(if self.appearance.tint.a > 0.2 {
                        rgb(0xffffff)
                    } else {
                        rgb(0x1c3049)
                    })
                    .cursor(
                        if matches!(self.drag_offset, Some((GlassTarget::Large, _))) {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        },
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.drag_offset =
                                Some((GlassTarget::Large, event.position - this.position));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .when(!self.contrast_shapes, |glass| {
                        glass
                            .child("Liquid glass")
                            .child("Drag to inspect the curved edge.")
                    }),
            )
            .when(material.shape.merge.is_some(), |layer| {
                layer.child(
                    div()
                        .id("small-test-glass")
                        .absolute()
                        .left(self.small_position.x)
                        .top(self.small_position.y)
                        .w(px(SMALL_GLASS_WIDTH))
                        .h(px(SMALL_GLASS_HEIGHT))
                        .rounded(px(self.radius))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if self.appearance.tint.a > 0.2 {
                            rgb(0xffffff)
                        } else {
                            rgb(0x1c3049)
                        })
                        .cursor(
                            if matches!(self.drag_offset, Some((GlassTarget::Small, _))) {
                                CursorStyle::ClosedHand
                            } else {
                                CursorStyle::OpenHand
                            },
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.drag_offset = Some((
                                    GlassTarget::Small,
                                    event.position - this.small_position,
                                ));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .child("Drag to merge"),
                )
            })
            .child(self.controls(cx))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| GlassStudy::new()),
        )
        .expect("failed to open liquid-glass material study");
        cx.activate(true);
    });
}
