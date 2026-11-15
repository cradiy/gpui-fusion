use gpui::{prelude::*, *};
use gpui_fusion::{
    components::input::{Input, InputResize, InputSide, InputState},
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    fields: Vec<Entity<Input>>,
    status: SharedString,
    _subscription: Subscription,
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme.read(cx);
        let context = ResolveContext::new(
            theme.store().theme(),
            theme.store().mode(),
            theme.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        let dark = theme.store().mode() == "dark";
        let glass = theme.material_mode() == MaterialMode::Glass;
        let action = |id, label| {
            div()
                .id(id)
                .px_4()
                .py_2()
                .rounded_lg()
                .bg(color("surface_raised"))
                .cursor_pointer()
                .child(label)
        };
        let mut form = div().flex().flex_col().gap_5().w_full();
        for (index, label) in [
            "Title",
            "Search",
            "Secret",
            "Notes",
            "Read only",
            "Unavailable",
            "Needs attention",
            "Always plain",
        ]
        .into_iter()
        .enumerate()
        {
            form = form.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label)
                    .child(self.fields[index].clone()),
            );
        }
        div()
            .id("input-gallery")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .child(
                div()
                    .w(px(640.))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Inputs"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                action("theme", if dark { "Light theme" } else { "Dark theme" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.theme.update(cx, |theme, cx| {
                                            theme
                                                .set_mode(
                                                    if theme.store().mode() == "dark" {
                                                        "light"
                                                    } else {
                                                        "dark"
                                                    },
                                                    cx,
                                                )
                                                .unwrap()
                                        });
                                    })),
                            )
                            .child(
                                action(
                                    "material",
                                    if glass {
                                        "Material: glass"
                                    } else {
                                        "Material: plain"
                                    },
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.theme.update(cx, |theme, cx| {
                                            theme.set_material_mode(
                                                if theme.material_mode() == MaterialMode::Glass {
                                                    MaterialMode::Plain
                                                } else {
                                                    MaterialMode::Glass
                                                },
                                                cx,
                                            )
                                        });
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .rounded_xl()
                            .p_6()
                            .bg(color("muted_surface"))
                            .child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .w(px(260.))
                                    .h(px(340.))
                                    .rounded_xl()
                                    .bg(color("primary").opacity(0.08)),
                            )
                            .child(form),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(color("foreground_muted"))
                            .child(self.status.clone()),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(IconAssets)
        .run(|cx| {
            let theme = ThemeRuntime::install(
                ThemeStore::new(default_theme(), "light").unwrap(),
                "neutral",
                cx,
            )
            .unwrap();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(760.), px(930.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let mut fields = Vec::new();
                        for index in 0..8 {
                            let editor = cx.new(|cx| {
                                let state = InputState::new(cx);
                                match index {
                                    0 => state.aria_label("Title").placeholder("Untitled document"),
                                    1 => {
                                        state.aria_label("Search").placeholder("Find in collection")
                                    }
                                    2 => state
                                        .aria_label("Secret")
                                        .password()
                                        .placeholder("Enter a secret"),
                                    3 => state
                                        .aria_label("Notes")
                                        .multiline()
                                        .placeholder("Write a note…"),
                                    4 => state
                                        .aria_label("Read only")
                                        .initial_value("Managed by your organization")
                                        .read_only(true),
                                    5 => state
                                        .aria_label("Unavailable")
                                        .initial_value("Not available")
                                        .disabled(true),
                                    6 => state
                                        .aria_label("Needs attention")
                                        .initial_value("Missing required details"),
                                    _ => state
                                        .aria_label("Always plain")
                                        .placeholder("Fixed material, shared colors"),
                                }
                            });
                            let owner = cx.weak_entity();
                            fields.push(cx.new(|cx| {
                                let input = Input::new(&editor, window, cx).rows(3).on_submit(
                                    move |_, _, cx| {
                                        let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                            view.status = "Submitted".into();
                                            cx.notify();
                                        });
                                    },
                                );
                                match index {
                                    0 => input.clearable(true),
                                    1 => input.search_icon(InputSide::Leading).clearable(true),
                                    3 => input.resize(InputResize::Vertical).resize_limits(
                                        size(px(180.), px(72.)),
                                        size(px(640.), px(480.)),
                                    ),
                                    6 => input.variant("invalid"),
                                    7 => input
                                        .material_mode(MaterialMode::Plain)
                                        .search_icon(InputSide::Trailing)
                                        .clearable(true),
                                    _ => input,
                                }
                            }));
                        }
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            fields,
                            status:
                                "Enter submits · Multiline uses Ctrl/Cmd+Enter · Ctrl/Cmd+Z undoes"
                                    .into(),
                            _subscription: subscription,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
