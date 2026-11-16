use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        input::{Input, InputState},
        popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide},
        select::{Select, SelectOption},
        switch::Switch,
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::IconAssets;

struct Gallery {
    theme: Entity<ThemeRuntime>,
    buttons: Vec<Entity<Button>>,
    panels: Vec<Entity<Popover>>,
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
        div().size_full().p_8().flex().flex_col().gap_6().bg(color("canvas")).text_color(color("foreground"))
            .child(div().text_2xl().child("Popovers"))
            .child(format!("Material: {:?}", theme.material_mode()))
            .child(div().flex().gap_3().children(self.buttons.iter().cloned()))
            .child(div().relative().flex_1().rounded_3xl().bg(color("surface")).p_6().flex().flex_col().justify_between()
                .child(div().absolute().left(px(180.)).top(px(90.)).size(px(240.)).rounded_full().bg(color("primary_subtle")))
                .child(div().absolute().left(px(70.)).bottom(px(50.)).w(px(300.)).h(px(160.)).rounded_3xl().bg(color("muted_surface")))
                .child(div().relative().flex().justify_between().child(self.panels[0].clone()).child(self.panels[1].clone()))
                .child(div().relative().flex().justify_end().child(self.panels[2].clone())))
            .child(div().text_sm().text_color(color("foreground_secondary")).child("Esc returns to the button · Click outside to close · Panels fit the available space"))
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
            theme.update(cx, |t, c| t.set_material_mode(MaterialMode::Glass, c));
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(900.), px(760.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |w, cx| {
                    cx.new(|cx| {
                        let name = cx.new(|c| InputState::new(c).placeholder("File name"));
                        let field = cx.new(|c| Input::new(&name, w, c));
                        let initial = name.focus_handle(cx);
                        let format = cx.new(|c| {
                            Select::new(
                                "Format",
                                [
                                    SelectOption::new("vector", "Vector"),
                                    SelectOption::new("bitmap", "Bitmap"),
                                    SelectOption::new("document", "Document"),
                                ],
                                w,
                                c,
                            )
                            .unwrap()
                        });
                        let export = cx.new(|c: &mut Context<Popover>| {
                            let weak = c.weak_entity();
                            let done = c.new(|c| {
                                Button::new("Done", w, c).on_click(move |w, c| {
                                    let weak = weak.clone();
                                    w.defer(c, move |w, c| {
                                        let _ = weak.update(c, |p, c| p.close(w, c));
                                    });
                                })
                            });
                            Popover::new(
                                "Export options",
                                move |_, _| {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child("Name")
                                        .child(field.clone())
                                        .child("Format")
                                        .child(format.clone())
                                },
                                w,
                                c,
                            )
                            .title("Export options")
                            .initial_focus(initial)
                            .footer(move |_, _| done.clone())
                        });
                        let snapping = cx.new(|c| Switch::new("Snap to grid", true, w, c));
                        let guides = cx.new(|c| Switch::new("Show guides", false, w, c));
                        let settings = cx.new(|c| {
                            Popover::new(
                                "Canvas settings",
                                move |_, _| {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .child(snapping.clone())
                                        .child(guides.clone())
                                },
                                w,
                                c,
                            )
                            .title("Canvas settings")
                            .placement(
                                PopoverPlacement::new(PopoverSide::Left).align(PopoverAlign::Start),
                            )
                        });
                        let folders = cx.new(|c| {
                            Popover::new(
                                "Recent folders",
                                |_, _| {
                                    div().flex().flex_col().gap_2().children(
                                        (1..=18).map(|n| {
                                            div().py_2().child(format!("Collection {n:02}"))
                                        }),
                                    )
                                },
                                w,
                                c,
                            )
                            .title("Recent folders")
                            .placement(
                                PopoverPlacement::new(PopoverSide::Bottom).align(PopoverAlign::End),
                            )
                        });
                        let theme_button = {
                            let theme = theme.clone();
                            cx.new(|c| {
                                Button::new("Toggle theme", w, c)
                                    .material_mode(MaterialMode::Plain)
                                    .on_click(move |_, c| {
                                        theme.update(c, |t, c| {
                                            t.set_mode(
                                                if t.store().mode() == "light" {
                                                    "dark"
                                                } else {
                                                    "light"
                                                },
                                                c,
                                            )
                                            .unwrap();
                                        })
                                    })
                            })
                        };
                        let material_button = {
                            let theme = theme.clone();
                            cx.new(|c| {
                                Button::new("Toggle material", w, c)
                                    .material_mode(MaterialMode::Plain)
                                    .on_click(move |_, c| {
                                        theme.update(c, |t, c| {
                                            t.set_material_mode(
                                                if t.material_mode() == MaterialMode::Plain {
                                                    MaterialMode::Glass
                                                } else {
                                                    MaterialMode::Plain
                                                },
                                                c,
                                            );
                                        })
                                    })
                            })
                        };
                        export.update(cx, |p, c| p.open(w, c));
                        Gallery {
                            _subscription: cx.observe(&theme, |_, _, c| c.notify()),
                            theme,
                            buttons: vec![theme_button, material_button],
                            panels: vec![export, settings, folders],
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
