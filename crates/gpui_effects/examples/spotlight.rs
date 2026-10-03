use std::{cell::Cell, rc::Rc, time::Instant};

use gpui::{
    App, Bounds, Context, MouseMoveEvent, Pixels, Point, Render, Window, WindowBounds,
    WindowOptions, canvas, div, point, prelude::*, px, rgb, rgba, size,
};
use gpui_effects::{SpotlightOptions, spotlight};
use gpui_platform::application;

struct Light {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    target: Point<f32>,
    center: Point<f32>,
    hovered: bool,
    strength: f32,
    clicks: usize,
}

impl Light {
    fn new() -> Self {
        Self {
            bounds: Rc::new(Cell::new(Bounds::default())),
            target: point(0.5, 0.5),
            center: point(0.5, 0.5),
            hovered: false,
            strength: 0.,
            clicks: 0,
        }
    }

    fn track(&mut self, position: Point<Pixels>) {
        let bounds = self.bounds.get();
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let local = position - bounds.origin;
        self.target = point(
            f32::from(local.x) / f32::from(bounds.size.width),
            f32::from(local.y) / f32::from(bounds.size.height),
        );
        if self.strength < 0.001 {
            self.center = self.target;
        }
    }

    fn advance(&mut self, dt: f32) -> bool {
        let blend = 1. - (-18. * dt).exp();
        self.center = self.center + (self.target - self.center) * blend;
        let target = if self.hovered { 1. } else { 0. };
        let fade = 1. - (-(if self.hovered { 14. } else { 7. }) * dt).exp();
        self.strength += (target - self.strength) * fade;
        let bounds = self.bounds.get();
        let delta = self.target - self.center;
        let moving = delta.x.abs() * f32::from(bounds.size.width) > 0.1
            || delta.y.abs() * f32::from(bounds.size.height) > 0.1;
        let fading = (target - self.strength).abs() > 0.001;
        if !moving {
            self.center = self.target;
        }
        if !fading {
            self.strength = target;
        }
        fading || (moving && self.strength > 0.)
    }
}

struct Preview {
    lights: [Light; 3],
    last_frame: Instant,
    radius: Pixels,
    surface: bool,
}

impl Preview {
    fn new() -> Self {
        Self {
            lights: std::array::from_fn(|_| Light::new()),
            last_frame: Instant::now(),
            radius: px(180.),
            surface: true,
        }
    }

    fn card(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let (number, title, detail, color) = [
            ("01", "Canvas", "Space for your next idea.", 0xa2b8ff),
            ("02", "Library", "Keep inspiration close.", 0x77dfc2),
            ("03", "Publish", "Ready when you are.", 0xffc38c),
        ][index];
        let light = &self.lights[index];
        let bounds = light.bounds.clone();
        spotlight(SpotlightOptions {
            center: light.center,
            strength: light.strength,
            color: rgb(color),
            radius: self.radius,
            surface_opacity: if self.surface { 0.16 } else { 0. },
            ..Default::default()
        })
        .id(("spotlight-card", index))
        .relative()
        .flex_1()
        .min_w(px(230.))
        .h(px(290.))
        .p(px(24.))
        .rounded(px(22.))
        .border_1()
        .border_color(rgba(0xffffff16))
        .bg(rgb(0x151a24))
        .flex()
        .flex_col()
        .gap(px(14.))
        .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
            this.lights[index].hovered = *hovered;
            if *hovered {
                this.lights[index].track(window.mouse_position());
            }
            this.last_frame = Instant::now();
            cx.notify();
        }))
        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
            this.lights[index].track(event.position);
            cx.notify();
        }))
        .child(
            canvas(move |region, _, _| bounds.set(region), |_, _, _, _| {})
                .absolute()
                .inset_0(),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x737f96))
                        .child(number),
                )
                .child(div().size(px(8.)).rounded_full().bg(rgb(color))),
        )
        .child(div().mt(px(26.)).text_size(px(26.)).child(title))
        .child(
            div()
                .text_size(px(13.))
                .text_color(rgb(0x96a2b8))
                .child(detail),
        )
        .child(div().flex_1())
        .child(
            div()
                .id(("open", index))
                .automation_id(format!("open-{index}"))
                .w_full()
                .px(px(14.))
                .py(px(11.))
                .rounded(px(10.))
                .border_1()
                .border_color(rgba(0xffffff18))
                .bg(rgba(0xffffff05))
                .text_size(px(13.))
                .cursor_pointer()
                .hover(|style| style.bg(rgba(0xffffff12)))
                .child(if light.clicks == 0 {
                    "Explore  →".into()
                } else {
                    format!("Opened {} times", light.clicks)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.lights[index].clicks += 1;
                    cx.notify();
                })),
        )
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        let mut animating = false;
        for light in &mut self.lights {
            animating |= light.advance(dt);
        }
        if animating {
            window.request_animation_frame();
        }
        let control = |id, label: String| {
            div()
                .id(id)
                .px(px(14.))
                .py(px(10.))
                .rounded(px(10.))
                .bg(rgb(0x202736))
                .text_color(rgb(0xc8d2e6))
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x2b3548)))
                .child(label)
        };
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("INTERACTIVE LIGHT"),
            )
            .child(div().text_size(px(38.)).child("A little light."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Move across a card. Follow the light to its edge."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        control(
                            "surface",
                            if self.surface {
                                "Surface + edge"
                            } else {
                                "Edge only"
                            }
                            .into(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.surface = !this.surface;
                            cx.notify();
                        })),
                    )
                    .child(
                        control("radius", format!("Radius  {:.0}px", f32::from(self.radius)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.radius = if this.radius >= px(260.) {
                                    px(100.)
                                } else {
                                    this.radius + px(40.)
                                };
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap(px(16.))
                    .children((0..3).map(|i| self.card(i, cx).into_any_element())),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(940.), px(630.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Spotlight");
                cx.new(|_| Preview::new())
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{MouseDownEvent, MouseUpEvent, PlatformInput, TestAppContext};

    #[gpui::test]
    fn light_preserves_child_clicks_and_settles_after_pointer_exit(cx: &mut TestAppContext) {
        let handle = cx.open_window(size(px(940.), px(630.)), |_, _| Preview::new());
        cx.update_window(handle.into(), |_, window, cx| {
            window.set_automation_enabled(true).unwrap();
            window.draw(cx).clear();
            let snapshot = window.automation_snapshot().unwrap();
            let button = snapshot
                .nodes
                .iter()
                .find(|node| node.automation_id.as_deref() == Some("open-0"))
                .unwrap()
                .bounds
                .unwrap();
            let position = button.center();
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.draw(cx).clear();
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.draw(cx).clear();
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.draw(cx).clear();
        })
        .unwrap();
        handle
            .update(cx, |view, _, _| {
                assert_eq!(view.lights[0].clicks, 1);
                assert!(view.lights[0].hovered);
                for _ in 0..120 {
                    view.lights[0].advance(1. / 60.);
                }
                assert_eq!(view.lights[0].strength, 1.);
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(5.), px(5.)),
                    ..Default::default()
                }),
                cx,
            );
            window.draw(cx).clear();
        })
        .unwrap();
        handle
            .update(cx, |view, _, _| {
                assert!(!view.lights[0].hovered);
                for _ in 0..120 {
                    view.lights[0].advance(1. / 60.);
                }
                assert_eq!(view.lights[0].strength, 0.);
                assert!(
                    !view.lights[0].advance(1. / 60.),
                    "idle light must stop scheduling frames"
                );
            })
            .unwrap();
    }
}
