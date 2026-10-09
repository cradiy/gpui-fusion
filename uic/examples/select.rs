use gpui::{prelude::*, *};
use uic::components::{
    input,
    select::{Select, SelectChanged, SelectMenu, SelectOption, SelectState},
};

struct Example {
    selection: Entity<SelectState>,
    changes: usize,
    _subscription: Subscription,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let names = [
            "Personal",
            "Design studio",
            "Product notes",
            "Archived projects",
            "Photography",
            "Research",
            "Reading room",
            "Weekend ideas",
            "North coast",
            "Garden journal",
            "Shared library",
            "Travel plans",
            "Music collection",
            "Writing desk",
            "Experiments",
            "Daily notes",
            "Recipes",
            "Learning",
            "Family",
            "Team workspace",
        ];
        let selection = cx.new(|cx| {
            SelectState::new(
                names
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        SelectOption::new(i.to_string(), label)
                            .keywords(if i == 1 { "creative art 设计" } else { "" })
                            .disabled(i == 3)
                    })
                    .collect(),
                window,
                cx,
            )
        });
        selection.update(cx, |state, cx| {
            state.select("1", cx);
        });
        let subscription = cx.subscribe(
            &selection,
            |this: &mut Example, _, _: &SelectChanged, cx| {
                this.changes += 1;
                cx.notify();
            },
        );
        Self {
            selection,
            changes: 0,
            _subscription: subscription,
        }
    }
}
impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let safe = window.insets().safe_area;
        window.set_system_bar_appearance(SystemBarAppearance {
            status: SystemBarStyle::Dark,
            navigation: SystemBarStyle::Dark,
        });
        let value = self
            .selection
            .read(cx)
            .selected_option()
            .map(|option| option.label.clone())
            .unwrap_or("Nothing selected".into());
        div().size_full().bg(rgb(0xf2f5fa)).text_color(rgb(0x25354b)).pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .child(div().size_full().p_5().flex().flex_col().justify_center().items_center()
                .child(div().w_full().max_w(px(580.)).p_6().rounded_2xl().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5eaf2)).flex().flex_col().gap_5()
                    .child(div().text_sm().text_color(rgb(0x8191a9)).child("A PLACE FOR EVERYTHING"))
                    .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).whitespace_normal().child("Find your next space."))
                    .child(div().text_sm().text_color(rgb(0x8191a9)).whitespace_normal().child("Choose a workspace. Search by name, or use the arrow keys to explore."))
                    .child(div().flex().flex_col().gap_2()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Workspace"))
                        .child(Select::new("workspace", &self.selection).label("Choose a workspace").placeholder("Choose a workspace").clearable(true).h(px(50.)).rounded_xl()
                            .menu(SelectMenu::new().w(px(360.)).h(px(420.)))
                            .render_option(|option, flags, _, _| {
                                let index: usize = option.id.parse().unwrap_or(0);
                                div().flex().items_center().gap_3()
                                    .child(div().size(px(30.)).flex_shrink_0().rounded_lg().bg(rgb([0xe4ecfa, 0xe5f0ec, 0xf3e9e4][index % 3])).text_color(rgb([0x5274ab, 0x547e70, 0xa27c65][index % 3])).flex().items_center().justify_center().text_sm().child(option.label.chars().next().unwrap().to_string()))
                                    .child(div().flex_1().min_w_0().flex().flex_col().gap_1().child(div().text_sm().child(option.label.clone())).child(div().text_xs().text_color(rgb(0x8391a4)).child(if flags.disabled { "Read-only archive" } else { "Your collection" })))
                            })))
                    .child(div().p_4().rounded_xl().bg(rgb(0xf3f6fb)).flex().flex_col().gap_2()
                        .child(div().text_xs().text_color(rgb(0x8191a9)).child("CURRENT SELECTION"))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
                        .child(div().text_xs().text_color(rgb(0x8191a9)).child(format!("{} selection changes", self.changes))))))
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
                    size(px(820.), px(780.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open select example");
    });
}
