use gpui::{
    Context, EventEmitter, Pixels, Point, Subscription, TouchEvent, TouchId, TouchPhase, Window,
};
use scheduler::Instant;
use std::{collections::VecDeque, time::Duration};

const SLOP: f32 = 8.;
const SETTLE: Duration = Duration::from_millis(260);
const VELOCITY_WINDOW: Duration = Duration::from_millis(120);

/// Emitted when the selected destination changes, including programmatic changes.
/// Animation may still be in progress. `None` means there are no pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageChanged {
    pub page: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pointer {
    Mouse,
    Touch(TouchId),
}

struct Drag {
    pointer: Pointer,
    start: Point<Pixels>,
    base: f32,
    anchor: usize,
    position: f32,
    claimed: bool,
    rejected: bool,
    samples: VecDeque<(Instant, f32)>,
}

/// Retained state shared by a pager and its navigation controls.
/// Keep page entities in the application if their state must survive leaving the viewport.
pub struct PagerState {
    count: usize,
    selected: Option<usize>,
    width: f32,
    enabled: bool,
    animate: bool,
    drag: Option<Drag>,
    motion: Option<(Instant, f32)>,
    contacts: Vec<TouchId>,
    _observations: [Subscription; 2],
}

impl EventEmitter<PageChanged> for PagerState {}

