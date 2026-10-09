use gpui::{prelude::*, *};
use uic::components::{
    input,
    select::{MultiSelect, MultiSelectChanged, SelectMenu, SelectOption, SelectState},
};

struct Example {
    categories: Entity<SelectState>,
    changes: usize,
    _subscription: Subscription,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let categories = cx.new(|cx| {
            SelectState::multiple(
                vec![
                    SelectOption::new("design", "Design").keywords("creative art"),
                    SelectOption::new("research", "Research").keywords("study insight"),
                    SelectOption::new("writing", "Writing").keywords("text editorial"),
                    SelectOption::new("photo", "Photography").keywords("image camera"),
                    SelectOption::new("music", "Music").keywords("audio sound"),
                    SelectOption::new("video", "Video").keywords("film motion"),
                    SelectOption::new("archive", "Archived").disabled(true),
                ],
                window,
                cx,
            )
            .max_selected(3)
        });
        categories.update(cx, |state, cx| {
            state.set_selected_ids(vec!["design".into(), "research".into()], cx);
        });
        let subscription = cx.subscribe(
            &categories,
            |this: &mut Self, _, _: &MultiSelectChanged, cx| {
                this.changes += 1;
                cx.notify();
            },
        );
        Self {
            categories,
            changes: 0,
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
        let count = self.categories.read(cx).selected_ids().len();
        div().size_full().bg(rgb(0xf3f5f9)).text_color(rgb(0x25354b))
            .pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .child(div().id("page").size_full().overflow_y_scroll().p_5()
                .child(div().mx_auto().w_full().max_w(px(540.)).p_6().rounded_2xl().bg(rgb(0xffffff)).flex().flex_col().gap_5()
                    .child(div().text_xs().text_color(rgb(0x8290a3)).child("ROOM FOR MORE THAN ONE"))
                    .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Build your collection."))
                    .child(div().text_sm().whitespace_normal().text_color(rgb(0x8290a3)).child("Choose the categories that describe your work."))
                    .child(div().p_5().rounded_xl().bg(rgb(0xf2f5fb)).flex().flex_col().gap_2()
                        .child(div().text_lg().child("Studio library"))
                        .child(div().text_sm().text_color(rgb(0x8290a3)).child("Ideas, studies and things worth keeping.")))
                    .child(div().flex().flex_col().gap_2()
                        .child(div().flex().justify_between().text_sm().child("Categories").child(format!("{count} / 3")))
                        .child(MultiSelect::new("categories", &self.categories).label("Categories").placeholder("Choose categories").clearable(true).rounded_xl()
                            .menu(SelectMenu::new().w(px(350.)).h(px(380.))))
                        .child(div().text_xs().text_color(rgb(0x8290a3)).whitespace_normal().child("Choose up to three. Search by name or keyword.")))
                    .child(div().text_sm().text_color(rgb(0x8290a3)).whitespace_normal().child("Selections update immediately. Remove a label to make room for another category."))
                    .child(div().text_xs().text_color(rgb(0x8290a3)).child(format!("{} selection changes", self.changes)))))
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
                    size(px(700.), px(720.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open multi select example");
    });
}
