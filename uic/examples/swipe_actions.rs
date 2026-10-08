use gpui::{prelude::*, *};
use uic::assets::{LucideAssets, LucideIcons};
use uic::components::swipe_actions::{
    SwipeActions, SwipeActionsState, SwipeDirection, SwipeTriggered,
};

struct Document {
    state: Entity<SwipeActionsState>,
    pinned: bool,
    archived: bool,
}

struct Example {
    rows: Vec<Document>,
    message: String,
    _subscriptions: Vec<Subscription>,
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rows: Vec<_> = (0..16)
            .map(|_| Document {
                state: cx.new(|cx| SwipeActionsState::new(window, cx)),
                pinned: false,
                archived: false,
            })
            .collect();
        let subscriptions = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                cx.subscribe(&row.state, move |this, _, event: &SwipeTriggered, cx| {
                    let row = &mut this.rows[index];
                    let action = match event.direction {
                        SwipeDirection::Left => {
                            row.archived = true;
                            "Archived"
                        }
                        SwipeDirection::Right => {
                            row.pinned = !row.pinned;
                            if row.pinned { "Pinned" } else { "Unpinned" }
                        }
                    };
                    this.message = format!("{action} · Document {:02}", index + 1);
                    cx.notify();
                })
            })
            .collect();
        Self {
            rows,
            message: "Your files, within reach.".into(),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xf2f5f8))
            .text_color(rgb(0x1c2c42))
            .font_family("sans-serif")
            .p_6()
            .flex()
            .flex_col()
            .items_center()
            .gap_5()
            .child(
                div()
                    .w_full()
                    .max_w(px(680.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().text_color(rgb(0x4b6b8b)).child("LIBRARY"))
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Documents"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x687b90))
                            .whitespace_normal()
                            .child("Swipe left to archive, right to pin. Release when the hint is ready."),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .max_w(px(680.))
                    .text_sm()
                    .text_color(rgb(0x426386))
                    .child(self.message.clone())
                    .child(div().id("restore").role(Role::Button).aria_label("Restore archived documents")
                        .focusable().tab_stop(true).mt_3().cursor_pointer().child("Restore archived")
                        .on_click(cx.listener(|this, _, _, cx| {
                            for row in &mut this.rows { row.archived = false; }
                            this.message = "All documents restored.".into();
                            cx.notify();
                        }))),

            )
            .child(
                div()
                    .id("documents")
                    .w_full()
                    .max_w(px(680.))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(self.rows.iter().enumerate().map(|(index, row)| {
                        let pinned = row.pinned;
                        let content = div()
                            .id(("document", index))
                            .h(px(84.))
                            .w_full()
                            .px_4()
                            .bg(rgb(0xffffff))
                            .rounded_xl()
                            .flex()
                            .items_center()
                            .gap_4()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.message = format!("Opened Document {:02}", index + 1);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(42.))
                                    .rounded_lg()
                                    .bg(rgb(0xe9eff8))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(0x4a6a94))
                                    .child(format!("{:02}", index + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("Document {:02}", index + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x8390a1))
                                            .child(if pinned { "Pinned · Workspace" } else { "Workspace · Updated today" }),
                                    ),
                            )
                            .child(div().text_sm().text_color(rgb(0x347b79))
                                .child(if pinned { "Pinned" } else { "" }));
                        SwipeActions::new(("swipe", index), &row.state, content)
                            .flex_shrink_0()
                            .mb_3()
                            .rounded_xl()
                            .threshold(px(120.))
                            .dismissed(row.archived.then_some(SwipeDirection::Left))
                            .feedback(move |swipe, _, _| {
                                let left = swipe.direction == SwipeDirection::Left;
                                let accent = if left { 0x5573a3 } else { 0x348578 };
                                let icon = if swipe.ready { LucideIcons::Check }
                                    else if left { LucideIcons::Archive } else { LucideIcons::Pin };
                                div().relative().size_full().rounded_xl()
                                    .bg(rgb(if swipe.ready { accent } else if left { 0xe5ecf6 } else { 0xe1efea }))
                                    .text_color(rgb(if swipe.ready { 0xffffff } else { accent }))
                                    .child(div().absolute().top_0().bottom_0()
                                        .when(left, |this| this.right_0())
                                        .when(!left, |this| this.left_0())
                                        .w(px(f32::from(swipe.displacement).abs())).max_w_full()
                                        .flex().items_center().justify_center().gap_2()
                                        .text_sm().font_weight(FontWeight::SEMIBOLD)
                                        .child(svg().path(icon.path()).size(px(22.)).flex_shrink_0())
                                        .child(if left { "Archive" } else if pinned { "Unpin" } else { "Pin" }))
                            })
                    })),
            )
    }
}

#[gpui_platform::main]
fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();
    gpui_platform::application()
        .with_assets(LucideAssets::new())
        .run(|cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| Example::new(window, cx))
            })
            .expect("open swipe actions example");
        });
}
