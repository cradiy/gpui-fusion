use scheduler::Instant;
use std::{cell::Cell, rc::Rc};

use gpui::{
    Along, App, Axis, Bounds, DispatchPhase, Entity, HitboxBehavior, HitboxId, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TouchId, TouchPhase,
    Window, canvas, prelude::*, px,
};

use super::motion::Motion;

pub(super) type ChangeCallback<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

pub(super) struct OptionGeometry<T> {
    pub value: T,
    pub disabled: bool,
    pub bounds: Cell<Option<Bounds<Pixels>>>,
}

pub(super) struct State {
    pub motion: Motion,
    pub axis: Axis,
    capture: Option<HitboxId>,
    press: Option<Press>,
    touch: Option<TouchId>,
}

struct Press {
    start: Point<Pixels>,
    bounds: Bounds<Pixels>,
    dragging: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            motion: Motion::default(),
            axis: Axis::Horizontal,
            capture: None,
            press: None,
            touch: None,
        }
    }

    pub fn cancel(&mut self, window: &mut Window) {
        if self.capture.is_some() && window.captured_hitbox() == self.capture {
            window.release_pointer();
        }
        self.press = None;
        self.touch = None;
        self.motion.cancel_press();
        self.motion.pointer = None;
    }

    fn begin(&mut self, position: Point<Pixels>, target: Bounds<Pixels>, selected: bool) {
        self.motion.begin_press(Instant::now());
        self.motion.pointer = Some(position);
        self.press = Some(Press {
            start: position,
            bounds: if selected {
                self.motion.current_bounds().unwrap_or(target)
            } else {
                target
            },
            dragging: false,
        });
    }

    fn finish<T: Clone + PartialEq>(
        &mut self,
        position: Point<Pixels>,
        root: Bounds<Pixels>,
        options: &[OptionGeometry<T>],
        selected: &T,
    ) -> Option<T> {
        self.move_lens(position, root, options);
        let press = self.press.take();
        self.touch = None;
        let center = self
            .motion
            .drag_bounds
            .map(|b| b.center().along(self.axis) + root.origin.along(self.axis));
        self.motion.drag_bounds = None;
        self.motion.pressed = false;
        if !press?.dragging {
            return None;
        }
        let center = center?;
        let option = options
            .iter()
            .filter(|o| !o.disabled && o.bounds.get().is_some())
            .min_by(|a, b| {
                let distance = |o: &OptionGeometry<T>| {
                    f32::from((o.bounds.get().unwrap().center().along(self.axis) - center).abs())
                };
                distance(a).total_cmp(&distance(b))
            })?;
        (&option.value != selected).then(|| option.value.clone())
    }

    fn move_lens<T>(
        &mut self,
        position: Point<Pixels>,
        root: Bounds<Pixels>,
        options: &[OptionGeometry<T>],
    ) {
        let Some(press) = &mut self.press else { return };
        let axis = self.axis;
        let delta = position.along(axis) - press.start.along(axis);
        press.dragging |= delta.abs() > px(4.);
        self.motion.pointer = Some(position);
        if !press.dragging {
            return;
        }
        let mut centers = options.iter().filter(|o| !o.disabled).filter_map(|o| {
            o.bounds
                .get()
                .map(|b| b.center().along(axis) - root.origin.along(axis))
        });
        let Some(first) = centers.next() else { return };
        let (min, max) = centers.fold((first, first), |(min, max), x| (min.min(x), max.max(x)));
        let mut bounds = press.bounds;
        bounds.origin = bounds.origin.apply_along(axis, |_| {
            (bounds.center().along(axis) + delta).clamp(min, max) - bounds.size.along(axis) / 2.
        });
        self.motion.drag_bounds = Some(bounds);
    }
}

fn option_at<T>(
    position: Point<Pixels>,
    root: Bounds<Pixels>,
    options: &[OptionGeometry<T>],
    axis: Axis,
) -> Option<(usize, Bounds<Pixels>)> {
    if !root.contains(&position) {
        return None;
    }
    if let Some((index, option, bounds)) = options.iter().enumerate().find_map(|(index, option)| {
        option
            .bounds
            .get()
            .filter(|b| b.contains(&position))
            .map(|b| (index, option, b))
    }) {
        return (!option.disabled).then_some((index, bounds));
    }
    options
        .iter()
        .enumerate()
        .filter(|(_, option)| !option.disabled)
        .filter_map(|(index, option)| option.bounds.get().map(|bounds| (index, bounds)))
        .min_by(|(_, a), (_, b)| {
            let distance = |bounds: &Bounds<Pixels>| {
                f32::from((bounds.center().along(axis) - position.along(axis)).abs())
            };
            distance(a).total_cmp(&distance(b))
        })
}

