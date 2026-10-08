mod state;

use gpui::{
    AnyElement, App, ElementId, Entity, HitboxBehavior, IntoElement, ParentElement, Refineable,
    RenderOnce, Role, StyleRefinement, Styled, TouchEvent, Window, canvas, div, prelude::*, px,
};
use std::rc::Rc;
use std::time::Instant;

pub use state::{RefreshRequested, RefreshState, RefreshStatus};

/// A vertical scroll viewport with touch-driven pull-to-refresh.
#[derive(IntoElement)]
pub struct RefreshContainer {
    id: ElementId,
    state: Entity<RefreshState>,
    children: Vec<AnyElement>,
    indicator: Option<Rc<dyn Fn(RefreshStatus) -> AnyElement>>,
    style: StyleRefinement,
}

impl RefreshContainer {
    pub fn new(id: impl Into<ElementId>, state: &Entity<RefreshState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            children: Vec::new(),
            indicator: None,
            style: StyleRefinement::default(),
        }
    }

    /// Replaces the pull and refresh indicator. Its text style inherits from the viewport.
    pub fn indicator<E: IntoElement>(
        mut self,
        render: impl Fn(RefreshStatus) -> E + 'static,
    ) -> Self {
        self.indicator = Some(Rc::new(move |status| render(status).into_any_element()));
        self
    }
}

impl ParentElement for RefreshContainer {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for RefreshContainer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RefreshContainer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let status = state.status();
        let (offset, animating) = state.visual(Instant::now());
        let scroll = state.scroll_handle();
        if animating {
            window.request_animation_frame();
        }
        let label = match status {
            RefreshStatus::Idle | RefreshStatus::Pulling { .. } => "Pull to refresh",
            RefreshStatus::Ready => "Release to refresh",
            RefreshStatus::Refreshing => "Refreshing…",
        };
        let indicator = self.indicator.map_or_else(
            || div().text_sm().child(label).into_any_element(),
            |render| render(status),
        );
        let weak = self.state.downgrade();
        let listener = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                window.on_mouse_event(move |event: &TouchEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let inside = hitbox.bounds.contains(&event.position)
                        && hitbox.content_mask.bounds.contains(&event.position);
                    let _ = weak.update(cx, |state, cx| {
                        if state.touch(event, inside, window.default_prevented(), cx) {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    });
                });
            },
        )
        .absolute()
        .inset_0();
        let mut root = div().id(self.id.clone()).relative().size_full();
        root.style().refine(&self.style);
        root.overflow_hidden()
            // Register before the content so descendants have first refusal in bubble phase.
            .child(listener)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(offset)
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .children((offset > px(0.)).then(|| {
                        div()
                            .id("refresh-status")
                            .role(Role::Status)
                            .aria_label(label)
                            .child(indicator)
                    })),
            )
            .child(
                div()
                    .id((self.id, "scroll"))
                    .relative()
                    .top(offset)
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(self.children),
            )
    }
}
