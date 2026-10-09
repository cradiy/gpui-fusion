mod input;
mod state;

use gpui::{
    AnyElement, App, ElementId, Entity, HitboxBehavior, IntoElement, Refineable, RenderOnce, Role,
    StyleRefinement, Styled, Window, canvas, div, prelude::*, relative,
};

pub use state::{PageChanged, PagerState};

type PageRenderer = Box<dyn FnMut(usize, &mut Window, &mut App) -> AnyElement>;

/// A horizontal viewport with draggable pages and animated page snapping.
/// The viewport needs a finite size; each page fills its content area.
/// Only pages intersecting the viewport are built. Retain page entities outside the renderer.
#[derive(IntoElement)]
pub struct Pager {
    id: ElementId,
    state: Entity<PagerState>,
    render_page: PageRenderer,
    drag_enabled: bool,
    style: StyleRefinement,
}

impl Pager {
    pub fn new<E: IntoElement>(
        id: impl Into<ElementId>,
        state: &Entity<PagerState>,
        mut render_page: impl FnMut(usize, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            render_page: Box::new(move |index, window, cx| {
                render_page(index, window, cx).into_any_element()
            }),
            drag_enabled: true,
            style: StyleRefinement::default(),
        }
    }

    /// Enables touch and left-button dragging. Programmatic navigation remains available.
    pub fn drag_enabled(mut self, enabled: bool) -> Self {
        self.drag_enabled = enabled;
        self
    }
}

impl Styled for Pager {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Pager {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            state.configure(self.drag_enabled, !window.prefers_reduced_motion(), cx)
        });
        let state = self.state.read(cx);
        let count = state.page_count();
        let (position, animating) = state.visual(cx.background_executor().now());
        if animating {
            window.request_animation_frame();
        }
        let start = (position.floor().max(0.) as usize).min(count);
        let end = (position.ceil().max(0.) as usize)
            .saturating_add(1)
            .min(count);
        let pages = (start..end)
            .map(|index| {
                div()
                    .id((self.id.clone(), index.to_string()))
                    .absolute()
                    .top_0()
                    .left(relative(index as f32 - position))
                    .size_full()
                    .role(Role::Group)
                    .aria_label(format!("Page {} of {}", index + 1, count))
                    .child((self.render_page)(index, window, cx))
            })
            .collect::<Vec<_>>();
        let weak = self.state.downgrade();
        let measure = weak.clone();
        let listener = canvas(
            move |bounds, window, cx| {
                let _ = measure.update(cx, |state, cx| state.measure(bounds.size.width, cx));
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |_, hitbox, window, _| input::register(weak, hitbox, window),
        )
        .absolute()
        .inset_0();
        let mut root = div().id(self.id.clone()).relative().size_full();
        root.style().refine(&self.style);
        root.overflow_hidden().child(
            div()
                .relative()
                .size_full()
                .overflow_hidden()
                .child(listener)
                .children(pages),
        )
    }
}

#[cfg(test)]
mod tests;
