use gpui::{prelude::*, *};
use gpui_fusion::{
    components::dropdown::{
        Dropdown, DropdownAlign, DropdownPlacement, DropdownPresentation, DropdownSide, MenuItem,
    },
    default_theme,
    style::{MaterialMode, Token},
    theme::{ResolveContext, ThemeStore, runtime::ThemeRuntime},
};
use gpui_icons::{GpuiIcons, IconAssets};

struct Gallery {
    theme: Entity<ThemeRuntime>,
    menus: Vec<Entity<Dropdown>>,
    status: SharedString,
    placement: usize,
    grid: bool,
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
        let surface = color("surface");
        div()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .bg(color("muted_surface"))
            .text_color(color("foreground"))
            .child(div().text_size(px(26.)).child("Dropdown menu"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        div()
                            .id("theme")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(surface)
                            .child(if dark { "Light theme" } else { "Dark theme" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.theme.update(cx, |theme, cx| {
                                    let next = if theme.store().mode() == "dark" {
                                        "light"
                                    } else {
                                        "dark"
                                    };
                                    theme.set_mode(next, cx).unwrap();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("placement")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(surface)
                            .child(
                                [
                                    "Position: default",
                                    "Position: below / end",
                                    "Position: right / center",
                                    "Position: above",
                                    "Position: overlap / center",
                                ][self.placement],
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.placement = (this.placement + 1) % 5;
                                let placement = match this.placement {
                                    1 => Some(
                                        DropdownPlacement::new(DropdownSide::Bottom)
                                            .align(DropdownAlign::End)
                                            .gap(px(10.)),
                                    ),
                                    2 => Some(
                                        DropdownPlacement::new(DropdownSide::Right)
                                            .align(DropdownAlign::Center)
                                            .gap(px(12.)),
                                    ),
                                    3 => Some(DropdownPlacement::new(DropdownSide::Top)),
                                    4 => Some(
                                        DropdownPlacement::new(DropdownSide::Overlay)
                                            .align(DropdownAlign::Center),
                                    ),
                                    _ => None,
                                };
                                for menu in &this.menus {
                                    menu.update(cx, |menu, cx| menu.set_placement(placement, cx));
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("material")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(surface)
                            .child(if glass {
                                "Material: glass"
                            } else {
                                "Material: plain"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.theme.update(cx, |theme, cx| {
                                    let next = if theme.material_mode() == MaterialMode::Glass {
                                        MaterialMode::Plain
                                    } else {
                                        MaterialMode::Glass
                                    };
                                    theme.set_material_mode(next, cx);
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("backdrop")
                            .focusable()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(surface)
                            .child(if self.grid {
                                "Backdrop: grid"
                            } else {
                                "Backdrop: shapes"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.grid = !this.grid;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .rounded(px(24.))
                    .bg(surface)
                    .overflow_hidden()
                    .when(self.grid, |panel| {
                        let ink = color("foreground").opacity(0.14);
                        panel.child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let cell = px(28.);
                                    let rows = (bounds.size.height / cell).ceil() as usize;
                                    let columns = (bounds.size.width / cell).ceil() as usize;
                                    for row in 0..rows {
                                        for column in (row % 2..columns).step_by(2) {
                                            let origin = bounds.origin
                                                + point(cell * column as f32, cell * row as f32);
                                            window.paint_quad(fill(
                                                Bounds::new(origin, size(cell, cell)),
                                                ink,
                                            ));
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .right(px(70.))
                            .top(px(40.))
                            .size(px(200.))
                            .rounded_full()
                            .bg(color("primary").opacity(0.12)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(60.))
                            .top(px(125.))
                            .w(px(260.))
                            .h(px(120.))
                            .rounded(px(36.))
                            .bg(color("primary").opacity(0.06)),
                    )
                    .child(
                        div()
                            .relative()
                            .p_8()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap_6()
                            .child(
                                div()
                                    .text_color(color("foreground_muted"))
                                    .child("Navigate your workspace"),
                            )
                            .child(div().flex().flex_wrap().gap_8().children(
                                self.menus.iter().enumerate().map(|(index, menu)| {
                                    div()
                                        .w(px(260.))
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap_4()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(color("foreground_muted"))
                                                .child(if index == 0 {
                                                    "Keep the button"
                                                } else {
                                                    "Expand from the button"
                                                }),
                                        )
                                        .child(menu.clone())
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(color("foreground_muted"))
                    .child(self.status.clone()),
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
            theme.update(cx, |theme, cx| {
                theme.set_material_mode(MaterialMode::Glass, cx)
            });
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(680.), px(540.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let menus = [DropdownPresentation::Popover, DropdownPresentation::Morph]
                            .into_iter()
                            .map(|presentation| {
                                let owner = cx.weak_entity();
                                cx.new(|cx| {
                                    Dropdown::new(
                                        "Workspace",
                                        [
                                            MenuItem::new("back", "Previous location")
                                                .icon(GpuiIcons::LucideChevronLeft.path()),
                                            MenuItem::new("forward", "Next location")
                                                .icon(GpuiIcons::LucideChevronRight.path())
                                                .disabled(true),
                                            MenuItem::Separator,
                                            MenuItem::new("details", "Workspace details")
                                                .icon(GpuiIcons::LucideInfo.path()),
                                            MenuItem::new("close", "Close workspace")
                                                .icon(GpuiIcons::LucideX.path()),
                                        ],
                                        window,
                                        cx,
                                    )
                                    .presentation(presentation)
                                    .open_label("Close workspace menu")
                                    .on_action(
                                        move |value, _, cx| {
                                            let _ = owner.update(cx, |view: &mut Gallery, cx| {
                                                view.status = format!("Command: {value}").into();
                                                cx.notify();
                                            });
                                        },
                                    )
                                })
                            })
                            .collect::<Vec<_>>();
                        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
                        Gallery {
                            theme,
                            menus,
                            placement: 0,
                            grid: true,
                            status: "Arrow keys navigate · Enter selects · Esc closes".into(),
                            _subscription: subscription,
                        }
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
