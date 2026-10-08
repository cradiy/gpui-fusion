use gpui::{prelude::*, *};
use std::time::Duration;
use uic::components::refresh::{RefreshContainer, RefreshRequested, RefreshState};

pub struct RefreshDemo {
    state: Entity<RefreshState>,
    completed: usize,
    clicks: usize,
    _subscription: Subscription,
}

impl RefreshDemo {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| RefreshState::new(window, cx));
        let subscription = cx.subscribe(&state, |_, state, _: &RefreshRequested, cx| {
            let state = state.downgrade();
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    this.completed += 1;
                    let _ = state.update(cx, |state, cx| state.finish(cx));
                    cx.notify();
                });
            })
            .detach();
        });
        Self {
            state,
            completed: 0,
            clicks: 0,
            _subscription: subscription,
        }
    }
}

impl Render for RefreshDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .text_color(rgb(0x172033))
            .child(
                div()
                    .flex_shrink_0()
                    .text_sm()
                    .child("Pull down at the top. Scroll up to browse the list."),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .id("refresh-summary")
                    .role(Role::Status)
                    .aria_label(format!(
                        "Refreshes: {} · Clicks: {}",
                        self.completed, self.clicks
                    ))
                    .child(format!(
                        "Refreshes: {} · Clicks: {}",
                        self.completed, self.clicks
                    )),
            )
            .child(
                super::button("request-refresh", "Refresh now")
                    .flex_shrink_0()
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            state.request(cx);
                        });
                    }),
            )
            .child(
                RefreshContainer::new("refresh-list", &self.state)
                    .flex_1()
                    .min_h_0()
                    .rounded_lg()
                    .bg(rgb(0xf4f7fb))
                    .children((1_usize..=30).map(|index| {
                        div()
                            .id(("refresh-row", index))
                            .role(Role::Button)
                            .aria_label(format!("Document {index:02}"))
                            .h(px(64.))
                            .px_4()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xe3eaf3))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clicks += 1;
                                cx.notify();
                            }))
                            .child(format!("Document {index:02}"))
                    })),
            )
    }
}
