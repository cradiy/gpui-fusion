mod input;
mod state;

use gpui::{
    AnyElement, App, ElementId, Entity, HitboxBehavior, IntoElement, Pixels, Refineable,
    RenderOnce, StyleRefinement, Styled, Window, canvas, div, prelude::*, px,
};
use gpui_effects::animated_collapse;

pub use state::{SwipeActionsState, SwipeDirection, SwipeProgress, SwipeTriggered};

type Feedback = Box<dyn FnOnce(SwipeProgress, &mut Window, &mut App) -> AnyElement>;

/// A row that triggers a directional action when released past a swipe threshold.
#[derive(IntoElement)]
pub struct SwipeActions {
    id: ElementId,
    state: Entity<SwipeActionsState>,
    content: AnyElement,
    enabled: [bool; 2],
    threshold: Pixels,
    feedback: Option<Feedback>,
    dismissed: Option<SwipeDirection>,
    style: StyleRefinement,
}

impl SwipeActions {
    pub fn new(
        id: impl Into<ElementId>,
        state: &Entity<SwipeActionsState>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            content: content.into_any_element(),
            enabled: [true; 2],
            threshold: px(96.),
            feedback: None,
            dismissed: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn left_enabled(mut self, enabled: bool) -> Self {
        self.enabled[0] = enabled;
        self
    }

    pub fn right_enabled(mut self, enabled: bool) -> Self {
        self.enabled[1] = enabled;
        self
    }

    /// Release distance in logical pixels. Defaults to 96; finite values are at least 16.
    pub fn threshold(mut self, distance: Pixels) -> Self {
        self.threshold = distance;
        self
    }

    /// Slides out in this direction, then collapses the row. Keep the element mounted
    /// with a stable ID; `None` restores it. Include spacing in the row's margin.
    pub fn dismissed(mut self, direction: Option<SwipeDirection>) -> Self {
        self.dismissed = direction;
        self
    }

    /// Draws feedback behind the moving content. Supply noninteractive elements that fill the row.
    pub fn feedback<E: IntoElement>(
        mut self,
        render: impl FnOnce(SwipeProgress, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.feedback = Some(Box::new(move |progress, window, cx| {
            render(progress, window, cx).into_any_element()
        }));
        self
    }
}

impl Styled for SwipeActions {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SwipeActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let now = cx.background_executor().now();
        self.state.update(cx, |state, cx| {
            state.configure(self.enabled[0], self.enabled[1], self.threshold, cx);
            state.set_dismissal(self.dismissed, now);
        });
        let (offset, animating) = self.state.read(cx).visual(now);
        let collapsed = self.state.read(cx).collapsed(now);
        let progress = self.state.read(cx).feedback(offset);
        if animating {
            window.request_animation_frame();
        }
        let feedback = if offset != px(0.) {
            self.feedback.map(|render| {
                div()
                    .absolute()
                    .inset_0()
                    .child(render(progress, window, cx))
            })
        } else {
            None
        };
        let weak = self.state.downgrade();
        let measure = weak.clone();
        let interactive = self.dismissed.is_none();
        let listener = canvas(
            move |bounds, window, cx| {
                let _ = measure.update(cx, |state, _| state.measure(bounds.size.width));
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |_, hitbox, window, _| {
                if interactive {
                    input::register(weak, hitbox, window);
                }
            },
        )
        .absolute()
        .inset_0();
        let mut root = div().id(self.id.clone()).relative().w_full();
        root.style().refine(&self.style);
        let root = root
            .overflow_hidden()
            .child(listener)
            .children(feedback)
            .child(div().relative().left(offset).w_full().child(self.content))
            .when(!interactive, |root| {
                root.child(
                    canvas(
                        |bounds, window, _| {
                            window.insert_hitbox(bounds, HitboxBehavior::BlockMouse);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
            });
        animated_collapse(self.id, !collapsed, move || root)
            .duration(std::time::Duration::from_millis(200))
    }
}

#[cfg(test)]
mod tests;
