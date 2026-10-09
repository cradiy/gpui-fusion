mod input;
mod state;

use gpui::{
    AnyElement, App, ElementId, Entity, HitboxBehavior, IntoElement, Refineable, RenderOnce, Style,
    StyleRefinement, Styled, TransformationMatrix, Window, canvas, div, point, prelude::*, size,
};
use gpui_effects::transform_group;
pub use state::ZoomState;

/// A bounded, fit-first preview with pointer-centered zoom and constrained panning.
/// Content fills a fitted rectangle with the aspect ratio supplied to `ZoomState`.
#[derive(IntoElement)]
pub struct ZoomView {
    id: ElementId,
    state: Entity<ZoomState>,
    content: AnyElement,
    wheel_zoom: bool,
    style: StyleRefinement,
}

impl ZoomView {
    pub fn new(
        id: impl Into<ElementId>,
        state: &Entity<ZoomState>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            content: content.into_any_element(),
            wheel_zoom: true,
            style: StyleRefinement::default(),
        }
    }

    /// Disables wheel zoom when false, leaving wheel scrolling to surrounding content.
    pub fn wheel_zoom(mut self, enabled: bool) -> Self {
        self.wheel_zoom = enabled;
        self
    }
}
impl Styled for ZoomView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ZoomView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let zoom = state.zoom();
        let pan = state.offset();
        let viewport = state.bounds.size;
        let fitted = state.fitted();
        let x = (viewport.width - fitted.width) / 2.;
        let y = (viewport.height - fitted.height) / 2.;
        let matrix = TransformationMatrix::unit()
            .translate(
                point(
                    viewport.width * (1. - zoom) / 2. + pan.x,
                    viewport.height * (1. - zoom) / 2. + pan.y,
                )
                .scale(1.),
            )
            .scale(size(zoom, zoom));
        let weak = self.state.downgrade();
        let measure = weak.clone();
        let toggle = self.state.clone();
        let listener = canvas(
            move |bounds, window, cx| {
                let _ = measure.update(cx, |state, cx| {
                    state.measure(bounds, window.supports_subtree_effects(), cx)
                });
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |_, hitbox, window, _| input::register(weak, hitbox, self.wheel_zoom, window),
        )
        .absolute()
        .inset_0();
        let mut resolved = Style::default();
        resolved.refine(&self.style);
        let corners = resolved.corner_radii.to_pixels(window.rem_size());
        let content = transform_group(
            div().relative().size_full().child(
                div()
                    .absolute()
                    .left(x)
                    .top(y)
                    .w(fitted.width)
                    .h(fitted.height)
                    .child(self.content),
            ),
            matrix,
        )
        .auto_raster_scale((self.id.clone(), "raster"))
        .clip_corners(corners);
        let mut root = div().id(self.id).relative().size_full();
        root.style().refine(&self.style);
        root.overflow_hidden().child(
            div()
                .id("viewport")
                .relative()
                .size_full()
                .overflow_hidden()
                .on_click(move |event, window, cx| {
                    if event.click_count() == 2 {
                        toggle.update(cx, |state, cx| {
                            let zoom = if state.zoom() > 1.01 { 1. } else { 2.5 };
                            state.zoom_at(zoom, event.position(), cx);
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
                .child(listener)
                .child(content),
        )
    }
}

#[cfg(test)]
mod tests;
