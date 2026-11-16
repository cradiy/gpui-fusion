use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        checkbox::{Checkbox, CheckboxPart, CheckboxState},
    },
    default_theme,
    style::{MaterialMode, State, Styled as FusionStyled, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    controls: Vec<Entity<Checkbox>>,
    theme_button: Entity<Button>,
    material_button: Entity<Button>,
    reset_button: Entity<Button>,
    status: SharedString,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let runtime = self.theme.read(cx);
        let store = runtime.store();
        let context =
            ResolveContext::new(store.theme(), store.mode(), runtime.style_set()).unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        div()
            .size_full()
            .bg(color("canvas"))
            .text_color(color("foreground"))
            .id("checkbox-gallery")
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .child(div().text_2xl().child("Checkboxes"))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(self.theme_button.clone())
                    .child(self.material_button.clone())
                    .child(self.reset_button.clone()),
            )
            .child(
                div()
                    .relative()
                    .rounded_xl()
                    .bg(color("surface"))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(px(280.))
                            .h(px(220.))
                            .rounded_xl()
                            .bg(color("primary_subtle")),
                    )
                    .child(
                        div()
                            .relative()
                            .text_sm()
                            .text_color(color("foreground_secondary"))
                            .child(if runtime.material_mode() == MaterialMode::Glass {
                                "Follow theme material · Glass"
                            } else {
                                "Follow theme material · Plain"
                            }),
                    )
                    .children(self.controls[..3].iter().cloned())
                    .child(div().h(px(1.)).bg(color("edge_subtle")))
                    .children(self.controls[3..6].iter().cloned()),
            )
            .child(
                div()
                    .rounded_xl()
                    .bg(color("surface"))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_sm()
                            .text_color(color("foreground_secondary"))
                            .child("Fixed materials and custom color"),
                    )
                    .children(self.controls[6..].iter().cloned()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(color("foreground_secondary"))
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(color("foreground_muted"))
                    .child("Click the box · Tab to focus · Space to toggle"),
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
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(720.), px(720.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let owner = cx.weak_entity();
                        let theme_button = cx.new(|cx| {
                            let theme = theme.clone();
                            Button::new("Toggle theme", window, cx).on_click(move |_, cx| {
                                theme.update(cx, |t, cx| {
                                    t.set_mode(
                                        if t.store().mode() == "light" {
                                            "dark"
                                        } else {
                                            "light"
                                        },
                                        cx,
                                    )
                                    .unwrap();
                                });
                            })
                        });
                        let material_button = cx.new(|cx| {
                            let theme = theme.clone();
                            Button::new("Toggle material", window, cx).on_click(move |_, cx| {
                                theme.update(cx, |t, cx| {
                                    t.set_material_mode(
                                        if t.material_mode() == MaterialMode::Plain {
                                            MaterialMode::Glass
                                        } else {
                                            MaterialMode::Plain
                                        },
                                        cx,
                                    )
                                });
                            })
                        });
                        let reset_owner = owner.clone();
                        let reset_button = cx.new(|cx| {
                            Button::new("Set mixed", window, cx).on_click(move |_, cx| {
                                let _ = reset_owner.update(cx, |g: &mut Gallery, cx| {
                                    g.controls[2]
                                        .update(cx, |c, cx| c.set_state(CheckboxState::Mixed, cx));
                                });
                            })
                        });
                        let controls = [
                            ("Keep annotations", CheckboxState::Unchecked),
                            ("Align to guides", CheckboxState::Checked),
                            ("Selected layers", CheckboxState::Mixed),
                            ("Unavailable option", CheckboxState::Unchecked),
                            ("Enforced option", CheckboxState::Checked),
                            ("Partial policy", CheckboxState::Mixed),
                            ("Plain material", CheckboxState::Checked),
                            ("Glass material", CheckboxState::Checked),
                            ("Accent selection", CheckboxState::Checked),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, (label, state))| {
                            let owner = owner.clone();
                            cx.new(|cx| {
                                let control = Checkbox::new(label, state, window, cx)
                                    .disabled((3..6).contains(&index))
                                    .on_change(move |state, _, cx| {
                                        let _ = owner.update(cx, |g: &mut Gallery, cx| {
                                            g.status = format!("{label}: {state:?}").into();
                                            cx.notify();
                                        });
                                    });
                                match index {
                                    6 => control.material_mode(MaterialMode::Plain),
                                    7 => control.material_mode(MaterialMode::Glass),
                                    8 => control.part(CheckboxPart::Indicator, |s| {
                                        s.state(State::Selected, |s| {
                                            s.bg(Token::<Hsla>::new("accent"))
                                                .border_color(Token::<Hsla>::new("accent"))
                                        })
                                    }),
                                    _ => control,
                                }
                            })
                        })
                        .collect();
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            controls,
                            theme_button,
                            material_button,
                            reset_button,
                            status: "Ready".into(),
                            _subscription: subscription,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
