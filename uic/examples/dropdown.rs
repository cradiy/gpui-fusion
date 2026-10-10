use gpui::{
    AppContext, Context, Entity, IntoElement, Render, SharedString, Window, WindowOptions, div,
    prelude::*, px, rgb,
};
use uic::components::dropdown::{DropdownPlacement, DropdownState, dropdown};

struct DropdownExample {
    actions: Entity<DropdownState>,
    placement: Entity<DropdownState>,
    selected_action: SharedString,
}

impl DropdownExample {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            actions: cx.new(|cx| DropdownState::new(window, cx)),
            placement: cx.new(|cx| DropdownState::new(window, cx)),
            selected_action: "No action selected".into(),
        }
    }

    fn action_item(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let dropdown_state = self.actions.clone();

        div()
            .id(id)
            .w_full()
            .px_3()
            .py_2()
            .rounded_md()
            .text_color(rgb(0x1e293b))
            .hover(|style| style.bg(rgb(0xf1f5f9)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected_action = label.into();
                dropdown_state.update(cx, |state, cx| state.close(window, cx));
                cx.notify();
            }))
            .child(label)
    }
}

impl Render for DropdownExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_8()
            .bg(rgb(0xf8fafc))
            .text_color(rgb(0x0f172a))
            .child(
                div()
                    .text_lg()
                    .child(format!("Selected: {}", self.selected_action)),
            )
            .child(
                dropdown(&self.actions)
                    .trigger(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x2563eb))
                            .text_color(rgb(0xffffff))
                            .child("Actions ▾"),
                    )
                    .menu(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.action_item("rename", "Rename", cx))
                            .child(self.action_item("download", "Download", cx))
                            .child(self.action_item("delete", "Delete", cx)),
                    ),
            )
            .child(
                dropdown(&self.placement)
                    .placement(DropdownPlacement::TopEnd)
                    .menu_gap(px(8.))
                    .min_w(px(180.))
                    .max_h(px(260.))
                    .p(px(6.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(0x334155))
                    .bg(rgb(0x111c2f))
                    .trigger(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(0x334155))
                            .child("Custom theme (top end)"),
                    )
                    .menu(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .text_color(rgb(0xe2e8f0))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .child("Arbitrary content"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .child("Any GPUI element works"),
                            ),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x64748b))
                    .child("Click outside or press Escape to close."),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| DropdownExample::new(window, cx))
        })
        .expect("failed to open dropdown example window");
    });
}
