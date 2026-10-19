use super::{GlobalModal, ModalLayer, SheetMotion};
use gpui::{prelude::*, *};
use scheduler::Instant;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct ScrollRegion {
    bounds: Bounds<Pixels>,
    pub(super) scroll: ScrollHandle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GestureRoute {
    Continuous,
    Sheet,
    List,
}

pub(super) struct SheetTouch {
    id: TouchId,
    start: Point<Pixels>,
    last: Point<Pixels>,
    started: Instant,
    sampled: Instant,
    velocity: f32,
    claimed: bool,
    route: GestureRoute,
    last_consumer: GestureRoute,
    scroll: Option<ScrollHandle>,
    initial_offset: f32,
    initial_max: f32,
}

pub(super) struct ScrollMotion {
    scroll: ScrollHandle,
    velocity: f32,
    sampled: Instant,
}

/// Coordinates an existing scroll viewport with the surrounding expandable sheet.
/// The content retains its own ScrollHandle, row actions and long-press gestures.
#[derive(IntoElement)]
pub struct SheetScrollArea {
    scroll: ScrollHandle,
    content: AnyElement,
}

impl SheetScrollArea {
    pub fn new(scroll: ScrollHandle, content: impl IntoElement) -> Self {
        Self {
            scroll,
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for SheetScrollArea {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let layer = cx
            .try_global::<GlobalModal>()
            .map(|global| global.0.clone());
        let offset = layer.as_ref().map_or(0., |layer| {
            layer.read(cx).edge_visual(cx.background_executor().now())
        });
        let scroll = self.scroll;
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if let Some(layer) = &layer {
                            layer.update(cx, |layer, _| {
                                layer.sheet_scroll_region = Some(ScrollRegion {
                                    bounds: bounds.intersect(&window.content_mask().bounds),
                                    scroll: scroll.clone(),
                                });
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .relative()
                    .top(px(offset))
                    .size_full()
                    .child(self.content),
            )
    }
}

fn rubber_band(distance: f32) -> f32 {
    112. * distance / (112. + distance.abs())
}

/// Returns the distance left over after the viewport reaches either edge.
fn scroll_by(scroll: &ScrollHandle, delta: f32, maximum: f32) -> f32 {
    if delta.abs() < 0.001 {
        return 0.;
    }
    let mut offset = scroll.offset();
    let previous = offset.y.as_f32();
    let next = if previous < -maximum {
        (previous + delta.max(0.)).min(0.)
    } else {
        (previous + delta).clamp(-maximum, 0.)
    };
    offset.y = px(next);
    scroll.set_offset(offset);
    delta - (next - previous)
}

impl ModalLayer {
    pub(super) fn edge_visual(&self, now: Instant) -> f32 {
        self.edge_motion
            .as_ref()
            .map_or_else(|| rubber_band(self.edge_pull), |motion| motion.value(now).0)
    }

    fn release_edge(&mut self, now: Instant) {
        let from = self.edge_visual(now);
        self.edge_pull = 0.;
        self.edge_motion = (from.abs() > 0.1).then_some(SheetMotion {
            from,
            to: 0.,
            started: now,
            duration: Duration::from_millis(320),
        });
    }

    pub(super) fn release_scroll_hold(&mut self) {
        if let Some(scroll) = self.held_scroll.take() {
            scroll.preserve_vertical_offset(false);
        }
        self.scroll_settle = None;
        self.scroll_hold_rebound = true;
    }

    pub(super) fn hold_scroll(&mut self, scroll: &ScrollHandle) {
        scroll.preserve_vertical_offset(true);
        self.held_scroll = Some(scroll.clone());
        self.scroll_settle = None;
    }

    pub(super) fn tick_scroll(&mut self, now: Instant) -> bool {
        let resizing = self
            .resize_motion
            .as_ref()
            .is_some_and(|motion| motion.value(now).1);
        let touching = self.sheet_surface_touch.is_some() || self.sheet_drag.is_some();
        if !touching
            && !resizing
            && !self.closing
            && self.scroll_hold_rebound
            && let Some(scroll) = self.held_scroll.clone()
        {
            let offset = scroll.offset();
            let target = offset.y.clamp(-scroll.max_offset().y, px(0.)).as_f32();
            if self.scroll_settle.is_none() && (offset.y.as_f32() - target).abs() > 0.1 {
                self.scroll_settle = Some(SheetMotion {
                    from: offset.y.as_f32(),
                    to: target,
                    started: now,
                    duration: Duration::from_millis(260),
                });
            }
            if let Some(motion) = &self.scroll_settle {
                let (y, moving) = motion.value(now);
                scroll.set_offset(point(offset.x, px(y)));
                if !moving {
                    self.release_scroll_hold();
                }
            } else {
                self.release_scroll_hold();
            }
        }
        if resizing {
            return false;
        }
        if let Some(mut motion) = self.scroll_motion.take() {
            let dt = now
                .saturating_duration_since(motion.sampled)
                .as_secs_f32()
                .min(0.05);
            let delta = motion.velocity * (1. - (-5.5 * dt).exp()) / 5.5;
            let remaining = scroll_by(&motion.scroll, delta, motion.scroll.max_offset().y.as_f32());
            motion.velocity *= (-5.5 * dt).exp();
            motion.sampled = now;
            if remaining.abs() > 0.1 {
                self.edge_pull = remaining + motion.velocity * 0.025;
                self.release_edge(now);
            } else if motion.velocity.abs() > 12. {
                self.scroll_motion = Some(motion);
            }
        }
        self.scroll_motion.is_some()
            || self.scroll_settle.is_some()
            || self
                .edge_motion
                .as_ref()
                .is_some_and(|motion| motion.value(now).1)
    }

    fn drag_scroll(&mut self, gesture: &mut SheetTouch, mut delta: f32) {
        if gesture.route == GestureRoute::Sheet {
            let gap = (self.sheet_max_height - self.sheet_collapsed_height)
                .as_f32()
                .max(0.);
            let extension =
                (self.sheet_drag_expansion * gap - self.sheet_offset.as_f32() - delta).min(gap);
            self.sheet_offset = px(self.sheet_drag_expansion * gap - extension);
            gesture.last_consumer = GestureRoute::Sheet;
            return;
        }
        // Reverse an elastic pull before moving content back into its scroll range.
        if self.edge_pull != 0. {
            if delta.signum() == self.edge_pull.signum() {
                self.edge_pull += delta;
                return;
            }
            let used = delta.abs().min(self.edge_pull.abs()) * delta.signum();
            self.edge_pull += used;
            delta -= used;
        }
        let gap = (self.sheet_max_height - self.sheet_collapsed_height)
            .as_f32()
            .max(0.);
        let mut extension = self.sheet_drag_expansion * gap - self.sheet_offset.as_f32();
        if gesture.route != GestureRoute::List && delta < 0. {
            let consumed = (-delta).min((gap - extension).max(0.));
            if consumed > 0.
                && let Some(scroll) = &gesture.scroll
            {
                self.hold_scroll(scroll);
            }
            if consumed > 0. {
                gesture.last_consumer = GestureRoute::Sheet;
            }
            extension += consumed;
            delta += consumed;
        }
        if delta.abs() > 0.001
            && let Some(scroll) = &gesture.scroll
        {
            let desired_height = self.sheet_collapsed_height.as_f32() + extension.clamp(0., gap);
            let growth = if gesture.route == GestureRoute::List {
                0.
            } else {
                desired_height - self.sheet_height.as_f32()
            };
            let max = (scroll.max_offset().y.as_f32() - growth).max(0.);
            let remaining = scroll_by(scroll, delta, max);
            if (remaining - delta).abs() > 0.001 {
                gesture.last_consumer = GestureRoute::List;
            }
            delta = remaining;
        }
        if gesture.route != GestureRoute::List {
            if delta > 0. {
                extension -= delta;
                gesture.last_consumer = GestureRoute::Sheet;
                delta = 0.;
            }
            self.sheet_offset = px(self.sheet_drag_expansion * gap - extension);
        }
        if gesture.scroll.is_some() {
            self.edge_pull += delta;
        }
    }

    pub(super) fn surface_touch(
        &mut self,
        event: &TouchEvent,
        phase: DispatchPhase,
        inside: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closing {
            return;
        }
        let now = cx.background_executor().now();
        let owned = self
            .sheet_surface_touch
            .as_ref()
            .is_some_and(|gesture| gesture.id == event.id && gesture.claimed);
        if phase.capture() != owned {
            return;
        }
        if event.phase == TouchPhase::Started {
            if self.sheet_surface_touch.is_some() {
                // A second contact terminates the drag instead of changing its owner.
                self.sheet_surface_touch = None;
                self.finish_sheet_drag(true, window, cx);
                self.release_edge(now);
                return;
            }
            if inside
                && !window.default_prevented()
                && self.sheet_drag.is_none()
                && !self
                    .sheet_exclusions
                    .iter()
                    .any(|bounds| bounds.contains(&event.position))
            {
                let scroll = self
                    .sheet_scroll_region
                    .as_ref()
                    .filter(|region| region.bounds.contains(&event.position))
                    .map(|region| region.scroll.clone());
                let initial_offset = scroll
                    .as_ref()
                    .map_or(0., |scroll| scroll.offset().y.as_f32());
                let initial_max = scroll
                    .as_ref()
                    .map_or(0., |scroll| scroll.max_offset().y.as_f32());
                self.scroll_motion = None;
                self.scroll_settle = None;
                self.scroll_hold_rebound = true;
                self.edge_motion = None;
                self.edge_pull = 0.;
                self.sheet_surface_touch = Some(SheetTouch {
                    id: event.id,
                    start: event.position,
                    last: event.position,
                    started: now,
                    sampled: now,
                    velocity: 0.,
                    claimed: false,
                    route: GestureRoute::Continuous,
                    last_consumer: GestureRoute::Sheet,
                    scroll,
                    initial_offset,
                    initial_max,
                });
                cx.notify();
            }
            return;
        }
        let Some(mut gesture) = self.sheet_surface_touch.take() else {
            return;
        };
        if gesture.id != event.id {
            self.sheet_surface_touch = Some(gesture);
            return;
        }
        let total = event.position - gesture.start;
        match event.phase {
            TouchPhase::Moved => {
                if !owned
                    && (window.default_prevented()
                        || (total.x.abs() > px(8.) && total.x.abs() >= total.y.abs()))
                {
                    return;
                }
                if !owned && total.y.abs() <= px(8.) {
                    self.sheet_surface_touch = Some(gesture);
                    return;
                }
                let elapsed = now
                    .saturating_duration_since(gesture.started)
                    .as_secs_f32()
                    .max(0.001);
                if !owned {
                    let available = if total.y < px(0.) {
                        gesture.initial_max + gesture.initial_offset > 0.5
                    } else {
                        gesture.initial_offset < -0.5
                    };
                    gesture.route = if elapsed < 0.18 && total.y.as_f32().abs() / elapsed > 600. {
                        if available {
                            GestureRoute::List
                        } else {
                            GestureRoute::Sheet
                        }
                    } else {
                        GestureRoute::Continuous
                    };
                    gesture.claimed = true;
                    if gesture.route != GestureRoute::List {
                        self.begin_sheet_drag(Some(event.id), gesture.start.y, cx);
                    }
                }
                let dt = now
                    .saturating_duration_since(gesture.sampled)
                    .as_secs_f32()
                    .max(0.001);
                let delta = (event.position.y - gesture.last.y).as_f32();
                let velocity = (delta / dt).clamp(-6500., 6500.);
                gesture.velocity = if !owned || velocity.signum() != gesture.velocity.signum() {
                    velocity
                } else {
                    gesture.velocity * 0.35 + velocity * 0.65
                };
                if gesture.route == GestureRoute::Continuous
                    && elapsed < 0.18
                    && velocity.abs() > 600.
                {
                    gesture.route = gesture.last_consumer;
                    if gesture.route == GestureRoute::List {
                        self.sheet_drag = None;
                        self.resize_motion = None;
                        self.sheet_expanded = self.sheet_drag_expansion >= 0.5;
                        self.sheet_offset = px(0.);
                    }
                }
                gesture.last = event.position;
                gesture.sampled = now;
                self.drag_scroll(&mut gesture, delta);
                self.sheet_surface_touch = Some(gesture);
                window.prevent_default();
                cx.notify();
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if owned {
                    let cancelled = event.phase == TouchPhase::Cancelled;
                    let final_delta = (event.position.y - gesture.last.y).as_f32();
                    if !cancelled && final_delta.abs() > 0.5 {
                        let dt = now
                            .saturating_duration_since(gesture.sampled)
                            .as_secs_f32()
                            .max(0.001);
                        gesture.velocity = (final_delta / dt).clamp(-6500., 6500.);
                        gesture.sampled = now;
                        self.drag_scroll(&mut gesture, final_delta);
                    }
                    if gesture.route == GestureRoute::Sheet {
                        self.scroll_hold_rebound = false;
                    }
                    if gesture.route != GestureRoute::List {
                        self.sheet_sample = Some((self.sheet_offset, gesture.sampled));
                        self.sheet_velocity = if now.saturating_duration_since(gesture.sampled)
                            < Duration::from_millis(100)
                        {
                            gesture.velocity
                        } else {
                            0.
                        };
                        self.finish_sheet_drag(cancelled, window, cx);
                    }
                    if !cancelled
                        && !self.closing
                        && self.edge_pull.abs() < 0.5
                        && now.saturating_duration_since(gesture.sampled)
                            < Duration::from_millis(100)
                        && gesture.velocity.abs() > 120.
                        && let Some(scroll) = gesture.scroll
                    {
                        let available = if gesture.velocity < 0. {
                            scroll.max_offset().y + scroll.offset().y > px(0.5)
                        } else {
                            scroll.offset().y < px(-0.5)
                        };
                        let at_full_height =
                            (self.sheet_max_height - self.sheet_height).abs() < px(0.5);
                        if available
                            && gesture.last_consumer == GestureRoute::List
                            && (gesture.route == GestureRoute::List || at_full_height)
                        {
                            self.scroll_motion = Some(ScrollMotion {
                                scroll,
                                velocity: gesture.velocity,
                                sampled: now,
                            });
                        }
                    }
                    self.release_edge(now);
                    window.prevent_default();
                    cx.notify();
                }
            }
            _ => self.sheet_surface_touch = Some(gesture),
        }
    }
}