pub(super) fn interaction<T: Clone + PartialEq + 'static>(
    state: Entity<State>,
    options: Rc<Vec<OptionGeometry<T>>>,
    selected: T,
    root_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    callback: Option<ChangeCallback<T>>,
    enabled: bool,
) -> impl IntoElement {
    let prepaint = state.clone();
    canvas(
        move |bounds, window, cx| {
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            prepaint.update(cx, |state, _| {
                if !enabled || !window.is_window_active() {
                    state.cancel(window);
                } else if state.capture.is_some() && window.captured_hitbox() == state.capture {
                    window.capture_pointer(hitbox.id);
                } else if state.press.is_some() {
                    state.cancel(window);
                }
                state.capture = Some(hitbox.id);
            });
            hitbox
        },
        move |bounds, hitbox, window, _| {
            let bounds = root_bounds.get().unwrap_or(bounds);
            if !enabled {
                return;
            }
            let touch_state = state.clone();
            let touch_options = options.clone();
            let touch_selected = selected.clone();
            let touch_callback = callback.clone();
            let touch_hitbox = hitbox.clone();
            window.on_touch_event(move |event, phase, window, cx| {
                if event.phase == TouchPhase::Started {
                    if !phase.bubble()
                        || window.default_prevented()
                        || !touch_hitbox.is_hovered(window)
                        || !bounds.contains(&event.position)
                    {
                        return;
                    }
                    if touch_state.read(cx).press.is_some() {
                        if touch_state.read(cx).touch.is_some() {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        return;
                    }
                    let Some((index, option_bounds)) = option_at(
                        event.position,
                        bounds,
                        &touch_options,
                        touch_state.read(cx).axis,
                    ) else {
                        return;
                    };
                    window.capture_pointer(touch_hitbox.id);
                    touch_state.update(cx, |state, cx| {
                        state.touch = Some(event.id);
                        state.begin(
                            event.position,
                            Bounds::new(option_bounds.origin - bounds.origin, option_bounds.size),
                            touch_options[index].value == touch_selected,
                        );
                        cx.notify();
                    });
                    if touch_options[index].value != touch_selected
                        && let Some(callback) = &touch_callback
                    {
                        callback(touch_options[index].value.clone(), window, cx);
                    }
                } else {
                    if !phase.capture() || touch_state.read(cx).touch != Some(event.id) {
                        return;
                    }
                    let value = touch_state.update(cx, |state, cx| {
                        let value = match event.phase {
                            TouchPhase::Moved => {
                                state.move_lens(event.position, bounds, &touch_options);
                                None
                            }
                            TouchPhase::Ended => {
                                window.release_pointer();
                                state.finish(
                                    event.position,
                                    bounds,
                                    &touch_options,
                                    &touch_selected,
                                )
                            }
                            TouchPhase::Cancelled => {
                                state.cancel(window);
                                None
                            }
                            TouchPhase::Started => unreachable!(),
                        };
                        cx.notify();
                        value
                    });
                    if let Some(value) = value
                        && let Some(callback) = &touch_callback
                    {
                        callback(value, window, cx);
                    }
                }
                window.prevent_default();
                cx.stop_propagation();
            });
            let down_state = state.clone();
            let down_options = options.clone();
            let down_hitbox = hitbox.clone();
            let down_selected = selected.clone();
            let down_callback = callback.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Left
                    || !down_hitbox.is_hovered(window)
                    || down_state.read(cx).touch.is_some()
                {
                    return;
                }
                let Some((index, option_bounds)) = option_at(
                    event.position,
                    bounds,
                    &down_options,
                    down_state.read(cx).axis,
                ) else {
                    return;
                };
                window.capture_pointer(down_hitbox.id);
                down_state.update(cx, |state, cx| {
                    state.begin(
                        event.position,
                        Bounds::new(option_bounds.origin - bounds.origin, option_bounds.size),
                        down_options[index].value == down_selected,
                    );
                    cx.notify();
                });
                if down_options[index].value != down_selected
                    && let Some(callback) = &down_callback
                {
                    callback(down_options[index].value.clone(), window, cx);
                }
                cx.stop_propagation();
            });
            let move_state = state.clone();
            let move_options = options.clone();
            let move_hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.dragging()
                    && window.captured_hitbox() == Some(move_hitbox.id)
                    && move_state.read(cx).touch.is_none()
                {
                    move_state.update(cx, |state, cx| {
                        state.move_lens(event.position, bounds, &move_options);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture
                    || event.button != MouseButton::Left
                    || window.captured_hitbox() != Some(hitbox.id)
                    || state.read(cx).touch.is_some()
                {
                    return;
                }
                window.release_pointer();
                let value = state.update(cx, |state, cx| {
                    let value = state.finish(event.position, bounds, &options, &selected);
                    cx.notify();
                    value
                });
                if let Some(value) = value
                    && let Some(callback) = &callback
                {
                    callback(value, window, cx);
                }
                cx.stop_propagation();
            });
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}
