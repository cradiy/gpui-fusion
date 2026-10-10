use gpui::{prelude::*, *};
use uic::components::split_pane::{SplitPane, SplitPaneEvent, SplitPaneState};

struct Example {
    columns: Entity<SplitPaneState>,
    rows: Entity<SplitPaneState>,
    commits: usize,
    selected: usize,
    _subscriptions: Vec<Subscription>,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let columns = cx.new(|cx| SplitPaneState::new(0.28, window, cx));
        let rows = cx.new(|cx| SplitPaneState::new(0.68, window, cx));
        let subscriptions = [&columns, &rows]
            .into_iter()
            .map(|state| {
                cx.subscribe(state, |this: &mut Self, _, event: &SplitPaneEvent, cx| {
                    if matches!(event, SplitPaneEvent::Changed(_)) {
                        this.commits += 1;
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            columns,
            rows,
            commits: 0,
            selected: 0,
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
        let names = ["Overview", "Type", "Colors", "Spacing"];
        let sidebar = div()
            .size_full()
            .rounded_lg()
            .bg(rgb(0xf1f4f9))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_hidden()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x8996ab))
                    .mb_2()
                    .child("WORKSPACE"),
            )
            .children(
                names
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| {
                        div()
                            .id(index)
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .when(index == self.selected, |row| {
                                row.bg(rgb(0xe0e8f8))
                                    .text_color(rgb(0x4168ba))
                            })
                            .hover(|style| style.bg(rgb(0xe7edf7)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected = index;
                                cx.notify();
                            }))
                            .child(name)
                    }),
            );
        let editor = div().id("editor").size_full().rounded_lg().bg(rgb(0xffffff)).p_5().overflow_y_scroll()
            .flex().flex_col().gap_4()
            .child(div().text_xs().text_color(rgb(0x8996ab)).child("DESIGN NOTES"))
            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(names[self.selected]))
            .child(div().text_sm().whitespace_normal().line_height(px(24.)).child("A workspace should leave room for the work. Drag either divider to find your balance."))
            .child(div().h(px(84.)).w_full().rounded_lg().bg(rgb(0xeaf0fc)).p_4()
                .child(div().h(px(8.)).w_3_4().rounded_full().bg(rgb(0x9db5e9)))
                .child(div().mt_3().h(px(8.)).w_1_2().rounded_full().bg(rgb(0xc0cfee))));
        let details = div()
            .id("details")
            .size_full()
            .rounded_lg()
            .bg(rgb(0xebf1f2))
            .p_4()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6d8b89))
                    .child("DETAILS"),
            )
            .child(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .child("The lower pane keeps its own size constraints."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6d8b89))
                    .child(format!("Layout changes saved: {}", self.commits)),
            );
        let right = SplitPane::new(&self.rows, editor, details)
            .axis(Axis::Vertical)
            .first_min_size(px(120.))
            .second_min_size(px(88.))
            .label("Editor height");
        let workspace = SplitPane::new(&self.columns, sidebar, right)
            .h(px(460.))
            .first_min_size(px(112.))
            .first_max_size(px(300.))
            .second_min_size(px(140.))
            .label("Sidebar width");
        let reset = div()
            .id("reset")
            .px_4()
            .py_2()
            .rounded_lg()
            .bg(rgb(0xe6ecf7))
            .cursor_pointer()
            .text_sm()
            .on_click(cx.listener(|this, _, _, cx| {
                this.columns
                    .update(cx, |state, cx| state.reset(cx));
                this.rows
                    .update(cx, |state, cx| state.reset(cx));
            }))
            .child("Reset layout");
        let instructions = concat!(
            "Drag the handles. Double-click to reset. ",
            "Focus a divider to resize with arrow keys.",
        );
        let content = div()
            .mx_auto()
            .w_full()
            .max_w(px(960.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x8290a3))
                    .child("A FLEXIBLE WORKSPACE"),
            )
            .child(
                div()
                    .text_3xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Make room."),
            )
            .child(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .text_color(rgb(0x8290a3))
                    .child(instructions),
            )
            .child(workspace)
            .child(reset);
        div()
            .size_full()
            .bg(rgb(0xf5f7fb))
            .text_color(rgb(0x26364d))
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
                    .child(content),
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
                    size(px(920.), px(720.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open split pane example");
    });
}
