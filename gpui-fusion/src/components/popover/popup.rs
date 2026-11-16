use super::PopoverPlacement;
use gpui::{prelude::*, *};

pub(super) struct Popup {
    pub content: AnyElement,
    pub anchor: Bounds<Pixels>,
    pub placement: PopoverPlacement,
    pub margin: Pixels,
    pub progress: f32,
}
impl IntoElement for Popup {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Popup {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let viewport = window.viewport_size();
        let (bounds, direction) = self.placement.resolve(
            self.anchor,
            window.layout_bounds(*layout).size,
            viewport,
            self.margin,
        );
        let origin = bounds.origin + direction * (10. * (1. - self.progress));
        self.content.prepaint_at(
            point(
                origin.x.clamp(
                    self.margin,
                    (viewport.width - self.margin - bounds.size.width).max(self.margin),
                ),
                origin.y.clamp(
                    self.margin,
                    (viewport.height - self.margin - bounds.size.height).max(self.margin),
                ),
            ),
            window,
            cx,
        );
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
