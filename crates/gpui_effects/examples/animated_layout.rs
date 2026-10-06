use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, Render, Stateful, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_effects::animated_layout;
use gpui_platform::application;

struct Card {
    id: usize,
    saved: bool,
    clicks: usize,
}

struct Preview {
    cards: Vec<Card>,
    next_id: usize,
    saved_only: bool,
    grid: bool,
    columns: u16,
    slow: bool,
    motion: bool,
}

fn button(id: &'static str, label: impl Into<gpui::SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(rgb(0x202736))
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x303a4c)))
        .child(label.into())
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(600.);
        let columns = if narrow { 1 } else { self.columns };
        let mut group = animated_layout("cards")
            .w_full()
            .gap(px(16.))
            .duration(Duration::from_millis(if self.slow { 1000 } else { 320 }))
            .enabled(self.motion)
            .when(self.grid, |group| group.grid().grid_cols(columns))
            .when(!self.grid, |group| group.flex().flex_wrap());
        let mut visible = 0;
        for card in &self.cards {
            if self.saved_only && !card.saved {
                continue;
            }
            visible += 1;
            let id = card.id;
            let accent = [0xa2b8ff, 0x77dfc2, 0xffc38c][id % 3];
            group = group.item(
                id,
                div()
                    .id("card")
                    .min_w_0()
                    .min_h(px(200.))
                    .p(px(20.))
                    .rounded(px(22.))
                    .when(self.grid || narrow, |card| card.w_full())
                    .when(!self.grid && !narrow, |card| {
                        card.w(px(215.)).flex_shrink_0()
                    })
                    .bg(rgb([0x1c293e, 0x1a302d, 0x322a25][id % 3]))
                    .border_1()
                    .border_color(rgb([0x344965, 0x34544b, 0x55463a][id % 3]))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .cursor_pointer()
                    .hover(|s| s.border_color(rgb(accent)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(card) = this.cards.iter_mut().find(|card| card.id == id) {
                            card.clicks += 1;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .text_color(rgb(accent))
                            .text_size(px(12.))
                            .child(format!("IDEA {:02}", id + 1))
                            .child(
                                div()
                                    .id("save")
                                    .size(px(28.))
                                    .rounded(px(8.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(18.))
                                    .hover(|s| s.bg(gpui::rgba(0xffffff10)))
                                    .child(if card.saved { "★" } else { "☆" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(card) =
                                            this.cards.iter_mut().find(|card| card.id == id)
                                        {
                                            card.saved = !card.saved;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .child(div().text_size(px(23.)).child(
                        [
                            "Quiet corners",
                            "A steady rhythm",
                            "New directions",
                            "Small wonders",
                            "Room to grow",
                            "A different angle",
                            "Bright beginnings",
                            "Collected thoughts",
                        ][id % 8],
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(0xa4b2c8))
                            .child(format!("{} clicks", card.clicks)),
                    ),
            );
        }
        div()
            .size_full()
            .p(px(32.))
            .bg(rgb(0x0c1018))
            .text_color(rgb(0xecf1fc))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(rgb(0x929fbc))
                    .child("ANIMATED LAYOUT"),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(38.))
                    .child("Make room for a change."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(0x8e9ab1))
                    .child("Reorder your ideas. Save a few. Click a card while it moves."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button("reverse", "Reverse").on_click(cx.listener(|this, _, _, cx| {
                            this.cards.reverse();
                            cx.notify();
                        })),
                    )
                    .child(button("rotate", "Rotate order").on_click(cx.listener(
                        |this, _, _, cx| {
                            if !this.cards.is_empty() {
                                this.cards.rotate_left(1);
                            }
                            cx.notify();
                        },
                    )))
                    .child(
                        button("add", "Add  +").on_click(cx.listener(|this, _, _, cx| {
                            this.cards.insert(
                                0,
                                Card {
                                    id: this.next_id,
                                    saved: this.saved_only,
                                    clicks: 0,
                                },
                            );
                            this.next_id += 1;
                            cx.notify();
                        })),
                    )
                    .child(button("remove", "Remove last").on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(index) = this
                                .cards
                                .iter()
                                .rposition(|card| !this.saved_only || card.saved)
                            {
                                this.cards.remove(index);
                            }
                            cx.notify();
                        },
                    )))
                    .child(
                        button(
                            "filter",
                            if self.saved_only {
                                "Show: Saved"
                            } else {
                                "Show: All"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.saved_only = !this.saved_only;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(
                        button(
                            "layout",
                            if self.grid {
                                "Layout: Grid"
                            } else {
                                "Layout: Wrap"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.grid = !this.grid;
                            cx.notify();
                        })),
                    )
                    .when(self.grid && !narrow, |row| {
                        row.child(button("columns", format!("Columns: {}", columns)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.columns = if this.columns == 3 { 2 } else { 3 };
                                cx.notify();
                            }),
                        ))
                    })
                    .child(
                        button(
                            "pace",
                            if self.slow {
                                "Pace: Slow"
                            } else {
                                "Pace: Natural"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.slow = !this.slow;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "motion",
                            if self.motion {
                                "Motion: On"
                            } else {
                                "Motion: Off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.motion = !this.motion;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .id("board")
                    .w_full()
                    .max_w(px(800.))
                    .min_h_0()
                    .flex_1()
                    .overflow_y_scroll()
                    .rounded(px(26.))
                    .bg(rgb(0x111925))
                    .border_1()
                    .border_color(rgb(0x253145))
                    .child(div().p(px(20.)).child(group).when(visible == 0, |content| {
                        content.child(
                            div()
                                .py(px(40.))
                                .text_size(px(14.))
                                .text_color(rgb(0x8e9ab1))
                                .child("Nothing here yet. Add an idea or show all cards."),
                        )
                    })),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(950.)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Animated layout");
                cx.new(|_| Preview {
                    cards: (0..6)
                        .map(|id| Card {
                            id,
                            saved: id % 2 == 0,
                            clicks: 0,
                        })
                        .collect(),
                    next_id: 6,
                    saved_only: false,
                    grid: true,
                    columns: 3,
                    slow: false,
                    motion: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
