use gpui::{prelude::*, *};
use std::time::Duration;
use uic::components::refresh::{RefreshContainer, RefreshRequested, RefreshState};

struct Example {
    refresh: Entity<RefreshState>,
    completed: usize,
    _subscription: Subscription,
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let refresh = cx.new(|cx| RefreshState::new(window, cx));
        let subscription = cx.subscribe(&refresh, |_, state, _: &RefreshRequested, cx| {
            let state = state.downgrade();
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let _ = this.update(cx, |this, cx| {
                    this.completed += 1;
                    let _ = state.update(cx, |state, cx| state.finish(cx));
                    cx.notify();
                });
            })
            .detach();
        });
        Self {
            refresh,
            completed: 0,
            _subscription: subscription,
        }
    }
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let state = self.refresh.clone();
        div().size_full().p_6().bg(rgb(0xf4f7fb)).text_color(rgb(0x172033))
            .flex().flex_col().gap_4()
            .child(div().text_2xl().child("Documents"))
            .child("Pull down with a touchscreen at the top of the list. Mouse wheels scroll normally.")
            .child(div().flex().items_center().gap_4()
                .child(div().id("refresh-now").role(Role::Button).aria_label("Refresh now")
                    .focusable().tab_stop(true).px_4().py_2().rounded_lg().bg(rgb(0x2563eb))
                    .text_color(rgb(0xffffff)).cursor_pointer()
                    .on_click(move |_, _, cx| { state.update(cx, |state, cx| { state.request(cx); }); })
                    .child("Refresh now"))
                .child(format!("{} refreshes completed", self.completed)))
            .child(RefreshContainer::new("documents", &self.refresh).flex_1().min_h_0()
                .rounded_xl().bg(rgb(0xffffff))
                .children((1..=40).map(|index| div().h(px(64.)).px_5().flex().items_center()
                    .border_b_1().border_color(rgb(0xe3eaf3)).child(format!("Document {index:02}")))))
    }
}

fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| Example::new(window, cx))
        })
        .expect("failed to open refresh example");
    });
}
