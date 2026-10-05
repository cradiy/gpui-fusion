use std::{cell::Cell, collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Position, Stateful, Style, StyleRefinement,
    Styled, Window, div, point, prelude::*, px, size,
};

use super::layout_transition::LayoutState;

/// A caller-styled group with a decoration that follows its selected item's bounds.
/// Size the decoration with `.size_full()` and use transparent item backgrounds.
pub fn selection_indicator(
    id: impl Into<ElementId>,
    indicator: impl IntoElement,
) -> SelectionIndicator {
    SelectionIndicator {
        id: id.into(),
        selected: None,
        indicator: Some(indicator.into_any_element()),
        items: Vec::new(),
        container: Some(div().id("container")),
        duration: Duration::from_millis(260),
        enabled: true,
        inset: px(0.),
        underline: None,
    }
}

/// Measures keyed items and animates a decoration behind them without moving their layout.
/// Selection, input handlers and keyboard navigation belong to the application.
pub struct SelectionIndicator {
    id: ElementId,
    selected: Option<ElementId>,
    indicator: Option<AnyElement>,
    items: Vec<(ElementId, AnyElement)>,
    container: Option<Stateful<Div>>,
    duration: Duration,
    enabled: bool,
    inset: Pixels,
    underline: Option<Pixels>,
}

impl SelectionIndicator {
    /// Selects an item by its stable key. Missing keys hide the decoration.
    pub fn selected(mut self, id: impl Into<ElementId>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Adds a normal layout child. Keys must be unique within the group.
    pub fn item(mut self, id: impl Into<ElementId>, item: impl IntoElement) -> Self {
        self.items.push((id.into(), item.into_any_element()));
        self
    }

    /// Time to reach new bounds. Zero snaps. Defaults to 260 ms with cubic ease-out.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Disable interpolation, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Insets the decoration on all sides of the item's border box.
    /// Values must be finite and nonnegative; each axis is capped at half its size.
    pub fn inset(mut self, inset: Pixels) -> Self {
        assert!(
            f32::from(inset).is_finite() && inset >= px(0.),
            "indicator inset must be finite and nonnegative"
        );
        self.inset = inset;
        self
    }

    /// Places a line at the bottom of the inset item bounds instead of filling them.
    /// Thickness must be finite and nonnegative and is capped at the available height.
    pub fn underline(mut self, thickness: Pixels) -> Self {
        assert!(
            f32::from(thickness).is_finite() && thickness >= px(0.),
            "indicator thickness must be finite and nonnegative"
        );
        self.underline = Some(thickness);
        self
    }
}

impl Styled for SelectionIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        self.container
            .as_mut()
            .expect("cannot style after layout")
            .style()
    }
}

impl IntoElement for SelectionIndicator {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for SelectionIndicator {
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
        let keys: HashSet<_> = self.items.iter().map(|(id, _)| id).collect();
        assert_eq!(
            keys.len(),
            self.items.len(),
            "selection indicator requires unique item IDs"
        );
        let selected_layout = Rc::new(Cell::new(None));
        let container_layout = Rc::new(Cell::new(None));
        let layer = IndicatorLayer {
            selected_layout: selected_layout.clone(),
            container_layout: container_layout.clone(),
            indicator: self.indicator.take(),
            duration: self.duration,
            enabled: self.enabled,
            inset: self.inset,
            underline: self.underline,
        };
        let mut container = self
            .container
            .take()
            .expect("selection indicator layout requested twice")
            .child(layer)
            .children(self.items.drain(..).map(|(id, child)| {
                let layout = (self.selected.as_ref() == Some(&id)).then(|| selected_layout.clone());
                IndicatorItem { id, child, layout }
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

struct IndicatorItem {
    id: ElementId,
    child: AnyElement,
    layout: Option<Rc<Cell<Option<LayoutId>>>>,
}

impl IntoElement for IndicatorItem {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for IndicatorItem {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        let id = window.with_element_namespace("items", |window| {
            window.with_element_namespace(self.id.clone(), |window| {
                self.child.request_layout(window, cx)
            })
        });
        if let Some(layout) = &self.layout {
            layout.set(Some(id));
        }
        (id, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_namespace("items", |window| {
            window.with_element_namespace(self.id.clone(), |window| self.child.prepaint(window, cx))
        });
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
        window.with_element_namespace("items", |window| {
            window.with_element_namespace(self.id.clone(), |window| self.child.paint(window, cx))
        });
    }
}

struct IndicatorLayer {
    selected_layout: Rc<Cell<Option<LayoutId>>>,
    container_layout: Rc<Cell<Option<LayoutId>>>,
    indicator: Option<AnyElement>,
    duration: Duration,
    enabled: bool,
    inset: Pixels,
    underline: Option<Pixels>,
}

impl IntoElement for IndicatorLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for IndicatorLayer {
    type RequestLayoutState = ();
    type PrepaintState = Option<AnyElement>;
    fn id(&self) -> Option<ElementId> {
        Some("indicator".into())
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
        // This layer never contributes an item or gap to the group's flex/grid flow.
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    size: size(px(0.).into(), px(0.).into()),
                    ..Default::default()
                },
                [],
                cx,
            ),
            (),
        )
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let origin = window
            .layout_bounds(
                self.container_layout
                    .get()
                    .expect("container must have a layout"),
            )
            .origin;
        let target = self.selected_layout.get().and_then(|id| {
            let mut target = window.layout_bounds(id);
            if target.size.width <= px(0.) || target.size.height <= px(0.) {
                return None;
            }
            target.origin -= origin;
            let inset = point(
                self.inset.min(target.size.width / 2.),
                self.inset.min(target.size.height / 2.),
            );
            target.origin += inset;
            target.size.width -= inset.x * 2.;
            target.size.height -= inset.y * 2.;
            if let Some(thickness) = self.underline {
                let height = thickness.min(target.size.height);
                target.origin.y += target.size.height - height;
                target.size.height = height;
            }
            Some(target)
        });
        let now = cx.background_executor().now();
        let sampled =
            window.with_element_state(id.unwrap(), |state: Option<Option<LayoutState>>, _| {
                let Some(target) = target else {
                    return (None, None);
                };
                let mut state = state
                    .flatten()
                    .unwrap_or_else(|| LayoutState::new(target, self.duration, now));
                let sample = state.update(target, self.duration, self.enabled, now);
                (Some(sample), Some(state))
            });
        let (displayed, moving) = sampled?;
        if moving {
            window.request_animation_frame();
        }
        let mut indicator = div()
            .w(displayed.size.width)
            .h(displayed.size.height)
            .relative()
            .child(self.indicator.take().expect("indicator prepainted twice"))
            .into_any_element();
        indicator.layout_as_root(displayed.size.map(AvailableSpace::Definite), window, cx);
        indicator.prepaint_at(origin + displayed.origin, window, cx);
        Some(indicator)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        indicator: &mut Option<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(indicator) = indicator {
            indicator.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests;
