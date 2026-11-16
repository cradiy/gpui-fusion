use gpui::{prelude::*, *};
use gpui_fusion::{
    components::{
        button::Button,
        tabs::{Tab, Tabs},
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Page {
    title: SharedString,
    samples: usize,
    record: Entity<Button>,
}
impl Page {
    fn new(title: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let owner = cx.weak_entity();
        let record = cx.new(|cx| {
            Button::new("Record sample", window, cx).on_click(move |_, cx| {
                let _ = owner.update(cx, |page, cx| {
                    page.samples += 1;
                    cx.notify();
                });
            })
        });
        Self {
            title: title.into(),
            samples: 0,
            record,
        }
    }
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .items_start()
            .min_h(px(130.))
            .child(div().text_lg().child(self.title.clone()))
            .child(format!("Recorded samples: {}", self.samples))
            .child(self.record.clone())
    }
}
struct Gallery {
    theme: Entity<ThemeRuntime>,
    tabs: Vec<Entity<Tabs>>,
    toolbar: Vec<Entity<Button>>,
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
        div().id("tabs-gallery").size_full().overflow_y_scroll().p_8().flex().flex_col().gap_6().bg(color("canvas")).text_color(color("foreground"))
            .child(div().text_2xl().child("Tabs"))
            .child(div().flex().gap_3().children(self.toolbar.iter().cloned()))
            .child(if runtime.material_mode() == MaterialMode::Glass { "Theme material: Glass" } else { "Theme material: Plain" })
            .child(div().p_5().rounded_xl().bg(color("muted_surface")).flex().flex_col().gap_4()
                .child("Follow theme · automatic activation").child(self.tabs[0].clone()))
            .child(div().p_5().rounded_xl().bg(color("muted_surface")).flex().flex_col().gap_4()
                .child("Fixed plain · arrows focus, Enter selects").child(self.tabs[1].clone()))
            .child(div().p_5().rounded_xl().bg(color("muted_surface")).flex().flex_col().gap_4()
                .child("Fixed glass · scrolling tab bar").child(self.tabs[2].clone()))
            .child(div().text_sm().text_color(color("foreground_secondary")).child("Left / Right navigate · Home / End jump · Page state is retained when switching tabs"))
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
                        size(px(780.), px(850.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let mut controls = Vec::new();
                        for index in 0..3 {
                            let tabs = if index == 2 {
                                (0..9)
                                    .map(|n| {
                                        Tab::new(
                                            format!("channel-{n}"),
                                            format!("Channel {}", n + 1),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                ["Summary", "Signals", "Calibration"]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(n, title)| {
                                        let panel = cx.new(|cx| Page::new(title, window, cx));
                                        Tab::new(format!("page-{n}"), title).panel(panel).icon(
                                            match n {
                                                0 => GpuiIcons::LucideInfo.path(),
                                                1 => GpuiIcons::LucideActivity.path(),
                                                _ => GpuiIcons::LucideSettings.path(),
                                            },
                                        )
                                    })
                                    .chain([Tab::new("restricted", "Restricted").disabled(true)])
                                    .collect()
                            };
                            controls.push(cx.new(|cx| {
                                let tabs = Tabs::new("Instrument", tabs, window, cx).unwrap();
                                match index {
                                    1 => tabs
                                        .manual_activation(true)
                                        .material_mode(MaterialMode::Plain),
                                    2 => tabs.material_mode(MaterialMode::Glass),
                                    _ => tabs,
                                }
                            }));
                        }
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
                                                            == MaterialMode::Plain
                                                        {
                                                            MaterialMode::Glass
                                                        } else {
                                                            MaterialMode::Plain
                                                        },
                                                        cx,
                                                    );
                                                }
                                            });
                                        })
                                })
                            })
                            .collect();
                        Gallery {
                            _subscription: cx.observe(&theme, |_, _, cx| cx.notify()),
                            theme,
                            tabs: controls,
                            toolbar,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
