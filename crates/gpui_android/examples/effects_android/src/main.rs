use gpui::{prelude::*, *};
use gpui_effects::{
    BloomOptions, BorderTrailOptions, EffectStage, Fluid, FluidOptions, FluidSplat, LiquidGlass,
    ParticleSpawn, Particles, border_trail, subtree_effect_chain,
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

struct Effects {
    tab: usize,
    particles: Particles,
    fluid: Fluid,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    touch: Option<TouchId>,
    previous: Option<Point<Pixels>>,
    last_frame: Instant,
    time: Duration,
    paused: bool,
    bloom: bool,
}

impl Effects {
    fn new() -> Self {
        Self {
            tab: 0,
            particles: Particles::new(4096),
            fluid: Fluid::new(FluidOptions {
                resolution: 128,
                pressure_iterations: 12,
                ..Default::default()
            }),
            bounds: Default::default(),
            touch: None,
            previous: None,
            last_frame: Instant::now(),
            time: Duration::ZERO,
            paused: false,
            bloom: true,
        }
    }

    fn inject(&mut self, position: Point<Pixels>) {
        let local = position - self.bounds.get().origin;
        let from = self.previous.unwrap_or(local);
        if self.tab == 0 {
            self.particles.emit(ParticleSpawn {
                from,
                to: local,
                count: if self.previous.is_none() { 120 } else { 24 },
                color: rgb(0x9ae9ff),
                speed: px(25.)..px(160.),
                stretch: 0.025,
                ..Default::default()
            });
        } else if self.tab == 1 {
            self.fluid.splat(FluidSplat {
                from,
                to: local,
                velocity: (local - from) * 35.,
                radius: px(26.),
                amount: 1.2,
                color: rgb(0x9b9aff),
            });
        }
        self.previous = Some(local);
    }

    fn surface(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.tab == 2 {
            let compact = window.viewport_size().height < px(500.);
            if !self.paused {
                window.request_animation_frame();
            }
            return div()
                .relative()
                .size_full()
                .overflow_hidden()
                .rounded_xl()
                .bg(rgb(0x142441))
                .child(
                    div()
                        .absolute()
                        .top(px(50.))
                        .left(px(30. + self.time.as_secs_f32().sin() * 24.))
                        .size(px(180.))
                        .rounded_full()
                        .bg(rgb(0x415dbd)),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(55.))
                        .right(px(10. + self.time.as_secs_f32().cos() * 20.))
                        .size(px(145.))
                        .rounded_full()
                        .bg(rgb(0x267f89)),
                )
                .children((0..16).map(|index| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(index as f32 * 36.))
                        .h(px(1.))
                        .bg(rgba(0xb4ccff30))
                }))
                .child(
                    border_trail(BorderTrailOptions {
                        progress: self.time.as_secs_f32() / 4.,
                        width: px(3.),
                        length: px(200.),
                        ..Default::default()
                    })
                    .absolute()
                    .left(px(24.))
                    .right(px(24.))
                    .top(px(if compact { 16. } else { 100. }))
                    .h(px(if compact { 128. } else { 200. }))
                    .rounded(px(28.))
                    .child(
                        LiquidGlass::new()
                            .size_full()
                            .rounded(px(28.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child("Liquid glass")
                            .child(div().text_sm().child("Shared WGSL effects")),
                    ),
                )
                .into_any_element();
        }
        let supported = if self.tab == 0 {
            window.supports_gpu_particles()
        } else {
            window.supports_gpu_fluid()
        };
        if !supported {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_5()
                .child("This GPU does not support this simulation.")
                .into_any_element();
        }
        let particles = (self.tab == 0).then(|| self.particles.frame());
        let fluid = (self.tab == 1).then(|| self.fluid.frame());
        let bounds = self.bounds.clone();
        let entity = cx.entity().downgrade();
        let source = canvas(
            move |rect, window, _| {
                bounds.set(rect);
                window.insert_hitbox(rect, HitboxBehavior::Normal)
            },
            move |rect, hitbox, window, _| {
                if let Some(frame) = particles {
                    window.paint_particles(rect, frame);
                }
                if let Some(frame) = fluid {
                    window.paint_fluid(rect, frame);
                }
                window.on_mouse_event(move |event: &TouchEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let _ = entity.update(cx, |this, cx| {
                        if event.phase == TouchPhase::Started
                            && hitbox.is_hovered(window)
                            && this.touch.is_none()
                        {
                            this.touch = Some(event.id);
                            this.previous = None;
                        }
                        if this.touch != Some(event.id) {
                            return;
                        }
                        match event.phase {
                            TouchPhase::Started | TouchPhase::Moved => this.inject(event.position),
                            TouchPhase::Ended | TouchPhase::Cancelled => {
                                this.touch = None;
                                this.previous = None;
                            }
                        }
                        window.prevent_default();
                        cx.notify();
                    });
                });
            },
        )
        .size_full();
        div()
            .id("simulation")
            .size_full()
            .rounded_xl()
            .overflow_hidden()
            .bg(rgb(0x0b1222))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.previous = None;
                    this.inject(event.position);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    this.inject(event.position);
                    cx.notify();
                } else {
                    this.previous = None;
                }
            }))
            .child(subtree_effect_chain(
                source,
                [EffectStage::bloom(BloomOptions {
                    radius: px(12.),
                    intensity: 0.8,
                    ..Default::default()
                })
                .enabled(self.bloom)],
            ))
            .into_any_element()
    }
}

