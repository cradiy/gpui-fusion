use gpui::{prelude::*, *};
use std::sync::Arc;
use uic::components::{
    pager::{PageChanged, Pager, PagerState},
    tabs::{TabVariant, Tabs},
    zoom_view::{ZoomState, ZoomView},
};

const LANDSCAPE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">
<defs><linearGradient id="sky" x2="0" y2="1"><stop stop-color="#c5e1e8"/><stop offset="1" stop-color="#f7edda"/></linearGradient><linearGradient id="water" x2="0" y2="1"><stop stop-color="#7ca9b4"/><stop offset="1" stop-color="#253e56"/></linearGradient></defs>
<rect width="1200" height="800" fill="url(#sky)"/><circle cx="885" cy="200" r="72" fill="#fff4cb"/>
<path d="M0 450L175 210 310 360 520 120 740 400 950 270 1200 430V800H0Z" fill="#729095"/>
<path d="M310 360L520 120 590 215 547 194 517 223 490 202 438 270Z" fill="#e7efdf"/>
<path d="M0 510L160 400 320 480 575 300 800 455 1025 365 1200 480V800H0Z" fill="#456973"/>
<path d="M0 510Q300 475 600 510T1200 500V800H0Z" fill="url(#water)"/>
<g stroke="#cae1d4" fill="none" opacity=".38"><path d="M60 560H380M620 552H970M410 594H810M100 650H500M670 690H1110M210 743H800" stroke-width="2"/><path d="M440 525H690M810 625H1090M60 702H265M800 765H1140"/></g>
<path d="M0 800V625L110 572 210 690 360 735 450 800Z" fill="#203e48"/><path d="M1200 800V605L1100 650 1030 722 860 800Z" fill="#203e48"/>
<g fill="#162e39"><path d="M65 620L98 420 132 620ZM150 672L183 492 216 672ZM1070 728L1105 490 1140 728ZM1135 687L1160 520 1185 687Z"/></g>
<g fill="#ffffff" opacity=".85"><circle cx="570" cy="633" r="4"/><circle cx="590" cy="633" r="4"/><circle cx="610" cy="633" r="4"/></g>
</svg>"##;

struct Example {
    pager: Entity<PagerState>,
    previews: Vec<Entity<ZoomState>>,
    images: Vec<Arc<Image>>,
    _subscriptions: Vec<Subscription>,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pager = cx.new(|cx| PagerState::new(2, window, cx));
        let previews: Vec<_> = (0..2)
            .map(|_| cx.new(|cx| ZoomState::new(size(px(1200.), px(800.)), window, cx)))
            .collect();
        let mut subscriptions = vec![cx.subscribe(&pager, |_, _, _: &PageChanged, cx| cx.notify())];
        subscriptions.extend(
            previews
                .iter()
                .map(|state| cx.observe(state, |_, _, cx| cx.notify())),
        );
        let sunset = LANDSCAPE
            .replace("#c5e1e8", "#dbb6ab")
            .replace("#f7edda", "#fbe1bd")
            .replace("#729095", "#a0858b")
            .replace("#456973", "#70576e")
            .replace("#7ca9b4", "#b88b8c")
            .replace("#253e56", "#52394f");
        let images = [LANDSCAPE.as_bytes().to_vec(), sunset.into_bytes()]
            .into_iter()
            .map(|bytes| Arc::new(Image::from_bytes(ImageFormat::Svg, bytes)))
            .collect();
        Self {
            pager,
            previews,
            images,
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
        let index = self.pager.read(cx).current_page().unwrap_or(0);
        let zoom = self.previews[index].read(cx).zoom();
        let pager = self.pager.clone();
        let previews = self.previews.clone();
        let images = self.images.clone();
        div().size_full().pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .bg(rgb(0xf3f5f8)).text_color(rgb(0x23314a))
            .child(div().size_full().p_4().max_w(px(1000.)).mx_auto().flex().flex_col().gap_4()
                .child(div().flex_shrink_0().pt_3().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Closer to the details."))
                .child(div().flex_shrink_0().text_sm().text_color(rgb(0x8190a4)).child("Two views. A different perspective."))
                .child(Tabs::new("landscapes", index).variant(TabVariant::Pill).flex_shrink_0()
                    .tab(0, "Morning light").tab(1, "Quiet evening")
                    .on_change(move |index, _, cx| { pager.update(cx, |state, cx| { state.scroll_to(index, cx); }); }))
                .child(Pager::new("previews", &self.pager, move |index, _, _| {
                    ZoomView::new(("preview", index), &previews[index], img(images[index].clone()).size_full().object_fit(ObjectFit::Contain)).rounded_2xl()
                }).flex_1().min_h_0().rounded_2xl().bg(rgb(0x16212e)))
                .child(div().flex_shrink_0().flex().items_center().gap_2()
                    .children([("−", 0usize), ("+", 1), ("Fit", 2)].map(|(label, action)| {
                        let state = self.previews[index].clone();
                        div().id(("zoom-control", action)).role(Role::Button).cursor_pointer().px_4().py_2().rounded_lg().bg(rgb(0xe3eaf4))
                            .on_click(move |_, _, cx| { state.update(cx, |state, cx| match action { 0 => state.zoom_to(state.zoom() / 1.5, cx), 1 => state.zoom_to(state.zoom() * 1.5, cx), _ => state.reset(cx) }); }).child(label)
                    }))
                    .child(div().ml_auto().text_sm().child(format!("{:.0}%", zoom * 100.))))
                .child(div().flex_shrink_0().pb_2().text_sm().text_color(rgb(0x8190a4)).child("Pinch or scroll to zoom. Double-tap to zoom or fit. Swipe pages at Fit; drag to explore when zoomed.")))
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
                    size(px(900.), px(800.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open zoom view example");
    });
}
