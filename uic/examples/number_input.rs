use gpui::{prelude::*, *};
use uic::components::{
    input,
    number_input::{NumberInput, NumberInputChanged, NumberInputOptions, NumberInputState},
    slider::{Slider, SliderEvent, SliderState},
};

struct Example {
    width: Entity<NumberInputState>,
    radius: Entity<NumberInputState>,
    slider: Entity<SliderState>,
    changes: usize,
    _subscriptions: Vec<Subscription>,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let width = cx.new(|cx| {
            NumberInputState::new(
                160.,
                NumberInputOptions {
                    min: Some(48.),
                    max: Some(240.),
                    step: 8.,
                    precision: 0,
                },
                window,
                cx,
            )
        });
        let radius = cx.new(|cx| {
            NumberInputState::new(
                16.,
                NumberInputOptions {
                    min: Some(0.),
                    max: Some(48.),
                    step: 0.5,
                    precision: 1,
                },
                window,
                cx,
            )
        });
        let slider = cx.new(|cx| SliderState::new(160., 48.0..=240.0, cx).step(8.));
        let subscriptions = vec![
            cx.subscribe(
                &width,
                |this: &mut Self, _, event: &NumberInputChanged, cx| {
                    this.slider
                        .update(cx, |slider, cx| slider.set_value(event.value, cx));
                    this.changes += 1;
                    cx.notify();
                },
            ),
            cx.subscribe(&radius, |this: &mut Self, _, _: &NumberInputChanged, cx| {
                this.changes += 1;
                cx.notify();
            }),
            cx.subscribe(&slider, |this: &mut Self, _, event: &SliderEvent, cx| {
                let value = match event {
                    SliderEvent::Changing(value) | SliderEvent::Changed(value) => *value,
                };
                this.width.update(cx, |number, cx| {
                    number.set_value(value, cx);
                });
                cx.notify();
            }),
        ];
        Self {
            width,
            radius,
            slider,
            changes: 0,
            _subscriptions: subscriptions,
        }
    }
}
fn unit() -> impl IntoElement {
    div().text_sm().text_color(rgb(0x8391a4)).pr_2().child("px")
}
impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().safe_area;
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let width = self.width.read(cx).value();
        let radius = self.radius.read(cx).value();
        div()
            .size_full()
            .bg(rgb(0xf2f5fa))
            .text_color(rgb(0x25354b))
            .pt(safe.top)
            .pb(safe.bottom)
            .pl(safe.left)
            .pr(safe.right)
            .child(
                div()
                    .id("page")
                    .size_full()
                    .overflow_y_scroll()
                    .p_5()
                    .child(
                        div()
                            .mx_auto()
                            .w_full()
                            .max_w(px(520.))
                            .p_6()
                            .rounded_2xl()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xe5eaf2))
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x8191a9))
                                    .child("MAKE IT YOURS"),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("A little precision."),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x8191a9))
                                    .whitespace_normal()
                                    .child("Type a value or take it one step at a time."),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(156.))
                                    .rounded_xl()
                                    .bg(rgb(0xf0f4fb))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .w(px(width as f32))
                                            .max_w_full()
                                            .h(px(96.))
                                            .rounded(px(radius as f32))
                                            .bg(rgb(0x5276cc))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(0xffffff))
                                            .text_sm()
                                            .child(format!("{width:.0} × 96")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Width"),
                                    )
                                    .child(
                                        NumberInput::new(&self.width)
                                            .label("Width")
                                            .suffix(unit())
                                            .h(px(48.)),
                                    )
                                    .child(Slider::new(&self.slider).label("Width"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x8391a4))
                                            .child("48–240 px · Step 8"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Corner radius"),
                                    )
                                    .child(
                                        NumberInput::new(&self.radius)
                                            .label("Corner radius")
                                            .suffix(unit())
                                            .h(px(48.)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x8391a4))
                                            .child("0–48 px · Step 0.5"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x8391a4))
                                    .whitespace_normal()
                                    .child(format!(
                                        "Enter to apply · Escape to cancel · {} numeric changes",
                                        self.changes
                                    )),
                            ),
                    ),
            )
    }
}
#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application().run(|cx| {
        input::init(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(680.), px(800.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open number input example");
    });
}
