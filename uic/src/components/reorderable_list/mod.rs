mod input;
mod state;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, MouseButton, Pixels, Refineable, RenderOnce, StyleRefinement, Styled,
    Window, div, point, prelude::*, px,
};
use state::Pointer;
pub use state::{ReorderEvent, ReorderState};
use std::{cell::Cell, collections::HashSet, rc::Rc};

type ReorderCallback = Rc<dyn Fn(&ReorderEvent, &mut Window, &mut App)>;

/// A scrollable vertical list with stable item IDs, long-press sorting and drag handles.
/// Apply reorder proposals to application data in `on_reorder` to accept them.
pub struct ReorderableList {
    id: ElementId,
    state: Entity<ReorderState>,
    items: Vec<(ElementId, AnyElement)>,
    on_reorder: Option<ReorderCallback>,
    enabled: bool,
    style: StyleRefinement,
}
impl ReorderableList {
    pub fn new(id: impl Into<ElementId>, state: &Entity<ReorderState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            items: vec![],
            on_reorder: None,
            enabled: true,
            style: StyleRefinement::default(),
        }
    }
    /// Adds a row with a stable, unique ID. Content may include a SwipeActions component.
    pub fn item(mut self, id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        self.items.push((id.into(), content.into_any_element()));
        self
    }
    pub fn on_reorder(
        mut self,
        callback: impl Fn(&ReorderEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(callback));
        self
    }
    /// Disables sorting while retaining ordinary row interaction and scrolling.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
impl Styled for ReorderableList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl IntoElement for ReorderableList {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

#[doc(hidden)]
pub struct ListLayout {
    root: AnyElement,
    rows: Vec<Rc<Cell<Option<LayoutId>>>>,
    offsets: Vec<Rc<Cell<Pixels>>>,
}
impl Element for ReorderableList {
    type RequestLayoutState = ListLayout;
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
    ) -> (LayoutId, ListLayout) {
        let keys: Vec<_> = self.items.iter().map(|(id, _)| id.clone()).collect();
        assert_eq!(
            keys.iter().collect::<HashSet<_>>().len(),
            keys.len(),
            "reorderable list requires unique item IDs"
        );
        self.state.update(cx, |state, cx| {
            state.configure(keys, self.enabled && self.on_reorder.is_some(), cx)
        });
        let scroll = self.state.read(cx).scroll_handle();
        let mut root = div().id("viewport").relative().w_full().min_h_0();
        root.style().refine(&self.style);
        let mut root = root
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&scroll);
        let mut rows = vec![];
        let mut offsets = vec![];
        for (id, content) in self.items.drain(..) {
            let layout = Rc::new(Cell::new(None));
            let offset = Rc::new(Cell::new(px(0.)));
            rows.push(layout.clone());
            offsets.push(offset.clone());
            let state = self.state.clone();
            let key = id.clone();
            let child = div()
                .id("row")
                .w_full()
                .flex_shrink_0()
                .on_long_press(move |event, window, cx| {
                    if window.default_prevented() {
                        return;
                    }
                    let started = state.update(cx, |state, cx| {
                        let Some((contact, _)) = state.candidate else {
                            return false;
                        };
                        state.begin(&key, Pointer::Touch(contact), event.position, cx)
                    });
                    if started {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
                .child(content)
                .into_any_element();
            root = root.child(Row {
                id,
                child: Some(child),
                layout,
                offset,
                state: self.state.clone(),
                deferred: false,
            });
        }
        let mut root = root.into_any_element();
        let layout = root.request_layout(window, cx);
        (
            layout,
            ListLayout {
                root,
                rows,
                offsets,
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut ListLayout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let geometry = layout
            .rows
            .iter()
            .map(|row| {
                let rect = window.layout_bounds(row.get().unwrap());
                Bounds {
                    origin: rect.origin - bounds.origin,
                    size: rect.size,
                }
            })
            .collect();
        let now = cx.background_executor().now();
        let (offsets, moving) = self.state.update(cx, |state, _| {
            state.layout(bounds, geometry, now, !window.prefers_reduced_motion())
        });
        for (cell, offset) in layout.offsets.iter().zip(offsets) {
            cell.set(offset);
        }
        if moving {
            window.request_animation_frame();
        }
        layout.root.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut ListLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        input::register(self.state.downgrade(), self.on_reorder.clone(), window);
        layout.root.paint(window, cx);
    }
}

struct Row {
    id: ElementId,
    child: Option<AnyElement>,
    layout: Rc<Cell<Option<LayoutId>>>,
    offset: Rc<Cell<Pixels>>,
    state: Entity<ReorderState>,
    deferred: bool,
}
impl IntoElement for Row {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Row {
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
        let id = self.child.as_mut().unwrap().request_layout(window, cx);
        self.layout.set(Some(id));
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
        window.with_element_offset(point(px(0.), self.offset.get()), |window| {
            self.deferred = self.state.read(cx).is_lifted(&self.id);
            if self.deferred {
                window.defer_draw(
                    self.child.take().unwrap(),
                    window.element_offset(),
                    0,
                    Some(window.content_mask()),
                );
            } else {
                self.child.as_mut().unwrap().prepaint(window, cx);
            }
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
        if !self.deferred {
            self.child.as_mut().unwrap().paint(window, cx);
        }
    }
}

/// A mouse drag target for an item. Touch continues to use long press so vertical scrolling remains available.
#[derive(IntoElement)]
pub struct ReorderHandle {
    state: Entity<ReorderState>,
    id: ElementId,
    content: AnyElement,
    style: StyleRefinement,
}
impl ReorderHandle {
    pub fn new(
        state: &Entity<ReorderState>,
        id: impl Into<ElementId>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            state: state.clone(),
            id: id.into(),
            content: content.into_any_element(),
            style: StyleRefinement::default(),
        }
    }
}
impl Styled for ReorderHandle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for ReorderHandle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut root = div().cursor_grab();
        root.style().refine(&self.style);
        root.on_mouse_down(MouseButton::Left, move |event, window, cx| {
            if window.default_prevented() {
                return;
            }
            if self.state.update(cx, |state, cx| {
                state.begin(&self.id, Pointer::Mouse, event.position, cx)
            }) {
                window.prevent_default();
                cx.stop_propagation();
            }
        })
        .child(self.content)
    }
}

#[cfg(test)]
mod tests;
