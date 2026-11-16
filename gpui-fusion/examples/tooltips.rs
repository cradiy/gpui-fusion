use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        tooltip::{Tooltip, TooltipSide},
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    hints: Vec<Entity<Tooltip>>,
    theme_button: Entity<Button>,
    material_button: Entity<Button>,
    _subscription: Subscription,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let runtime = self.theme.read(cx);
        let context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )
        .unwrap();
        let color = |name| context.resolve(&Token::<Hsla>::new(name).into()).unwrap();
        div().id("tooltip-gallery").size_full().p_8().overflow_y_scroll()
            .bg(color("canvas")).text_color(color("foreground"))
            .flex().flex_col().gap_6()
            .on_key_down(|event, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab" && !modifiers.control && !modifiers.alt && !modifiers.platform {
                    if modifiers.shift { window.focus_prev(cx); } else { window.focus_next(cx); }
                    cx.stop_propagation();
                }
            })
            .child(div().text_2xl().child("Tooltips"))
            .child(div().flex().gap_3().child(self.theme_button.clone()).child(self.material_button.clone()))
            .child(div().text_color(color("foreground_muted")).child(
                if runtime.material_mode() == MaterialMode::Glass { "Theme material: Glass" } else { "Theme material: Plain" }
            ))
            .child(div().relative().rounded_xl().p_8().bg(color("surface"))
                .flex().flex_col().gap_8()
                .child(div().absolute().right_0().top_0().w(px(200.)).h(px(180.)).rounded_xl().bg(color("primary_subtle")))
                .child(div().relative().child("Hover or use Tab to reveal a hint"))
                .child(div().flex().flex_wrap().gap_5().children(self.hints[..4].iter().cloned()))
                .child(div().text_color(color("foreground_muted")).child("Fixed materials"))
                .child(div().flex().gap_5().children(self.hints[4..6].iter().cloned()))
                .child(div().text_color(color("foreground_muted")).child("Disabled control"))
                .child(self.hints[6].clone()))
            .child(div().text_sm().text_color(color("foreground_muted")).child("Escape dismisses · Hints stay visible while hovered · Clicking activates the control"))
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
                        size(px(760.), px(520.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let theme_button = cx.new(|cx| {
                            let theme = theme.clone();
                            Button::new("Toggle theme", window, cx).on_click(move |_, cx| {
                                theme.update(cx, |theme, cx| {
                                    theme
                                        .set_mode(
                                            if theme.store().mode() == "light" {
                                                "dark"
                                            } else {
                                                "light"
                                            },
                                            cx,
                                        )
                                        .unwrap();
                                })
                            })
                        });
                        let material_button = cx.new(|cx| {
                            let theme = theme.clone();
                            Button::new("Toggle material", window, cx).on_click(move |_, cx| {
                                theme.update(cx, |theme, cx| {
                                    theme.set_material_mode(
                                        if theme.material_mode() == MaterialMode::Plain {
                                            MaterialMode::Glass
                                        } else {
                                            MaterialMode::Plain
                                        },
                                        cx,
                                    );
                                })
                            })
                        });
                        let hints = [
                            ("Inspect", "Inspect the selected object", TooltipSide::Top),
                            (
                                "Arrange",
                                "Distribute selected objects at equal intervals",
                                TooltipSide::Bottom,
                            ),
                            (
                                "History",
                                "Browse saved revisions without changing the current document",
                                TooltipSide::Left,
                            ),
                            (
                                "Measure",
                                "Measure distances between objects",
                                TooltipSide::Right,
                            ),
                            (
                                "Plain hint",
                                "A compact hint with a solid surface",
                                TooltipSide::Top,
                            ),
                            (
                                "Glass hint",
                                "A frosted surface with clear foreground text",
                                TooltipSide::Top,
                            ),
                            (
                                "Publish",
                                "Connect a destination before publishing",
                                TooltipSide::Top,
                            ),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, (label, text, side))| {
                            let button = cx.new(|cx| {
                                let mut button = Button::new(label, window, cx)
                                    .icon(GpuiIcons::LucideInfo.path());
                                button.set_disabled(index == 6, cx);
                                button
                            });
                            cx.new(|cx| {
                                let hint = Tooltip::new(text, &button, window, cx).side(side);
                                match index {
                                    0 => hint.shortcut("Ctrl+I"),
                                    4 => hint.material_mode(MaterialMode::Plain),
                                    5 => hint.material_mode(MaterialMode::Glass),
                                    _ => hint,
                                }
                            })
                        })
                        .collect();
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            hints,
                            theme_button,
                            material_button,
                            _subscription: subscription,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
