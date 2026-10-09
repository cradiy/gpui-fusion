use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Bounds, DispatchPhase, Element, ElementId, FocusHandle, GlobalElementId, Hitbox,
    HitboxBehavior, HitboxId, InspectorElementId, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Refineable as _, Style, StyleRefinement, Styled,
    TouchId, TouchPhase, Window,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InteractionPhase {
    Hover,
    Start,
    Preview,
    Commit,
    Cancel,
}

type Callback = Rc<dyn Fn(f32, InteractionPhase, &mut Window, &mut App)>;

#[derive(Clone, Default)]
pub(super) struct CaptureToken(Rc<Cell<Option<HitboxId>>>, Rc<Cell<Option<TouchDrag>>>);

#[derive(Clone, Copy)]
struct TouchDrag {
    id: TouchId,
    origin: Point<Pixels>,
    claimed: bool,
}

impl CaptureToken {
    pub(super) fn clear_touch(&self) {
        self.1.set(None);
    }
}

pub(super) struct SliderInteraction {
    capture_token: CaptureToken,
    focus_handle: FocusHandle,
    callback: Callback,
    edge_inset: Pixels,
    style: StyleRefinement,
}

impl SliderInteraction {
    pub(super) fn new(
        capture_token: CaptureToken,
        focus_handle: FocusHandle,
        callback: impl Fn(f32, InteractionPhase, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            capture_token,
            focus_handle,
            callback: Rc::new(callback),
            edge_inset: Pixels::ZERO,
            style: StyleRefinement::default(),
        }
    }

    pub(super) fn edge_inset(mut self, edge_inset: Pixels) -> Self {
        self.edge_inset = edge_inset.max(Pixels::ZERO);
        self
    }
}

impl IntoElement for SliderInteraction {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SliderInteraction {
    type RequestLayoutState = Style;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        if let Some(previous_hitbox) = self.capture_token.0.get()
            && window.captured_hitbox() == Some(previous_hitbox)
        {
            window.capture_pointer(hitbox.id);
        }
        self.capture_token.0.set(Some(hitbox.id));
        hitbox
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let down_hitbox = hitbox.clone();
        let down_focus = self.focus_handle.clone();
        let down_callback = self.callback.clone();
        let down_inset = self.edge_inset;
        let move_hitbox = hitbox.clone();
        let move_callback = self.callback.clone();
        let move_inset = self.edge_inset;
        let up_hitbox = hitbox.clone();
        let up_callback = self.callback.clone();
        let up_inset = self.edge_inset;
        let touch = self.capture_token.1.clone();
        let touch_hitbox = hitbox.clone();
        let touch_callback = self.callback.clone();
        let touch_focus = self.focus_handle.clone();
        let touch_inset = self.edge_inset;

        style.paint(bounds, window, cx, move |window, _| {
            window.on_touch_event(move |event, phase, window, cx| {
                let current = touch.get();
                if event.phase == TouchPhase::Started {
                    if phase.bubble()
                        && current.is_none()
                        && !window.default_prevented()
                        && touch_hitbox.is_hovered(window)
                        && touch_hitbox.bounds.contains(&event.position)
                    {
                        touch.set(Some(TouchDrag {
                            id: event.id,
                            origin: event.position,
                            claimed: false,
                        }));
                    }
                    return;
                }
                if !phase.capture() {
                    return;
                }
                let Some(mut drag) = current.filter(|drag| drag.id == event.id) else {
                    return;
                };
                let ratio = horizontal_ratio(event.position.x, touch_hitbox.bounds, touch_inset);
                match event.phase {
                    TouchPhase::Moved => {
                        if !drag.claimed {
                            let delta = event.position - drag.origin;
                            let dx = f32::from(delta.x).abs();
                            let dy = f32::from(delta.y).abs();
                            if dx.max(dy) < 6. {
                                return;
                            }
                            if dy >= dx || window.default_prevented() {
                                touch.set(None);
                                return;
                            }
                            drag.claimed = true;
                            touch.set(Some(drag));
                            touch_focus.focus(window, cx);
                            touch_callback(
                                horizontal_ratio(drag.origin.x, touch_hitbox.bounds, touch_inset),
                                InteractionPhase::Start,
                                window,
                                cx,
                            );
                        }
                        touch_callback(ratio, InteractionPhase::Preview, window, cx);
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        touch.set(None);
                        if !drag.claimed {
                            return;
                        }
                        touch_callback(
                            ratio,
                            if event.phase == TouchPhase::Ended {
                                InteractionPhase::Commit
                            } else {
                                InteractionPhase::Cancel
                            },
                            window,
                            cx,
                        );
                    }
                    TouchPhase::Started => return,
                }
                window.prevent_default();
                cx.stop_propagation();
            });
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && down_hitbox.is_hovered(window)
                {
                    window.capture_pointer(down_hitbox.id);
                    down_focus.focus(window, cx);
                    down_callback(
                        horizontal_ratio(event.position.x, down_hitbox.bounds, down_inset),
                        InteractionPhase::Start,
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            });
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.dragging()
                    && window.captured_hitbox() == Some(move_hitbox.id)
                {
                    move_callback(
                        horizontal_ratio(event.position.x, move_hitbox.bounds, move_inset),
                        InteractionPhase::Preview,
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                } else if phase == DispatchPhase::Bubble
                    && !event.dragging()
                    && move_hitbox.is_hovered(window)
                {
                    move_callback(
                        horizontal_ratio(event.position.x, move_hitbox.bounds, move_inset),
                        InteractionPhase::Hover,
                        window,
                        cx,
                    );
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Left
                    && window.captured_hitbox() == Some(up_hitbox.id)
                {
                    up_callback(
                        horizontal_ratio(event.position.x, up_hitbox.bounds, up_inset),
                        InteractionPhase::Commit,
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            });
        });
    }
}

impl Styled for SliderInteraction {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn horizontal_ratio(x: Pixels, bounds: Bounds<Pixels>, inset: Pixels) -> f32 {
    let width = f32::from(bounds.size.width - inset * 2.).max(1.0);
    (f32::from(x - bounds.origin.x - inset) / width).clamp(0.0, 1.0)
}
