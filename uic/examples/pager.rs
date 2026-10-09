use gpui::{prelude::*, *};
use uic::components::{
    pager::{PageChanged, Pager, PagerState},
    tabs::{TabVariant, Tabs},
};

const PAGES: [(&str, &str, &str, u32); 3] = [
    (
        "Overview",
        "Make room for a good day.",
        "A place for the things you want to keep close.",
        0x416dba,
    ),
    (
        "Activity",
        "A little progress, every day.",
        "Small moments that move your work forward.",
        0x278575,
    ),
    (
        "Collection",
        "Keep what inspires you.",
        "Ideas, notes and discoveries worth coming back to.",
        0x9c6b43,
    ),
];

struct Page {
    index: usize,
    count: usize,
    scroll: ScrollHandle,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, title, description, accent) = PAGES[self.index];
        div()
            .id("page-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_5()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex_shrink_0()
                    .rounded_2xl()
                    .p_6()
                    .bg(rgb(accent))
                    .text_color(rgb(0xffffff))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("0{} / YOUR SPACE", self.index + 1)),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_sm().child(description))
                    .child(
                        div()
                            .mt_3()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("count")
                                    .role(Role::Button)
                                    .cursor_pointer()
                                    .px_4()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(rgba(0xffffff25))
                                    .hover(|style| style.bg(rgba(0xffffff40)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.count += 1;
                                        cx.notify();
                                    }))
                                    .child("Add a moment"),
                            )
                            .child(format!("{} saved", self.count)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x8490a2))
                            .child("Scroll to explore"),
                    ),
            )
            .children((0..12).map(move |index| {
                let label = match self.index {
                    0 => ["Morning notes", "A new direction", "Things to revisit"][index % 3],
                    1 => ["Design review", "Shared a collection", "Updated workspace"][index % 3],
                    _ => ["Color studies", "Weekend reading", "Saved for later"][index % 3],
                };
                div()
                    .flex_shrink_0()
                    .rounded_xl()
                    .p_4()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xe8edf3))
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .size(px(42.))
                            .rounded_lg()
                            .bg(rgb(0xf0f4f9))
                            .text_color(rgb(accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(
                        div().flex_1().flex().flex_col().gap_1().child(label).child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x8994a4))
                                .child("Personal workspace · Just for you"),
                        ),
                    )
            }))
    }
}

struct Example {
    pager: Entity<PagerState>,
    pages: Vec<Entity<Page>>,
    _subscription: Subscription,
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pager = cx.new(|cx| PagerState::new(PAGES.len(), window, cx));
        let subscription = cx.subscribe(&pager, |_, _, _: &PageChanged, cx| cx.notify());
        let pages = (0..PAGES.len())
            .map(|index| {
                cx.new(|_| Page {
                    index,
                    count: 0,
                    scroll: ScrollHandle::new(),
                })
            })
            .collect();
        Self {
            pager,
            pages,
            _subscription: subscription,
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
        let selected = self.pager.read(cx).current_page().unwrap_or(0);
        let pages = self.pages.clone();
        div()
            .size_full()
            .pt(safe.top)
            .pb(safe.bottom)
            .pl(safe.left)
            .pr(safe.right)
            .bg(rgb(0xf3f5f8))
            .text_color(rgb(0x23314a))
            .child(
                div()
                    .size_full()
                    .max_w(px(960.))
                    .mx_auto()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_2()
                            .py_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Your space"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x8190a4))
                                    .child("Swipe between pages. Each page keeps its place."),
                            ),
                    )
                    .child({
                        let state = self.pager.clone();
                        PAGES.iter().enumerate().fold(
                            Tabs::new("page-tabs", selected)
                                .label("Workspace pages")
                                .variant(TabVariant::Pill)
                                .flex_shrink_0()
                                .on_change(move |index, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.scroll_to(index, cx);
                                    });
                                }),
                            |tabs, (index, (name, _, _, _))| tabs.tab(index, *name),
                        )
                    })
                    .child(
                        Pager::new("workspace", &self.pager, move |index, _, _| {
                            pages[index].clone()
                        })
                        .flex_1()
                        .min_h_0()
                        .rounded_2xl()
                        .bg(rgb(0xf9fafc))
                        .border_1()
                        .border_color(rgb(0xe3e9f0)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .justify_between()
                            .px_2()
                            .pb_2()
                            .text_sm()
                            .text_color(rgb(0x8190a4))
                            .child("Drag sideways · Scroll vertically")
                            .child(format!("{:02} / 03", selected + 1)),
                    ),
            )
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
                    size(px(860.), px(780.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open pager example");
    });
}
