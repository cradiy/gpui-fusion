use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div, point,
    prelude::*, px, rgb, rgba, size,
};
use gpui_effects::layout_transition;
use gpui_platform::application;

struct Card {
    id: usize,
    clicks: usize,
}

struct Preview {
    cards: Vec<Card>,
    next_id: usize,
    columns: usize,
    sidebar: bool,
    compact: bool,
    motion: bool,
}

impl Preview {
    fn new() -> Self {
        Self {
            cards: (0..6).map(|id| Card { id, clicks: 0 }).collect(),
            next_id: 6,
            columns: 2,
            sidebar: true,
            compact: false,
            motion: true,
        }
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = (f32::from(window.viewport_size().width) - 64.).max(240.);
        let sidebar_width = if self.sidebar && width > 650. {
            196.
        } else {
            54.
        };
        let board_width = width - sidebar_width - 20.;
        let columns = self
            .columns
            .min(((board_width + 16.) / 200.).floor().max(1.) as usize);
        let card_width = (board_width - 16. * (columns - 1) as f32) / columns as f32;
        let card_height = if self.compact { 152. } else { 204. };
        let rows = self.cards.len().div_ceil(columns).max(1);
        let height = (rows as f32 * (card_height + 16.) - 16.).max(380.);
        let control = |id, label: String| {
            div()
                .id(id)
                .px(px(13.))
                .py(px(10.))
                .rounded(px(9.))
                .bg(rgb(0x202736))
                .text_color(rgb(0xc8d2e6))
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(0x2b3548)))
                .child(label)
        };
        let cards = self
            .cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                let id = card.id;
                let color = [0xa2b8ff, 0x77dfc2, 0xffc38c][id % 3];
                let titles = [
                    "Brand system",
                    "Product page",
                    "Icon library",
                    "Mobile flow",
                    "Launch assets",
                    "Components",
                ];
                layout_transition(
                    ("card", id),
                    Bounds::new(
                        point(
                            px((index % columns) as f32 * (card_width + 16.)),
                            px((index / columns) as f32 * (card_height + 16.)),
                        ),
                        size(px(card_width), px(card_height)),
                    ),
                )
                .enabled(self.motion)
                .automation_id(format!("card-{id}"))
                .rounded(px(16.))
                .bg(rgb(0x171e2a))
                .border_1()
                .border_color(rgba(0xffffff16))
                .p(px(20.))
                .overflow_hidden()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(0x8190a8))
                                .child(format!("PROJECT {:02}", id + 1)),
                        )
                        .child(div().size(px(8.)).rounded_full().bg(rgb(color))),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .text_color(rgb(0xe5edfa))
                        .child(titles[id % titles.len()]),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .id("open")
                        .automation_id(format!("open-{id}"))
                        .text_size(px(12.))
                        .text_color(rgb(color))
                        .cursor_pointer()
                        .child(if card.clicks == 0 {
                            "Open project  →".into()
                        } else {
                            format!("Opened {} times", card.clicks)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(card) = this.cards.iter_mut().find(|card| card.id == id) {
                                card.clicks += 1;
                                cx.notify();
                            }
                        })),
                )
            })
            .collect::<Vec<_>>();
        div()
            .id("preview")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("LAYOUT & MOTION"),
            )
            .child(div().text_size(px(36.)).child("Room to rearrange."))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Reorder, resize, or change your mind halfway through."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(control("reverse", "Reverse".into()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cards.reverse();
                            cx.notify();
                        },
                    )))
                    .child(control("add", "+ Card".into()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cards.insert(
                                0,
                                Card {
                                    id: this.next_id,
                                    clicks: 0,
                                },
                            );
                            this.next_id += 1;
                            cx.notify();
                        },
                    )))
                    .child(control("remove", "Remove".into()).on_click(cx.listener(
                        |this, _, _, cx| {
                            if !this.cards.is_empty() {
                                this.cards.remove(0);
                                cx.notify();
                            }
                        },
                    )))
                    .child(
                        control("columns", format!("Columns: {}", self.columns)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.columns = if this.columns == 3 {
                                    1
                                } else {
                                    this.columns + 1
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        control(
                            "density",
                            if self.compact {
                                "Comfortable"
                            } else {
                                "Compact"
                            }
                            .into(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.compact = !this.compact;
                            cx.notify();
                        })),
                    )
                    .child(control("sidebar", "Sidebar".into()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.sidebar = !this.sidebar;
                            cx.notify();
                        },
                    )))
                    .child(
                        control(
                            "motion",
                            if self.motion {
                                "Motion: On"
                            } else {
                                "Motion: Off"
                            }
                            .into(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.motion = !this.motion;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(height))
                    .flex_shrink_0()
                    .child(
                        layout_transition(
                            "sidebar",
                            Bounds::new(point(px(0.), px(0.)), size(px(sidebar_width), px(height))),
                        )
                        .enabled(self.motion)
                        .rounded(px(16.))
                        .bg(rgb(0x111824))
                        .border_1()
                        .border_color(rgba(0xffffff10))
                        .p(px(16.))
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .gap(px(24.))
                        .child(div().size(px(22.)).rounded(px(6.)).bg(rgb(0x8faafa)))
                        .children((sidebar_width > 54.).then(|| {
                            div()
                                .w(px(150.))
                                .flex()
                                .flex_col()
                                .gap(px(18.))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(0x7889a5))
                                        .child("WORKSPACE"),
                                )
                                .child(div().text_size(px(14.)).child("All projects"))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(rgb(0x7889a5))
                                        .child("Shared with you"),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(rgb(0x7889a5))
                                        .child("Archive"),
                                )
                        })),
                    )
                    .child(
                        layout_transition(
                            "board",
                            Bounds::new(
                                point(px(sidebar_width + 20.), px(0.)),
                                size(px(board_width), px(height)),
                            ),
                        )
                        .enabled(self.motion)
                        .children(cards),
                    ),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1060.), px(820.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Layout transitions");
                cx.new(|_| Preview::new())
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
