use gpui::{
    Context, EventEmitter, Pixels, Point, ScrollHandle, Subscription, TouchEvent, TouchId,
    TouchPhase, Window, px,
};
use std::time::{Duration, Instant};

const THRESHOLD: f32 = 64.;
const HOLD: f32 = 48.;
const SETTLE: Duration = Duration::from_millis(180);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RefreshStatus {
    Idle,
    Pulling { progress: f32 },
    Ready,
    Refreshing,
}

/// Emitted once per request. The application calls `finish` after success or failure.
pub struct RefreshRequested;

/// State for one refresh viewport. Keep it alive across renders.
pub struct RefreshState {
    scroll: ScrollHandle,
    gesture: Gesture,
    refreshing: bool,
    motion: Option<(Instant, f32, f32)>,
    _observation: Subscription,
}

impl EventEmitter<RefreshRequested> for RefreshState {}

impl RefreshState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            scroll: ScrollHandle::new(),
            gesture: Gesture::default(),
            refreshing: false,
            motion: None,
            _observation: window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        }
    }

    pub fn scroll_handle(&self) -> ScrollHandle {
        self.scroll.clone()
    }

    pub fn status(&self) -> RefreshStatus {
        if self.refreshing {
            RefreshStatus::Refreshing
        } else if self.gesture.offset >= THRESHOLD {
            RefreshStatus::Ready
        } else if self.gesture.claimed {
            RefreshStatus::Pulling {
                progress: self.gesture.offset / THRESHOLD,
            }
        } else {
            RefreshStatus::Idle
        }
    }

    /// Requests refresh from a button, keyboard action, or accessibility action.
    /// Returns false while another refresh is pending.
    pub fn request(&mut self, cx: &mut Context<Self>) -> bool {
        if self.refreshing {
            return false;
        }
        let from = self.visual(Instant::now()).0;
        self.refreshing = true;
        self.gesture.reject();
        self.motion = Some((Instant::now(), from.into(), HOLD));
        cx.notify();
        cx.emit(RefreshRequested);
        true
    }

    /// Ends the pending refresh, including when the application's request fails.
    pub fn finish(&mut self, cx: &mut Context<Self>) {
        if !self.refreshing {
            return;
        }
        let from = self.visual(Instant::now()).0;
        self.refreshing = false;
        self.motion = Some((Instant::now(), from.into(), 0.));
        cx.notify();
    }

    pub(super) fn visual(&self, now: Instant) -> (Pixels, bool) {
        if let Some((start, from, to)) = self.motion {
            let t = (now.duration_since(start).as_secs_f32() / SETTLE.as_secs_f32()).min(1.);
            return (px(from + (to - from) * (1. - (1. - t).powi(3))), t < 1.);
        }
        (
            px(if self.refreshing {
                HOLD
            } else {
                self.gesture.offset
            }),
            false,
        )
    }

    pub(super) fn touch(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        prevented: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let from = self.visual(Instant::now()).0;
        let was_claimed = self.gesture.claimed;
        let (consumed, request) = self.gesture.touch(
            event,
            inside,
            self.scroll.offset().y >= px(-0.5),
            self.refreshing || prevented,
        );
        if self.gesture.claimed {
            self.motion = None;
        } else if was_claimed {
            self.motion = Some((Instant::now(), from.into(), 0.));
        }
        if request {
            self.request(cx);
        } else if consumed || was_claimed {
            cx.notify();
        }
        consumed
    }
}

#[derive(Default)]
struct Gesture {
    contacts: Vec<TouchId>,
    start: Point<Pixels>,
    blocked: bool,
    claimed: bool,
    offset: f32,
}

impl Gesture {
    fn reject(&mut self) {
        self.blocked = true;
        self.claimed = false;
        self.offset = 0.;
    }

    fn touch(
        &mut self,
        event: &TouchEvent,
        inside: bool,
        at_top: bool,
        disabled: bool,
    ) -> (bool, bool) {
        let mut consumed = self.claimed;
        if event.phase == TouchPhase::Started {
            if self.contacts.is_empty() {
                if !inside {
                    return (false, false);
                }
                self.blocked = disabled || !at_top;
                self.start = event.position;
                self.offset = 0.;
            } else {
                self.reject();
            }
            self.contacts.push(event.id);
            return (consumed, false);
        }
        if !self.contacts.contains(&event.id) {
            return (false, false);
        }
        if disabled {
            self.reject();
        }
        match event.phase {
            TouchPhase::Moved if !self.blocked => {
                let delta = event.position - self.start;
                let dx = f32::from(delta.x).abs();
                let dy = f32::from(delta.y);
                if !self.claimed && dx.max(dy.abs()) > 8. {
                    if !at_top || dy <= dx {
                        self.reject();
                    } else {
                        self.claimed = true;
                    }
                }
                if self.claimed {
                    let distance = (dy - 8.).max(0.);
                    self.offset = 128. * distance / (128. + distance);
                    consumed = true;
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let request = self.claimed
                    && !self.blocked
                    && event.phase == TouchPhase::Ended
                    && self.offset >= THRESHOLD;
                self.contacts.retain(|id| *id != event.id);
                self.reject();
                return (consumed, request);
            }
            _ => {}
        }
        (consumed, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::point;

    fn event(id: u64, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            id: TouchId(id),
            phase,
            position: point(px(x), px(y)),
            force: None,
        }
    }

    #[test]
    fn scrolling_and_horizontal_gestures_never_turn_into_refreshes() {
        for (at_top, x, y) in [(false, 0., 180.), (true, 90., 20.), (true, 0., -90.)] {
            let mut gesture = Gesture::default();
            assert_eq!(
                gesture.touch(&event(1, TouchPhase::Started, 0., 0.), true, at_top, false),
                (false, false)
            );
            assert_eq!(
                gesture.touch(&event(1, TouchPhase::Moved, x, y), true, at_top, false),
                (false, false)
            );
            // Reaching the top or changing direction does not steal an existing scroll.
            assert_eq!(
                gesture.touch(&event(1, TouchPhase::Moved, 0., 300.), true, true, false),
                (false, false)
            );
            assert_eq!(
                gesture.touch(&event(1, TouchPhase::Ended, 0., 300.), true, true, false),
                (false, false)
            );
        }
    }

    #[test]
    fn release_triggers_once_but_reversal_cancel_and_second_contact_do_not() {
        for interruption in 0..4 {
            let mut gesture = Gesture::default();
            gesture.touch(&event(1, TouchPhase::Started, 0., 0.), true, true, false);
            assert_eq!(
                gesture.touch(&event(1, TouchPhase::Moved, 0., 200.), true, true, false),
                (true, false)
            );
            match interruption {
                1 => {
                    gesture.touch(&event(1, TouchPhase::Moved, 0., 12.), true, true, false);
                }
                2 => {
                    gesture.touch(
                        &event(1, TouchPhase::Cancelled, 0., 200.),
                        true,
                        true,
                        false,
                    );
                }
                3 => {
                    gesture.touch(&event(2, TouchPhase::Started, 10., 200.), true, true, false);
                }
                _ => {}
            }
            let (_, requested) =
                gesture.touch(&event(1, TouchPhase::Ended, 0., 200.), true, true, false);
            assert_eq!(requested, interruption == 0);
            assert!(
                !gesture
                    .touch(&event(1, TouchPhase::Ended, 0., 200.), true, true, false)
                    .1
            );
        }
    }
}