fn button(id: &'static str, label: &'static str, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .px_3()
        .py_2()
        .rounded_lg()
        .text_sm()
        .cursor_pointer()
        .bg(rgb(if selected { 0x365689 } else { 0x1e2c43 }))
        .child(label)
}

impl Render for Effects {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if !self.paused {
            self.time += now
                .duration_since(self.last_frame)
                .min(Duration::from_millis(100));
        }
        self.last_frame = now;
        let insets = window.insets().effective();
        let compact = window.viewport_size().height < px(500.);
        let status = format!(
            "Particles: {} · Fluid: {} · Backdrop: {}",
            window.supports_gpu_particles(),
            window.supports_gpu_fluid(),
            window.supports_backdrop_blur()
        );
        let surface = self.surface(window, cx);
        div()
            .size_full()
            .bg(rgb(0x101a2b))
            .text_color(rgb(0xebf1ff))
            .font_family("sans-serif")
            .pt(insets.top)
            .pb(insets.bottom)
            .pl(insets.left)
            .pr(insets.right)
            .child(
                div()
                    .size_full()
                    .p(px(if compact { 12. } else { 20. }))
                    .flex()
                    .flex_col()
                    .gap(px(if compact { 8. } else { 16. }))
                    .when(!compact, |this| {
                        this.child(div().text_2xl().child("Effects lab"))
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x9aacc8))
                            .whitespace_normal()
                            .child(status),
                    )
                    .child(
                        div().flex().gap_2().children(
                            ["Particles", "Fluid", "Materials"]
                                .into_iter()
                                .enumerate()
                                .map(|(tab, label)| {
                                    button(label, label, self.tab == tab).on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.tab = tab;
                                            this.previous = None;
                                            this.touch = None;
                                            cx.notify();
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(div().flex_1().min_h_0().w_full().child(surface))
                    .when(!compact, |this| {
                        this.child(
                            div().text_sm().text_color(rgb(0x9aacc8)).child(
                                "Touch or drag to paint. Try rotating and resuming the app.",
                            ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button(
                                    "pause",
                                    if self.paused { "Resume" } else { "Pause" },
                                    self.paused,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.paused = !this.paused;
                                        this.particles.set_paused(this.paused);
                                        this.fluid.set_paused(this.paused);
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(button("clear", "Clear", false).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.particles.clear();
                                    this.fluid.clear();
                                    cx.notify();
                                },
                            )))
                            .child(button("bloom", "Bloom", self.bloom).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.bloom = !this.bloom;
                                    cx.notify();
                                },
                            ))),
                    ),
            )
    }
}

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Effects::new()))
            .expect("open effects example");
    });
}
