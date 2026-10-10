use gpui::{prelude::*, *};
use uic::components::{
    autocomplete::{Autocomplete, AutocompleteAction, AutocompleteEvent, AutocompleteState},
    input,
    select::SelectOption,
};

fn options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("design", "Design system").keywords("Workspace / UI library"),
        SelectOption::new("research", "Research notes").keywords("Workspace / Discovery"),
        SelectOption::new("photos", "Photography").keywords("Personal / Collections"),
        SelectOption::new("writing", "Writing room").keywords("Personal / Drafts"),
        SelectOption::new("archive", "Archive")
            .keywords("Workspace / Read-only")
            .disabled(true),
    ]
}
struct Example {
    search: Entity<AutocompleteState>,
    status: SharedString,
    _subscription: Subscription,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| AutocompleteState::new(options(), window, cx).auto_highlight(true));
        let subscription = cx.subscribe(
            &search,
            |this: &mut Self, _, event: &AutocompleteEvent, cx| {
                this.status = match event {
                    AutocompleteEvent::Change(value) => format!("Editing: {value}"),
                    AutocompleteEvent::Selected(option) => {
                        format!("Selected workspace: {}", option.label)
                    }
                    AutocompleteEvent::Submit(value) => format!("Submitted: {value}"),
                }
                .into();
                cx.notify();
            },
        );
        Self {
            search,
            status: "Choose a suggestion or enter your own name.".into(),
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
        div()
            .size_full()
            .bg(rgb(0xf3f5f9))
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
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x8290a3))
                                    .child("YOUR NEXT DESTINATION"),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Find your workspace."),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .whitespace_normal()
                                    .text_color(rgb(0x8290a3))
                                    .child("A familiar place, or something entirely new."),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(div().text_sm().child("Workspace"))
                                    .child(
                                        Autocomplete::new(&self.search)
                                            .key_binding("ctrl-p", AutocompleteAction::Previous)
                                            .key_binding("ctrl-n", AutocompleteAction::Next)
                                            .label("Workspace")
                                            .placeholder("Search or enter a name")
                                            .rounded_xl()
                                            .render_option(|option, flags, _, _| {
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .size(px(32.))
                                                            .rounded_lg()
                                                            .bg(if flags.highlighted {
                                                                rgb(0xdbe5fa)
                                                            } else {
                                                                rgb(0xf0f3f8)
                                                            })
                                                            .text_color(rgb(0x536e9c))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_sm()
                                                            .child(
                                                                option
                                                                    .label
                                                                    .chars()
                                                                    .next()
                                                                    .unwrap_or(' ')
                                                                    .to_string(),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_1()
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .child(option.label.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x8290a3))
                                                                    .truncate()
                                                                    .child(option.keywords.clone()),
                                                            ),
                                                    )
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .whitespace_normal()
                                    .text_color(rgb(0x8290a3))
                                    .child("↑ ↓ or Ctrl+P / Ctrl+N to choose · Enter to confirm · Esc to dismiss"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("loading")
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(rgb(0xeaf0fc))
                                            .text_sm()
                                            .cursor_pointer()
                                            .child("Loading")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search.update(cx, |state, cx| {
                                                    state.set_loading(true, cx)
                                                });
                                                this.search.focus_handle(cx).focus(window, cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("empty")
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(rgb(0xeaf0fc))
                                            .text_sm()
                                            .cursor_pointer()
                                            .child("Empty")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search.update(cx, |state, cx| {
                                                    state.set_loading(false, cx);
                                                    state.set_options(vec![], cx);
                                                });
                                                this.search.focus_handle(cx).focus(window, cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("reset")
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(rgb(0xeaf0fc))
                                            .text_sm()
                                            .cursor_pointer()
                                            .child("Reset")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.search.update(cx, |state, cx| {
                                                    state.set_loading(false, cx);
                                                    state.set_value("", cx);
                                                    state.set_options(options(), cx);
                                                });
                                                this.search.focus_handle(cx).focus(window, cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .mt_4()
                                    .p_4()
                                    .rounded_xl()
                                    .bg(rgb(0xf5f7fb))
                                    .text_sm()
                                    .whitespace_normal()
                                    .child(self.status.clone()),
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
                    size(px(680.), px(640.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open autocomplete example");
    });
}
