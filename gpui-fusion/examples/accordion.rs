use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        accordion::{Accordion, AccordionItem, AccordionMode},
        button::Button,
        input::{Input, InputState},
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Details {
    field: Entity<Input>,
    add: Entity<Button>,
    lines: usize,
}
impl Details {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            InputState::new(cx)
                .aria_label("Description")
                .placeholder("Enter a description")
        });
        let field = cx.new(|cx| Input::new(&editor, window, cx));
        let owner = cx.weak_entity();
        let add = cx.new(|cx| {
            Button::new("Add detail", window, cx).on_click(move |_, cx| {
                let _ = owner.update(cx, |details, cx| {
                    details.lines += 1;
                    cx.notify();
                });
            })
        });
        Self {
            field,
            add,
            lines: 0,
        }
    }
}
impl Render for Details {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.field.clone())
            .child(div().flex().child(self.add.clone()))
            .children((0..self.lines).map(|i| div().text_sm().child(format!("Detail {}", i + 1))))
    }
}
struct Gallery {
    theme: Entity<ThemeRuntime>,
    controls: Vec<Entity<Accordion>>,
    toolbar: Vec<Entity<Button>>,
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
        let labels = [
            "Single · follow theme",
            "Multiple · fixed plain",
            "Multiple · fixed glass",
        ];
        div().id("accordion-gallery").size_full().overflow_y_scroll().p_8().flex().flex_col().gap_6().bg(color("canvas")).text_color(color("foreground"))
            .child(div().text_2xl().child("Accordion"))
            .child(div().flex().gap_3().children(self.toolbar.iter().cloned()))
            .child(if theme.material_mode()==MaterialMode::Glass { "Theme material: Glass" } else { "Theme material: Plain" })
            .children(self.controls.iter().zip(labels).map(|(control,label)| div().p_5().rounded_xl().bg(color("muted_surface")).flex().flex_col().gap_4().child(label).child(control.clone())))
            .child(div().text_sm().text_color(color("foreground_secondary")).child("Enter / Space toggle · Up / Down navigate · Content is retained when collapsed"))
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
                        size(px(780.), px(900.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let controls = (0..3)
                            .map(|index| {
                                let items = [
                                    ("output", "Output profile", GpuiIcons::LucideSettings),
                                    ("metadata", "Metadata", GpuiIcons::LucideInfo),
                                    ("diagnostics", "Diagnostics", GpuiIcons::LucideActivity),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(n, (value, label, icon))| {
                                    let content = cx.new(|cx| Details::new(window, cx));
                                    AccordionItem::new(value, label, content)
                                        .icon(icon.path())
                                        .disabled(n == 2)
                                })
                                .collect::<Vec<_>>();
                                cx.new(|cx| {
                                    let mut control =
                                        Accordion::new("Export configuration", items, window, cx)
                                            .unwrap();
                                    if index > 0 {
                                        control = control
                                            .mode(AccordionMode::Multiple)
                                            .material_mode(if index == 1 {
                                                MaterialMode::Plain
                                            } else {
                                                MaterialMode::Glass
                                            });
                                    }
                                    if index == 0 {
                                        control.set_expanded(["output"], window, cx).unwrap();
                                    }
                                    control
                                })
                            })
                            .collect();
                        let toolbar = ["Toggle theme", "Toggle material"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                let theme = theme.clone();
                                cx.new(|cx| {
                                    Button::new(label, window, cx)
                                        .material_mode(MaterialMode::Plain)
                                        .on_click(move |_, cx| {
                                            theme.update(cx, |theme, cx| {
                                                if index == 0 {
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
                                                } else {
                                                    theme.set_material_mode(
                                                        if theme.material_mode()
                                                            == MaterialMode::Glass
                                                        {
                                                            MaterialMode::Plain
                                                        } else {
                                                            MaterialMode::Glass
                                                        },
                                                        cx,
                                                    );
                                                }
                                            })
                                        })
                                })
                            })
                            .collect();
                        Gallery {
                            _subscription: cx.observe(&theme, |_, _, cx| cx.notify()),
                            theme,
                            controls,
                            toolbar,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
