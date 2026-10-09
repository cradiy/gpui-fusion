use gpui::{prelude::*, *};
use uic::components::{
    input,
    tags_input::{TagsInput, TagsInputChanged, TagsInputOptions, TagsInputState},
};

struct Example {
    tags: Entity<TagsInputState>,
    changes: usize,
    _subscription: Subscription,
}
impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tags = cx.new(|cx| {
            TagsInputState::new(
                vec!["Design".into(), "Research".into(), "Ideas".into()],
                TagsInputOptions {
                    max_tags: Some(6),
                    ..Default::default()
                },
                window,
                cx,
            )
            .validator(|tag| {
                if tag.chars().count() > 24 {
                    Err("Keep each tag to 24 characters or fewer".into())
                } else {
                    Ok(())
                }
            })
        });
        let subscription = cx.subscribe(&tags, |this: &mut Self, _, _: &TagsInputChanged, cx| {
            this.changes += 1;
            cx.notify();
        });
        Self {
            tags,
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
        let data = self.tags.read(cx);
        let count = data.tags().len();
        let error = data.error().map(ToString::to_string);
        let disabled = data.is_disabled(cx);
        let reset = self.tags.clone();
        let toggle = self.tags.clone();
        div().size_full().bg(rgb(0xf2f5fa)).text_color(rgb(0x25354b)).pt(safe.top).pb(safe.bottom).pl(safe.left).pr(safe.right)
            .child(div().id("page").size_full().overflow_y_scroll().p_5()
                .child(div().mx_auto().w_full().max_w(px(540.)).p_6().rounded_2xl().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5eaf2)).flex().flex_col().gap_5()
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x8191a9)).child("A PLACE FOR EVERY IDEA"))
                    .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).whitespace_normal().child("Make it easy to find."))
                    .child(div().text_sm().text_color(rgb(0x8191a9)).whitespace_normal().child("A few thoughtful labels keep your collection organized."))
                    .child(div().w_full().p_5().rounded_xl().bg(rgb(0xf3f6fb)).flex().flex_col().gap_2()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Studio notes"))
                        .child(div().text_sm().text_color(rgb(0x8191a9)).child("Your next good idea starts here.")))
                    .child(div().flex().flex_col().gap_2()
                        .child(div().flex().justify_between().items_center()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Project tags"))
                            .child(div().text_xs().text_color(rgb(0x8191a9)).child(format!("{count} / 6"))))
                        .child(TagsInput::new(&self.tags).label("Project tags").placeholder("Add a tag…").rounded_xl())
                        .child(div().text_xs().whitespace_normal().text_color(if error.is_some() { rgb(0xc55764) } else { rgb(0x8191a9) })
                            .child(error.unwrap_or_else(|| "Press Enter to add. Use Backspace to select and remove.".into()))))
                    .child(div().flex().gap_3()
                        .child(div().id("reset").px_4().py_2().rounded_lg().bg(rgb(0xeff3fa)).text_sm().cursor_pointer().child("Reset")
                            .on_click(move |_, _, cx| { reset.update(cx, |state, cx| { let _ = state.set_tags(vec!["Design".into(), "Research".into(), "Ideas".into()], cx); }); }))
                        .child(div().id("disable").px_4().py_2().rounded_lg().text_sm().cursor_pointer().child(if disabled { "Enable editing" } else { "Disable editing" })
                            .on_click(move |_, _, cx| { toggle.update(cx, |state, cx| state.set_disabled(!state.is_disabled(cx), cx)); })))
                    .child(div().text_xs().text_color(rgb(0x8191a9)).child(format!("{} tag changes", self.changes)))))
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
                    size(px(700.), px(660.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .expect("open tags input example");
    });
}
