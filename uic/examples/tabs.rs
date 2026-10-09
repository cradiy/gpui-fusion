use gpui::{prelude::*, *};
use uic::components::tabs::{TabVariant, Tabs, TabsAppearance};

const SECTIONS: [&str; 8] = [
    "Overview",
    "Activity",
    "Files",
    "Shared",
    "Favorites",
    "Offline",
    "Archive",
    "Settings",
];

struct Example {
    selected: usize,
}

impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().effective();
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let mut underline = Tabs::new("underline", self.selected).label("Workspace sections");
        let mut pill = Tabs::new("pill", self.selected)
            .label("Workspace sections, pill style")
            .variant(TabVariant::Pill)
            .p_1()
            .bg(rgb(0xf0f4f9))
            .appearance(TabsAppearance {
                indicator: rgb(0x23314a).into(),
                selected_text: rgb(0xffffff).into(),
                ..Default::default()
            });
        for (index, label) in SECTIONS.iter().enumerate() {
            if index == 5 {
                underline = underline.disabled_tab(index, *label);
                pill = pill.disabled_tab(index, *label);
            } else {
                underline = underline.tab(index, *label);
                pill = pill.tab(index, *label);
            }
        }
        let first = cx.entity();
        let second = cx.entity();
        div().size_full().pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .bg(rgb(0xf3f5f8)).text_color(rgb(0x23314a))
            .child(div().id("content").size_full().overflow_y_scroll().p_5()
                .child(div().max_w(px(880.)).mx_auto().flex().flex_col().gap_6()
                    .child(div().pt_6().text_3xl().font_weight(FontWeight::SEMIBOLD).child("A place for everything."))
                    .child(div().text_color(rgb(0x8190a4)).child("Switch sections, keep your context."))
                    .child(div().p_5().rounded_2xl().bg(rgb(0xffffff)).flex().flex_col().gap_5()
                        .child(div().text_sm().text_color(rgb(0x8190a4)).child("UNDERLINE"))
                        .child(underline.on_change(move |value, _, cx| first.update(cx, |this, cx| { this.selected = value; cx.notify(); })))
                        .child(div().mt_3().rounded_xl().p_6().bg(rgb(0xf5f8fd)).flex().flex_col().gap_3()
                            .child(div().text_2xl().child(SECTIONS[self.selected]))
                            .child(div().text_color(rgb(0x8190a4)).child("Everything you need, right where you left it."))))
                    .child(div().p_5().rounded_2xl().bg(rgb(0xffffff)).flex().flex_col().gap_5()
                        .child(div().text_sm().text_color(rgb(0x8190a4)).child("PILL"))
                        .child(pill.on_change(move |value, _, cx| second.update(cx, |this, cx| { this.selected = value; cx.notify(); }))))
                    .child(div().text_sm().text_color(rgb(0x8190a4)).child("Scroll sideways for more sections. Offline is unavailable. Use ← / →, Home or End when focused."))))
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
                    size(px(860.), px(720.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Example { selected: 0 }),
        )
        .expect("open tabs example");
    });
}
