use std::{cell::Cell, collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Stateful, StyleRefinement, Styled, Window, div, prelude::*, px,
};

use super::layout_transition::LayoutState;

/// A normal layout container whose keyed children animate to their measured positions.
/// Style the group for flex or grid layout and supply stable, unique item IDs.
pub fn animated_layout(id: impl Into<ElementId>) -> AnimatedLayout {
    AnimatedLayout {
        id: id.into(),
        container: Some(div().id("container")),
        items: Vec::new(),
        duration: Duration::from_millis(260),
        enabled: true,
    }
}

/// Animates item positions without scaling content or changing layout sizes.
/// New items appear immediately; removed items are not retained for exit animation.
pub struct AnimatedLayout {
    id: ElementId,
    container: Option<Stateful<Div>>,
    items: Vec<(ElementId, AnyElement)>,
    duration: Duration,
    enabled: bool,
}

impl AnimatedLayout {
    /// Adds an ordinary layout child. The key identifies its motion and element state.
    pub fn item(mut self, id: impl Into<ElementId>, child: impl IntoElement) -> Self {
        self.items.push((id.into(), child.into_any_element()));
        self
    }

    /// Time to reach a new position. Defaults to 260 ms with cubic ease-out.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Snap to measured positions when disabled, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Styled for AnimatedLayout {
    fn style(&mut self) -> &mut StyleRefinement {
        self.container
            .as_mut()
            .expect("cannot style after layout")
            .style()
    }
}

impl IntoElement for AnimatedLayout {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for AnimatedLayout {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
    ) -> (LayoutId, AnyElement) {
        let keys: HashSet<_> = self.items.iter().map(|(key, _)| key).collect();
        assert_eq!(
            keys.len(),
            self.items.len(),
            "animated layout requires unique item IDs"
        );
        let container_layout = Rc::new(Cell::new(None));
        let mut container = self
            .container
            .take()
            .expect("animated layout requested twice")
            .children(self.items.drain(..).map(|(id, child)| LayoutItem {
                id,
                child,
                container_layout: container_layout.clone(),
                duration: self.duration,
                enabled: self.enabled,
            }))
            .into_any_element();
        let layout_id = container.request_layout(window, cx);
        container_layout.set(Some(layout_id));
        (layout_id, container)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        container: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        container.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        container: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        container.paint(window, cx);
    }
}

struct LayoutItem {
    id: ElementId,
    child: AnyElement,
    container_layout: Rc<Cell<Option<LayoutId>>>,
    duration: Duration,
    enabled: bool,
}

impl IntoElement for LayoutItem {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for LayoutItem {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
    ) -> (LayoutId, ()) {
        // Return the child's node directly so flex/grid item styles keep their meaning.
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = window
            .layout_bounds(
                self.container_layout
                    .get()
                    .expect("container must have a layout"),
            )
            .origin;
        // Both bounds share the current element offset, excluding ancestor motion and scrolling.
        let target = Bounds {
            origin: bounds.origin - origin,
            size: Default::default(),
        };
        let now = cx.background_executor().now();
        let sample =
            window.with_element_state(id.unwrap(), |state: Option<Option<LayoutState>>, _| {
                if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
                    return (None, None);
                }
                let mut state = state
                    .flatten()
                    .unwrap_or_else(|| LayoutState::new(target, self.duration, now));
                let result = state.update(target, self.duration, self.enabled, now);
                (Some(result), Some(state))
            });
        if let Some((displayed, moving)) = sample {
            if moving {
                window.request_animation_frame();
            }
            let position = window.pixel_snap_point(origin + displayed.origin);
            window.with_element_offset(position - bounds.origin, |window| {
                self.child.prepaint(window, cx)
            });
        } else {
            self.child.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests;
