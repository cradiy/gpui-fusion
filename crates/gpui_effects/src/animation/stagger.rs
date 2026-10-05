use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, Stateful, StyleRefinement, Styled, Window,
    div, prelude::*,
};

use super::presence::{PresenceFrame, PresencePhase, PresenceState};

/// Item order used when scheduling an exit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StaggerOrder {
    Forward,
    #[default]
    Reverse,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Timing {
    visible: bool,
    duration: Duration,
    interval: Duration,
    exit_order: StaggerOrder,
    enabled: bool,
}

struct Item {
    id: ElementId,
    build: Box<dyn FnOnce(PresenceFrame) -> AnyElement>,
}

struct State {
    timing: Timing,
    items: HashMap<ElementId, PresenceState>,
}

/// A keyed group whose items enter in sequence and exit in reverse by default.
/// Keep the group and its items supplied until exit finishes.
pub fn staggered_presence(id: impl Into<ElementId>, visible: bool) -> StaggeredPresence {
    StaggeredPresence {
        id: id.into(),
        timing: Timing {
            visible,
            duration: Duration::from_millis(240),
            interval: Duration::from_millis(60),
            exit_order: StaggerOrder::Reverse,
            enabled: true,
        },
        animate_initial: false,
        items: Vec::new(),
        container: Some(div().id("container")),
    }
}

/// Caller-styled flex or grid container. Item builders define their own animation.
/// Item layout slots remain until the entire group finishes exiting.
pub struct StaggeredPresence {
    id: ElementId,
    timing: Timing,
    animate_initial: bool,
    items: Vec<Item>,
    container: Option<Stateful<Div>>,
}

impl StaggeredPresence {
    /// Adds an item with a unique, stable key. Removing it skips its exit animation.
    pub fn item<E: IntoElement>(
        mut self,
        id: impl Into<ElementId>,
        build: impl FnOnce(PresenceFrame) -> E + 'static,
    ) -> Self {
        self.items.push(Item {
            id: id.into(),
            build: Box::new(move |frame| build(frame).into_any_element()),
        });
        self
    }

    /// Full travel time for each item. Zero snaps the whole group, including delays.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.timing.duration = duration;
        self
    }
    /// Delay between successive items. Defaults to 60 ms.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.timing.interval = interval;
        self
    }
    /// Selects forward or reverse scheduling for exits.
    pub fn exit_order(mut self, order: StaggerOrder) -> Self {
        self.timing.exit_order = order;
        self
    }
    /// Skip interpolation and delays, for example for reduced motion.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.timing.enabled = enabled;
        self
    }
    /// Animate items on the group's first visible render. Defaults to false.
    pub fn animate_initial(mut self, animate: bool) -> Self {
        self.animate_initial = animate;
        self
    }
}

impl Styled for StaggeredPresence {
    fn style(&mut self) -> &mut StyleRefinement {
        self.container
            .as_mut()
            .expect("cannot style after layout")
            .style()
    }
}

impl IntoElement for StaggeredPresence {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for StaggeredPresence {
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
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let keys: HashSet<_> = self.items.iter().map(|item| item.id.clone()).collect();
        assert_eq!(
            keys.len(),
            self.items.len(),
            "staggered presence requires unique item IDs"
        );
        let now = cx.background_executor().now();
        let timing = self.timing;
        let target = if timing.visible { 1. } else { 0. };
        let (frames, moving) = window.with_element_state(id.unwrap(), |state: Option<State>, _| {
            let initial = state.is_none();
            let mut state = state.unwrap_or(State {
                timing,
                items: HashMap::new(),
            });
            let retarget = state.timing != timing;
            let mut moving = false;
            let frames = self
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let previous = state.items.get(&item.id);
                    if retarget || previous.is_none() {
                        let current = previous.map(|state| state.sample(now).0).unwrap_or(
                            if initial && !self.animate_initial {
                                target
                            } else {
                                0.
                            },
                        );
                        let rank = if !timing.visible && timing.exit_order == StaggerOrder::Reverse
                        {
                            self.items.len() - 1 - index
                        } else {
                            index
                        };
                        let delay = timing
                            .interval
                            .saturating_mul(u32::try_from(rank).unwrap_or(u32::MAX));
                        let snap = !timing.enabled || timing.duration.is_zero();
                        state.items.insert(
                            item.id.clone(),
                            PresenceState::transition(
                                if snap { target } else { current },
                                target,
                                timing.duration,
                                if snap { Duration::ZERO } else { delay },
                                now,
                            ),
                        );
                    }
                    let (progress, active) = state.items[&item.id].sample(now);
                    moving |= active;
                    let phase = if !timing.visible {
                        PresencePhase::Exiting
                    } else if active {
                        PresencePhase::Entering
                    } else {
                        PresencePhase::Visible
                    };
                    PresenceFrame { progress, phase }
                })
                .collect::<Vec<_>>();
            state.items.retain(|key, _| keys.contains(key));
            state.timing = timing;
            ((frames, moving), state)
        });
        if moving {
            window.request_animation_frame();
        }
        let mut container = self
            .container
            .take()
            .expect("stagger layout requested twice");
        if !timing.visible && !moving {
            container = container.hidden();
        } else {
            container =
                container.children(self.items.drain(..).zip(frames).map(|(item, frame)| {
                    StaggerChild {
                        id: item.id,
                        child: (item.build)(frame),
                        paint: frame.progress > 0.,
                        block_mouse: frame.phase != PresencePhase::Visible,
                    }
                }));
        }
        let mut container = container.into_any_element();
        (container.request_layout(window, cx), container)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

struct StaggerChild {
    id: ElementId,
    child: AnyElement,
    paint: bool,
    block_mouse: bool,
}

impl IntoElement for StaggerChild {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for StaggerChild {
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
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.paint {
            self.child.prepaint(window, cx);
        }
        if self.block_mouse {
            window.insert_hitbox(bounds, HitboxBehavior::BlockMouse);
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
        if self.paint {
            self.child.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests;