impl PagerState {
    pub fn new(page_count: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            count: page_count,
            selected: (page_count > 0).then_some(0),
            width: 0.,
            enabled: true,
            animate: !window.prefers_reduced_motion(),
            drag: None,
            motion: None,
            contacts: Vec::new(),
            _observations: [
                window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
                cx.observe_window_activation(window, |state, window, cx| {
                    if !window.is_window_active() {
                        state.cancel(cx);
                        state.contacts.clear();
                    }
                }),
            ],
        }
    }

    pub fn page_count(&self) -> usize {
        self.count
    }

    /// Selected destination, or `None` for an empty pager.
    pub fn current_page(&self) -> Option<usize> {
        self.selected
    }

    /// Updates the number of pages, cancelling motion and clamping the selection.
    pub fn set_page_count(&mut self, count: usize, cx: &mut Context<Self>) {
        if self.count == count {
            return;
        }
        self.count = count;
        self.drag = None;
        self.motion = None;
        self.select(
            (count > 0).then(|| self.selected.unwrap_or(0).min(count - 1)),
            cx,
        );
        cx.notify();
    }

    /// Animates to a zero-based page. Returns false for an out-of-range index.
    pub fn scroll_to(&mut self, page: usize, cx: &mut Context<Self>) -> bool {
        if page >= self.count {
            return false;
        }
        let now = cx.background_executor().now();
        let from = self.visual(now).0;
        self.drag = None;
        self.settle(page, from, now, cx);
        true
    }

    /// Selects a page immediately. Returns false for an out-of-range index.
    pub fn jump_to(&mut self, page: usize, cx: &mut Context<Self>) -> bool {
        if page >= self.count {
            return false;
        }
        self.drag = None;
        self.motion = None;
        self.select(Some(page), cx);
        cx.notify();
        true
    }

    fn select(&mut self, page: Option<usize>, cx: &mut Context<Self>) {
        if self.selected != page {
            self.selected = page;
            cx.emit(PageChanged { page });
        }
    }

    fn settle(&mut self, page: usize, from: f32, now: Instant, cx: &mut Context<Self>) {
        self.select(Some(page), cx);
        self.motion = (self.animate && (from - page as f32).abs() > 0.0001).then_some((now, from));
        cx.notify();
    }

    pub(super) fn visual(&self, now: Instant) -> (f32, bool) {
        if let Some(drag) = &self.drag
            && drag.claimed
        {
            return (drag.position, false);
        }
        let target = self.selected.unwrap_or(0) as f32;
        if let Some((started, from)) = self.motion {
            let t = (now.duration_since(started).as_secs_f32() / SETTLE.as_secs_f32()).min(1.);
            return (target + (from - target) * (1. - t).powi(3), t < 1.);
        }
        (target, false)
    }

    pub(super) fn configure(&mut self, enabled: bool, animate: bool, cx: &mut Context<Self>) {
        self.animate = animate;
        if !animate {
            self.motion = None;
        }
        if self.enabled != enabled {
            self.cancel(cx);
            self.enabled = enabled;
        }
    }

    pub(super) fn measure(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let width = f32::from(width).max(0.);
        if self.width != width {
            self.cancel(cx);
            self.width = width;
        }
    }

    pub(super) fn claimed(&self) -> bool {
        self.drag.as_ref().is_some_and(|drag| drag.claimed)
    }

    pub(super) fn begin(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.enabled || self.count < 2 || self.width <= 0. || self.drag.is_some() {
            return;
        }
        let now = cx.background_executor().now();
        let base = self.visual(now).0;
        self.drag = Some(Drag {
            pointer,
            start: position,
            base,
            anchor: base.round() as usize,
            position: base,
            claimed: false,
            rejected: false,
            samples: VecDeque::from([(now, f32::from(position.x))]),
        });
    }

    pub(super) fn move_to(
        &mut self,
        pointer: Pointer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let now = cx.background_executor().now();
        let visual = self.visual(now).0;
        let Some(drag) = self
            .drag
            .as_mut()
            .filter(|drag| drag.pointer == pointer && !drag.rejected)
        else {
            return false;
        };
        let dx = f32::from(position.x - drag.start.x);
        let dy = f32::from(position.y - drag.start.y).abs();
        if !drag.claimed && dx.abs().max(dy) > SLOP {
            if dx.abs() <= dy * 1.2
                || (visual <= 0. && dx > 0.)
                || (visual >= (self.count - 1) as f32 && dx < 0.)
            {
                drag.rejected = true;
                return false;
            }
            drag.claimed = true;
            drag.base = visual;
            drag.anchor = visual.round() as usize;
            self.motion = None;
        }
        if !drag.claimed {
            return false;
        }
        let min = drag.anchor.saturating_sub(1) as f32;
        let max = (drag.anchor + 1).min(self.count - 1) as f32;
        drag.position = (drag.base - dx / self.width).clamp(min, max);
        drag.samples.push_back((now, f32::from(position.x)));
        while drag.samples.len() > 2 && now.duration_since(drag.samples[1].0) >= VELOCITY_WINDOW {
            drag.samples.pop_front();
        }
        cx.notify();
        true
    }

    pub(super) fn end(
        &mut self,
        pointer: Pointer,
        cancelled: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .drag
            .as_ref()
            .is_none_or(|drag| drag.pointer != pointer)
        {
            return false;
        }
        let drag = self.drag.take().unwrap();
        if !drag.claimed {
            return false;
        }
        let now = cx.background_executor().now();
        let target = if cancelled {
            self.selected.unwrap_or(0)
        } else {
            let velocity = drag.samples.front().zip(drag.samples.back()).map_or(
                0.,
                |((first, x), (last, end))| {
                    let elapsed = now.duration_since(*first).as_secs_f32();
                    if elapsed <= 0. || now.duration_since(*last) >= VELOCITY_WINDOW {
                        0.
                    } else {
                        (end - x) / elapsed
                    }
                },
            );
            let distance = drag.position - drag.anchor as f32;
            let direction = if velocity.abs() >= 500.
                && (drag.position - drag.base).abs() * self.width >= SLOP * 2.
            {
                -velocity.signum()
            } else if distance.abs() >= 0.25 {
                distance.signum()
            } else {
                0.
            };
            (drag.anchor as isize + direction as isize).clamp(0, self.count as isize - 1) as usize
        };
        self.settle(target, drag.position, now, cx);
        true
    }

    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) -> bool {
        self.drag
            .as_ref()
            .map(|drag| drag.pointer)
            .is_some_and(|pointer| self.end(pointer, true, cx))
    }

    pub(super) fn touch(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        prevented: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let pointer = Pointer::Touch(event.id);
        if event.phase == TouchPhase::Started {
            if self.contacts.contains(&event.id) {
                return false;
            }
            let first = self.contacts.is_empty();
            self.contacts.push(event.id);
            if first {
                if inside && !prevented {
                    self.begin(pointer, event.position, cx);
                }
                return false;
            }
            return self.cancel(cx);
        }
        if !self.contacts.contains(&event.id) {
            return false;
        }
        let consumed = if prevented || event.phase == TouchPhase::Cancelled {
            self.cancel(cx)
        } else if event.phase == TouchPhase::Moved {
            self.move_to(pointer, event.position, cx)
        } else {
            if self.claimed() {
                self.move_to(pointer, event.position, cx);
            }
            self.end(pointer, false, cx)
        };
        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.contacts.retain(|id| *id != event.id);
        }
        consumed
    }
}
