use gpui::{
    Bounds, Context, Pixels, Point, Size, Subscription, TouchEvent, TouchId, TouchPhase, Window,
    point, px, size,
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pointer {
    Mouse,
    Touch(TouchId),
}

struct Drag {
    pointer: Pointer,
    start: Point<Pixels>,
    pan: Point<Pixels>,
    claimed: bool,
}
struct Pinch {
    center: Point<Pixels>,
    distance: f32,
    zoom: f32,
    pan: Point<Pixels>,
}

/// Retained zoom and pan for one preview. Zoom 1 fits the entire content in the viewport.
pub struct ZoomState {
    source: Size<Pixels>,
    pub(super) bounds: Bounds<Pixels>,
    zoom: f32,
    pan: Point<Pixels>,
    max_zoom: f32,
    pub(super) supported: bool,
    drag: Option<Drag>,
    contacts: Vec<(TouchId, Point<Pixels>)>,
    pinch: Option<Pinch>,
    blocked: bool,
    _observations: [Subscription; 2],
}

impl ZoomState {
    pub fn new(content_size: Size<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        validate_size(content_size);
        Self {
            source: content_size,
            bounds: Bounds::default(),
            zoom: 1.,
            pan: point(px(0.), px(0.)),
            max_zoom: 8.,
            supported: window.supports_subtree_effects(),
            drag: None,
            contacts: vec![],
            pinch: None,
            blocked: false,
            _observations: [
                window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
                cx.observe_window_activation(window, |state, window, _| {
                    if !window.is_window_active() {
                        state.cancel();
                    }
                }),
            ],
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }
    pub fn offset(&self) -> Point<Pixels> {
        self.pan
    }
    pub fn content_size(&self) -> Size<Pixels> {
        self.source
    }

    /// Replaces the content aspect ratio and returns to fit. Dimensions must be finite and positive.
    pub fn set_content_size(&mut self, content_size: Size<Pixels>, cx: &mut Context<Self>) {
        validate_size(content_size);
        if self.source != content_size {
            self.source = content_size;
            self.reset(cx);
        }
    }

    /// Sets the largest multiple of fit, default 8. Must be finite and at least 1.
    pub fn set_max_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        assert!(
            zoom.is_finite() && zoom >= 1.,
            "maximum zoom must be finite and at least 1"
        );
        self.max_zoom = zoom;
        self.zoom_to(self.zoom, cx);
    }

    /// Zooms around the viewport center, clamped to 1..=max_zoom.
    pub fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.cancel();
        self.zoom_at(zoom, self.bounds.center(), cx);
    }

    /// Fits and centers the whole preview.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.cancel();
        self.zoom = 1.;
        self.pan = point(px(0.), px(0.));
        cx.notify();
    }

    pub(super) fn fitted(&self) -> Size<Pixels> {
        let scale = (self.bounds.size.width / self.source.width)
            .min(self.bounds.size.height / self.source.height);
        size(self.source.width * scale, self.source.height * scale)
    }

    fn clamp_pan(&mut self) {
        let fitted = self.fitted();
        let x = ((fitted.width * self.zoom - self.bounds.size.width) / 2.).max(px(0.));
        let y = ((fitted.height * self.zoom - self.bounds.size.height) / 2.).max(px(0.));
        self.pan.x = self.pan.x.clamp(-x, x);
        self.pan.y = self.pan.y.clamp(-y, y);
    }

    pub(super) fn measure(
        &mut self,
        bounds: Bounds<Pixels>,
        supported: bool,
        cx: &mut Context<Self>,
    ) {
        self.supported = supported;
        if self.bounds.size != bounds.size {
            self.cancel();
            self.bounds = bounds;
            self.clamp_pan();
            cx.notify();
        } else {
            self.bounds = bounds;
        }
        if !supported && self.zoom != 1. {
            self.reset(cx);
        }
    }

    pub(super) fn zoom_at(&mut self, zoom: f32, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.supported || !zoom.is_finite() || !finite(position) {
            return;
        }
        let zoom = zoom.clamp(1., self.max_zoom);
        let relative = position - self.bounds.center();
        self.pan = relative - (relative - self.pan) * (zoom / self.zoom);
        self.zoom = zoom;
        self.clamp_pan();
        cx.notify();
    }

    pub(super) fn claimed(&self) -> bool {
        self.pinch.is_some() || self.drag.as_ref().is_some_and(|drag| drag.claimed) || self.blocked
    }

    pub(super) fn cancel(&mut self) {
        self.drag = None;
        self.pinch = None;
        self.contacts.clear();
        self.blocked = false;
    }

    pub(super) fn begin(&mut self, pointer: Pointer, position: Point<Pixels>) {
        if self.supported && self.zoom > 1. && self.drag.is_none() && finite(position) {
            self.drag = Some(Drag {
                pointer,
                start: position,
                pan: self.pan,
                claimed: false,
            });
        }
    }

    pub(super) fn move_to(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !finite(position) {
            return false;
        }
        let Some(drag) = self.drag.as_mut().filter(|drag| drag.pointer == pointer) else {
            return false;
        };
        let delta = position - drag.start;
        if !drag.claimed && delta.x.abs().max(delta.y.abs()) <= px(8.) {
            return false;
        }
        drag.claimed = true;
        self.pan = drag.pan + delta;
        self.clamp_pan();
        cx.notify();
        true
    }

    pub(super) fn end(&mut self, pointer: Pointer) -> bool {
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.pointer == pointer)
        {
            return self.drag.take().unwrap().claimed;
        }
        false
    }

    pub(super) fn touch(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        prevented: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.supported || !finite(event.position) {
            return false;
        }
        let pointer = Pointer::Touch(event.id);
        match event.phase {
            TouchPhase::Started => {
                if !inside || self.contacts.iter().any(|(id, _)| *id == event.id) {
                    return false;
                }
                if self.contacts.is_empty() && prevented {
                    return false;
                }
                self.contacts.push((event.id, event.position));
                if self.contacts.len() == 1 {
                    self.begin(pointer, event.position);
                    return false;
                }
                self.drag = None;
                if self.contacts.len() == 2 && !self.blocked {
                    let a = self.contacts[0].1;
                    let b = self.contacts[1].1;
                    self.pinch = Some(Pinch {
                        center: (a + b) / 2.,
                        distance: distance(a, b).max(1.),
                        zoom: self.zoom,
                        pan: self.pan,
                    });
                } else {
                    self.pinch = None;
                    self.blocked = true;
                }
                true
            }
            TouchPhase::Moved => {
                let Some(contact) = self.contacts.iter_mut().find(|(id, _)| *id == event.id) else {
                    return false;
                };
                contact.1 = event.position;
                if self.blocked {
                    return true;
                }
                if let Some(pinch) = &self.pinch {
                    let a = self.contacts[0].1;
                    let b = self.contacts[1].1;
                    let zoom =
                        (pinch.zoom * distance(a, b) / pinch.distance).clamp(1., self.max_zoom);
                    let center = self.bounds.center();
                    self.pan = (a + b) / 2.
                        - center
                        - (pinch.center - center - pinch.pan) * (zoom / pinch.zoom);
                    self.zoom = zoom;
                    self.clamp_pan();
                    cx.notify();
                    true
                } else if prevented {
                    self.drag = None;
                    false
                } else {
                    self.move_to(pointer, event.position, cx)
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if !self.contacts.iter().any(|(id, _)| *id == event.id) {
                    return false;
                }
                let consumed = self.claimed();
                self.contacts.retain(|(id, _)| *id != event.id);
                self.end(pointer);
                if self.pinch.take().is_some() || event.phase == TouchPhase::Cancelled {
                    self.blocked = !self.contacts.is_empty();
                    self.drag = None;
                }
                if self.contacts.is_empty() {
                    self.blocked = false;
                }
                consumed
            }
        }
    }
}

fn finite(point: Point<Pixels>) -> bool {
    f32::from(point.x).is_finite() && f32::from(point.y).is_finite()
}
fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    f32::from(a.x - b.x).hypot(f32::from(a.y - b.y))
}
fn validate_size(size: Size<Pixels>) {
    assert!(
        f32::from(size.width).is_finite()
            && f32::from(size.height).is_finite()
            && size.width > px(0.)
            && size.height > px(0.),
        "content dimensions must be finite and positive"
    );
}
