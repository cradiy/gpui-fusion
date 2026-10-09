use gpui::{prelude::*, *};
use uic::components::slider::{RangeSlider, RangeSliderEvent, RangeSliderState, SliderAppearance};

struct Example {
    price: Entity<RangeSliderState>,
    duration: Entity<RangeSliderState>,
    commits: usize,
    _subscriptions: Vec<Subscription>,
}

impl Example {
    fn new(cx: &mut Context<Self>) -> Self {
        let price = cx.new(|cx| {
            RangeSliderState::new(80.0..=260.0, 0.0..=400.0, cx)
                .step(1.)
                .min_gap(40.)
        });
        let duration = cx.new(|cx| {
            RangeSliderState::new(15.0..=45.0, 0.0..=60.0, cx)
                .step(1.)
                .min_gap(5.)
        });
        let subscriptions = [&price, &duration]
            .into_iter()
            .flat_map(|state| {
                [
                    cx.subscribe(state, |this, _, event: &RangeSliderEvent, cx| {
                        if matches!(event, RangeSliderEvent::Changed(_)) {
                            this.commits += 1;
                            cx.notify();
                        }
                    }),
                    cx.observe(state, |_, _, cx| cx.notify()),
                ]
            })
            .collect();
        Self {
            price,
            duration,
            commits: 0,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().effective();
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let price = self.price.read(cx).values();
        let duration = self.duration.read(cx).values();
        let disabled = self.price.read(cx).is_disabled();
        let toggle = self.price.clone();
        let reset_price = self.price.clone();
        let reset_duration = self.duration.clone();
        div().size_full().bg(rgb(0xf3f5f9)).text_color(rgb(0x24354d))
            .pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .child(div().id("page").size_full().overflow_y_scroll().p_6()
                .child(div().mx_auto().w_full().max_w(px(540.)).p_6().rounded_2xl().bg(rgb(0xffffff)).flex().flex_col().gap_6()
                    .child(div().text_xs().text_color(rgb(0x8090a8)).child("A LITTLE MORE PRECISE"))
                    .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Find your range."))
                    .child(div().text_sm().whitespace_normal().text_color(rgb(0x8090a8)).child("Choose the space between too little and too much."))
                    .child(div().p_5().rounded_xl().bg(rgb(0xf3f6fc)).flex().flex_col().gap_3()
                        .child(div().text_sm().child("Budget"))
                        .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(format!("${:.0} – ${:.0}", price.start(), price.end())))
                        .child(RangeSlider::new(&self.price).label("Budget").h(px(44.)))
                        .child(div().flex().justify_between().text_xs().text_color(rgb(0x8090a8)).child("$0").child("$400")))
                    .child(div().flex().flex_col().gap_3()
                        .child(div().flex().justify_between().text_sm().child("Duration").child(format!("{:.0}–{:.0} min", duration.start(), duration.end())))
                        .child(RangeSlider::new(&self.duration).label("Duration").h(px(44.)).appearance(SliderAppearance::default().active_track(rgb(0x55958b).into()).thumb_border(rgb(0x55958b).into()).focus_ring(rgb(0x79b8aa).into()))))
                    .child(div().text_xs().whitespace_normal().text_color(rgb(0x8090a8)).child("Drag either handle. Tab selects an endpoint; arrow keys adjust it."))
                    .child(div().flex().flex_wrap().gap_3()
                        .child(div().id("reset").px_4().py_2().rounded_lg().bg(rgb(0xeef2fa)).text_sm().cursor_pointer().child("Reset")
                            .on_click(move |_, _, cx| {
                                reset_price.update(cx, |state, cx| state.set_values(80.0..=260.0, cx));
                                reset_duration.update(cx, |state, cx| state.set_values(15.0..=45.0, cx));
                            }))
                        .child(div().id("disable").px_4().py_2().text_sm().cursor_pointer().child(if disabled { "Enable budget" } else { "Disable budget" })
                            .on_click(move |_, _, cx| toggle.update(cx, |state, cx| state.set_disabled(!disabled, cx)))))
                    .child(div().text_xs().text_color(rgb(0x8090a8)).child(format!("{} completed adjustments", self.commits)))))
    }
}

#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(680.), px(700.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(Example::new),
        )
        .expect("open range slider example");
    });
}
