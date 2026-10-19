use scheduler::Instant;
use std::time::Duration;

mod appearance;
mod modal;
mod sheet_frame;
mod sheet_scroll;
pub use sheet_scroll::SheetScrollArea;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Global, KeyDownEvent, MouseButton,
    Refineable as _, Render, Window, WindowId, deferred, div, prelude::*,
};

pub use appearance::{ModalAppearance, ModalButtonAppearance};
use modal::ModalFooter;
pub use modal::{Modal, ModalPlacement};

use super::input::Submit;

const MODAL_PRIORITY: usize = 1_000;
pub(crate) const SHEET_HANDLE_HEIGHT: gpui::Pixels = gpui::px(36.);

struct ActiveModal {
    modal: Modal,
    window_id: WindowId,
    previous_focus: Option<FocusHandle>,
}

pub struct ModalLayer {
    active: Option<ActiveModal>,
    focus_handle: FocusHandle,
    appearance: ModalAppearance,
    sheet_drag: Option<(Option<gpui::TouchId>, gpui::Pixels)>,
    sheet_exclusions: Vec<gpui::Bounds<gpui::Pixels>>,
    sheet_surface_touch: Option<sheet_scroll::SheetTouch>,
    sheet_scroll_region: Option<sheet_scroll::ScrollRegion>,
    scroll_motion: Option<sheet_scroll::ScrollMotion>,
    held_scroll: Option<gpui::ScrollHandle>,
    scroll_settle: Option<SheetMotion>,
    scroll_hold_rebound: bool,
    edge_pull: f32,
    edge_motion: Option<SheetMotion>,
    sheet_offset: gpui::Pixels,
    sheet_velocity: f32,
    sheet_sample: Option<(gpui::Pixels, Instant)>,
    sheet_motion: Option<SheetMotion>,
    sheet_height: gpui::Pixels,
    sheet_collapsed_height: gpui::Pixels,
    sheet_max_height: gpui::Pixels,
    sheet_expanded: bool,
    sheet_drag_expansion: f32,
    resize_motion: Option<SheetMotion>,
    closing: bool,
    dismiss_task: Option<gpui::Task<()>>,
}

struct SheetMotion {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
}

impl SheetMotion {
    fn value(&self, now: Instant) -> (f32, bool) {
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .min(1.);
        let eased = 1. - (1. - t).powi(3);
        (
            self.from + (self.to - self.from) * eased,
            self.from != self.to && t < 1.,
        )
    }
}

impl ModalLayer {
    fn new(appearance: ModalAppearance, cx: &mut Context<Self>) -> Self {
        Self {
            active: None,
            focus_handle: cx.focus_handle(),
            appearance,
            sheet_drag: None,
            sheet_exclusions: Vec::new(),
            sheet_surface_touch: None,
            sheet_scroll_region: None,
            scroll_motion: None,
            held_scroll: None,
            scroll_settle: None,
            scroll_hold_rebound: true,
            edge_pull: 0.,
            edge_motion: None,
            sheet_offset: gpui::px(0.),
            sheet_velocity: 0.,
            sheet_sample: None,
            sheet_motion: None,
            sheet_height: gpui::px(360.),
            sheet_collapsed_height: gpui::px(0.),
            sheet_max_height: gpui::px(0.),
            sheet_expanded: false,
            sheet_drag_expansion: 0.,
            resize_motion: None,
            closing: false,
            dismiss_task: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    fn show(&mut self, modal: Modal, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_task = None;
        self.release_scroll_hold();
        self.sheet_scroll_region = None;
        self.scroll_motion = None;
        self.edge_pull = 0.;
        self.edge_motion = None;
        self.sheet_expanded = false;
        self.resize_motion = None;
        self.closing = false;
        self.sheet_drag = None;
        self.sheet_surface_touch = None;
        self.sheet_offset = gpui::px(0.);
        self.sheet_motion = modal.sheet_animation.map(|duration| SheetMotion {
            from: 1.,
            to: 0.,
            started: cx.background_executor().now(),
            duration,
        });
        self.active = Some(ActiveModal {
            modal,
            window_id: window.window_handle().window_id(),
            previous_focus: window.focused(cx),
        });
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        if active.window_id != window.window_handle().window_id() || self.closing {
            return;
        }
        if let Some(duration) = active.modal.sheet_animation {
            let from = self
                .sheet_motion
                .as_ref()
                .map_or(0., |motion| motion.value(cx.background_executor().now()).0)
                + self.sheet_offset.as_f32() / self.sheet_height.as_f32().max(1.);
            self.sheet_offset = gpui::px(0.);
            self.sheet_drag = None;
            self.sheet_surface_touch = None;
            self.closing = true;
            self.sheet_motion = Some(SheetMotion {
                from,
                to: 1.,
                started: cx.background_executor().now(),
                duration,
            });
            self.dismiss_task = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(duration).await;
                let _ = this.update_in(cx, |this, window, cx| this.finish_dismiss(window, cx));
            }));
            cx.notify();
            return;
        }
        self.finish_dismiss(window, cx);
    }

    fn finish_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = self.active.take() else {
            return;
        };

        if active.window_id != window.window_handle().window_id() {
            self.active = Some(active);
            return;
        }
        if let Some(previous_focus) = active.previous_focus {
            window.focus(&previous_focus, cx);
        } else {
            window.blur();
        }
        self.release_scroll_hold();
        self.sheet_drag = None;
        self.sheet_surface_touch = None;
        self.sheet_offset = gpui::px(0.);
        self.sheet_motion = None;
        self.closing = false;
        cx.notify();
    }

    fn sheet_expansion(&self, cx: &App) -> f32 {
        if self.sheet_drag.is_some() {
            self.sheet_drag_expansion
        } else {
            self.resize_motion
                .as_ref()
                .map_or(f32::from(self.sheet_expanded), |motion| {
                    motion.value(cx.background_executor().now()).0
                })
        }
    }

    fn begin_sheet_drag(&mut self, touch: Option<gpui::TouchId>, y: gpui::Pixels, cx: &App) {
        self.sheet_velocity = 0.;
        self.sheet_sample = Some((gpui::px(0.), cx.background_executor().now()));
        if let Some(scroll) = self
            .sheet_scroll_region
            .as_ref()
            .map(|region| region.scroll.clone())
        {
            self.hold_scroll(&scroll);
        }
        self.sheet_drag_expansion = self.sheet_expansion(cx);
        self.resize_motion = None;
        self.sheet_motion = None;
        self.sheet_drag = Some((touch, y));
        self.sheet_offset = gpui::px(0.);
    }

    fn update_sheet_drag(&mut self, delta: gpui::Pixels, cx: &App) {
        let now = cx.background_executor().now();
        if let Some((previous, sampled)) = self.sheet_sample {
            let dt = now.saturating_duration_since(sampled).as_secs_f32();
            if dt > 0.001 && delta != previous {
                self.sheet_velocity = ((delta - previous).as_f32() / dt).clamp(-6500., 6500.);
            }
        }
        self.sheet_sample = Some((delta, now));
        let expandable = self
            .active
            .as_ref()
            .is_some_and(|active| active.modal.sheet_expandable);
        self.sheet_offset = if expandable {
            delta
        } else {
            delta.max(gpui::px(0.))
        };
    }

    fn finish_sheet_drag(&mut self, cancelled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sheet_drag.take().is_none() {
            return;
        }
        if self.sheet_sample.is_some_and(|(_, sampled)| {
            cx.background_executor()
                .now()
                .saturating_duration_since(sampled)
                >= Duration::from_millis(100)
        }) {
            self.sheet_velocity = 0.;
        }
        let expandable = self
            .active
            .as_ref()
            .is_some_and(|active| active.modal.sheet_expandable);
        let duration = self
            .active
            .as_ref()
            .and_then(|active| active.modal.sheet_animation)
            .unwrap_or(Duration::from_millis(260));
        let gap = (self.sheet_max_height - self.sheet_collapsed_height)
            .as_f32()
            .max(0.);
        let below_fit = self.sheet_offset.as_f32() - self.sheet_drag_expansion * gap;
        if expandable && below_fit > 0. {
            self.sheet_expanded = false;
            self.resize_motion = None;
            self.sheet_offset = gpui::px(below_fit);
            self.sheet_height = self.sheet_collapsed_height;
            if !cancelled && below_fit >= 72. && self.sheet_velocity >= -120. {
                self.dismiss(window, cx);
            } else {
                self.sheet_motion = Some(SheetMotion {
                    from: below_fit / self.sheet_collapsed_height.as_f32().max(1.),
                    to: 0.,
                    started: cx.background_executor().now(),
                    duration,
                });
                self.sheet_offset = gpui::px(0.);
                cx.notify();
            }
            return;
        }
        if expandable && (self.sheet_expanded || self.sheet_offset < gpui::px(0.)) {
            let gap = (self.sheet_max_height - self.sheet_collapsed_height)
                .as_f32()
                .max(1.);
            let from = (self.sheet_drag_expansion - self.sheet_offset.as_f32() / gap).clamp(0., 1.);
            if !cancelled {
                if self.sheet_velocity < -450. && self.sheet_offset < gpui::px(0.) {
                    self.sheet_expanded = true;
                } else if self.sheet_velocity > 450. && self.sheet_offset > gpui::px(0.) {
                    self.sheet_expanded = false;
                } else if self.sheet_offset < gpui::px(-48.) {
                    self.sheet_expanded = true;
                } else if self.sheet_offset > gpui::px(48.) {
                    self.sheet_expanded = false;
                }
            }
            self.resize_motion = Some(SheetMotion {
                from,
                to: f32::from(self.sheet_expanded),
                started: cx.background_executor().now(),
                duration,
            });
            self.sheet_offset = gpui::px(0.);
            cx.notify();
        } else if !cancelled && self.sheet_offset >= gpui::px(72.) {
            self.dismiss(window, cx);
        } else {
            self.sheet_motion = self
                .active
                .as_ref()
                .and_then(|active| active.modal.sheet_animation)
                .map(|duration| SheetMotion {
                    from: self.sheet_offset.as_f32() / self.sheet_height.as_f32().max(1.),
                    to: 0.,
                    started: cx.background_executor().now(),
                    duration,
                });
            self.sheet_offset = gpui::px(0.);
            cx.notify();
        }
    }

    fn execute_ok(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let should_close = self
            .active
            .as_ref()
            .and_then(|active| active.modal.on_ok.as_ref())
            .is_none_or(|callback| callback(window, cx));
        if should_close {
            self.dismiss(window, cx);
        }
    }

    fn default_button(
        id: &'static str,
        label: AnyElement,
        appearance: ModalButtonAppearance,
    ) -> AnyElement {
        div()
            .id(id)
            .h(appearance.height)
            .px(appearance.padding_x)
            .flex()
            .items_center()
            .justify_center()
            .rounded(appearance.radius)
            .border(appearance.border_width)
            .border_color(appearance.border)
            .bg(appearance.background)
            .text_color(appearance.foreground)
            .cursor_pointer()
            .hover(move |style| style.bg(appearance.hover_background))
            .child(label)
            .into_any_element()
    }
}

impl Render for ModalLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scrolling = self.tick_scroll(cx.background_executor().now());
        let Some(active) = self.active.as_ref() else {
            return div().into_any_element();
        };
        if active.window_id != window.window_handle().window_id() {
            return div().into_any_element();
        }

        self.sheet_exclusions.clear();
        self.sheet_scroll_region = None;
        let (motion, moving) = self.sheet_motion.as_ref().map_or((0., false), |motion| {
            motion.value(cx.background_executor().now())
        });
        if scrolling
            || moving
            || self
                .resize_motion
                .as_ref()
                .is_some_and(|motion| motion.value(cx.background_executor().now()).1)
        {
            window.request_animation_frame();
        }
        let expansion = self.sheet_expansion(cx);
        let modal = &active.modal;
        let expandable = modal.sheet_expandable;
        let appearance = modal.appearance.unwrap_or(self.appearance);
        let close_on_escape = modal.close_on_escape;
        let close_on_backdrop = modal.close_on_backdrop;
        let ok_on_enter = modal.ok_on_enter && matches!(&modal.footer, ModalFooter::Default);
        let styled = modal.styled;
        let placement = modal.placement;
        let panel_style = modal.style.clone();
        self.sheet_height = match panel_style.size.height {
            Some(gpui::Length::Definite(height)) => {
                height.to_pixels(window.viewport_size().height.into(), window.rem_size())
            }
            _ => window.viewport_size().height,
        }
        .min(window.viewport_size().height);
        let title = modal.title.as_ref().map(|slot| slot(window, cx));
        let close_button = modal.close_button.as_ref().map(|slot| slot(window, cx));
        let content = (modal.content)(window, cx);

        let mut panel = div()
            .id("global-modal-content")
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |layer, event: &gpui::MouseDownEvent, _, cx| {
                    if expandable && !layer.closing && layer.sheet_drag.is_none() {
                        layer.begin_sheet_drag(None, event.position.y, cx);
                    }
                    cx.stop_propagation();
                }),
            );

        if styled {
            panel.style().refine(&panel_style);
        }

        if expandable {
            let weak = cx.entity().downgrade();
            // Paint before the content so interactive/occluding descendants own their gestures.
            panel = panel.child(
                gpui::canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
                    move |_, hitbox, window, _| {
                        window.on_mouse_event(
                            move |event: &gpui::TouchEvent, phase, window, cx| {
                                let _ = weak.update(cx, |layer, cx| {
                                    layer.surface_touch(
                                        event,
                                        phase,
                                        hitbox.is_hovered(window),
                                        window,
                                        cx,
                                    );
                                });
                            },
                        );
                    },
                )
                .absolute()
                .inset_0(),
            );
        }

        if let ModalPlacement::Bottom {
            drag_to_dismiss: true,
            ..
        } = placement
        {
            let weak = cx.entity().downgrade();
            panel = panel.child(
                div()
                    .id("bottom-sheet-handle")
                    .relative()
                    .h(SHEET_HANDLE_HEIGHT)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|layer, event: &gpui::MouseDownEvent, _, cx| {
                            if layer.closing {
                                return;
                            }
                            layer.begin_sheet_drag(None, event.position.y, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .w(gpui::px(36.))
                            .h(gpui::px(4.))
                            .rounded_full()
                            .bg(gpui::rgba(0x80808066)),
                    )
                    .child(
                        gpui::canvas(
                            |bounds, window, _| {
                                window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal)
                            },
                            move |_, hitbox, window, _| {
                                window.on_mouse_event(
                                    move |event: &gpui::TouchEvent, phase, window, cx| {
                                        if !phase.bubble() {
                                            return;
                                        }
                                        let _ = weak.update(cx, |layer, cx| {
                                            if layer.closing || layer.sheet_surface_touch.is_some()
                                            {
                                                return;
                                            }
                                            if event.phase == gpui::TouchPhase::Started
                                                && hitbox.bounds.contains(&event.position)
                                                && layer.sheet_drag.is_none()
                                            {
                                                layer.begin_sheet_drag(
                                                    Some(event.id),
                                                    event.position.y,
                                                    cx,
                                                );
                                            }
                                            let Some((Some(id), start)) = layer.sheet_drag else {
                                                return;
                                            };
                                            if id != event.id {
                                                return;
                                            }
                                            window.prevent_default();
                                            cx.stop_propagation();
                                            match event.phase {
                                                gpui::TouchPhase::Moved => {
                                                    layer.update_sheet_drag(
                                                        event.position.y - start,
                                                        cx,
                                                    );
                                                    cx.notify();
                                                }
                                                gpui::TouchPhase::Ended
                                                | gpui::TouchPhase::Cancelled => {
                                                    layer.finish_sheet_drag(
                                                        event.phase == gpui::TouchPhase::Cancelled,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                                _ => {}
                                            }
                                        });
                                    },
                                );
                            },
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        }

        if title.is_some() || close_button.is_some() {
            let header = div()
                .relative()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .px(appearance.header_padding_x)
                .py(appearance.header_padding_y)
                .when(styled && appearance.section_borders, |this| {
                    this.border_b_1().border_color(appearance.section_border)
                })
                .children(title)
                .when_some(close_button, |this, button| {
                    this.child(
                        div()
                            .id("global-modal-close")
                            .cursor_pointer()
                            .on_click(cx.listener(|layer, _, window, cx| {
                                layer.dismiss(window, cx);
                                cx.stop_propagation();
                            }))
                            .child(button),
                    )
                });
            panel = panel.child(header);
        }

        let body = div()
            .id("global-modal-body")
            .when(expandable, |body| {
                body.flex().flex_col().flex_grow(1.).flex_shrink_1()
            })
            .when(!expandable, |body| body.flex_1())
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .when(styled, |this| {
                this.px(appearance.body_padding_x)
                    .py(appearance.body_padding_y)
            })
            .child(content);
        panel = panel.child(body);

        panel = match &modal.footer {
            ModalFooter::Hidden => panel,
            ModalFooter::Custom(footer) => panel.child(footer(window, cx)),
            ModalFooter::Default => {
                let cancel = match modal.cancel_button.as_ref() {
                    Some(button) => button(window, cx),
                    None => Self::default_button(
                        "global-modal-cancel-default",
                        (modal.cancel_text)(window, cx),
                        appearance.cancel_button,
                    ),
                };
                let ok = match modal.ok_button.as_ref() {
                    Some(button) => button(window, cx),
                    None => Self::default_button(
                        "global-modal-ok-default",
                        (modal.ok_text)(window, cx),
                        appearance.ok_button,
                    ),
                };
                let on_cancel = modal.on_cancel.clone();

                let footer = div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(appearance.footer_gap)
                    .px(appearance.footer_padding_x)
                    .py(appearance.footer_padding_y)
                    .when(styled && appearance.section_borders, |this| {
                        this.border_t_1().border_color(appearance.section_border)
                    })
                    .child(
                        div()
                            .id("global-modal-cancel")
                            .on_click(cx.listener(move |layer, _, window, cx| {
                                let should_close = on_cancel
                                    .as_ref()
                                    .is_none_or(|callback| callback(window, cx));
                                if should_close {
                                    layer.dismiss(window, cx);
                                }
                                cx.stop_propagation();
                            }))
                            .child(cancel),
                    )
                    .child(
                        div()
                            .id("global-modal-ok")
                            .on_click(cx.listener(move |layer, _, window, cx| {
                                layer.execute_ok(window, cx);
                                cx.stop_propagation();
                            }))
                            .child(ok),
                    );
                panel.child(footer)
            }
        };

        let insets = window.insets();
        let zero = gpui::px(0.);
        let ime_bottom = (insets.ime.bottom - insets.consumed.bottom).max(zero);
        if let ModalPlacement::Bottom {
            avoid_safe_area, ..
        } = placement
        {
            let bottom = if avoid_safe_area {
                (insets.safe_area.bottom - insets.consumed.bottom - ime_bottom).max(zero)
            } else {
                zero
            };
            panel = panel
                .when(!expandable, |panel| {
                    panel.top(self.sheet_offset + self.sheet_height * motion)
                })
                .child(div().flex_shrink_0().h(bottom));
        }

        let dismiss_fraction = if expandable {
            (self.sheet_offset.as_f32()
                - expansion * (self.sheet_max_height - self.sheet_collapsed_height).as_f32())
            .max(0.)
                / self.sheet_collapsed_height.as_f32().max(1.)
        } else {
            self.sheet_offset.as_f32().max(0.) / self.sheet_height.as_f32().max(1.)
        };
        let backdrop = div()
            .id("global-modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .track_focus(&self.focus_handle)
            .bg(appearance
                .backdrop
                .opacity((1. - motion - dismiss_fraction).clamp(0., 1.)))
            .occlude()
            .on_mouse_move(cx.listener(|layer, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some((None, start)) = layer.sheet_drag {
                    if event.pressed_button != Some(MouseButton::Left) {
                        layer.sheet_drag = None;
                        layer.sheet_offset = gpui::px(0.);
                    } else {
                        layer.update_sheet_drag(event.position.y - start, cx);
                        cx.stop_propagation();
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|layer, _, window, cx| {
                    if matches!(layer.sheet_drag, Some((None, _))) {
                        layer.finish_sheet_drag(window.default_prevented(), window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .capture_action(cx.listener(move |layer, _: &Submit, window, cx| {
                if ok_on_enter {
                    layer.execute_ok(window, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(move |layer, event: &KeyDownEvent, window, cx| {
                if close_on_escape && event.keystroke.key == "escape" {
                    layer.dismiss(window, cx);
                    cx.stop_propagation();
                } else if ok_on_enter
                    && event.keystroke.key == "enter"
                    && !window
                        .context_stack()
                        .iter()
                        .any(|context| context.contains("multiline"))
                {
                    layer.execute_ok(window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |layer, _, window, cx| {
                    if close_on_backdrop {
                        layer.dismiss(window, cx);
                    }
                    cx.stop_propagation();
                }),
            );

        let backdrop = match placement {
            ModalPlacement::Center => {
                let safe = insets.effective();
                backdrop
                    .items_center()
                    .pt(safe.top)
                    .pb(safe.bottom)
                    .pl(safe.left)
                    .pr(safe.right)
            }
            ModalPlacement::Top { offset } => backdrop.items_start().pt(offset),
            ModalPlacement::Bottom {
                avoid_safe_area, ..
            } => {
                let safe = insets.effective();
                backdrop
                    .items_end()
                    .overflow_hidden()
                    .pb(ime_bottom)
                    .when(avoid_safe_area, |this| {
                        this.pt(safe.top).pl(safe.left).pr(safe.right)
                    })
            }
        };

        let panel = if expandable {
            sheet_frame::SheetFrame {
                panel: panel.h_full().max_h(gpui::relative(1.)).into_any_element(),
                layer: cx.entity().downgrade(),
                expansion,
                drag_offset: self.sheet_offset,
                motion,
            }
            .into_any_element()
        } else {
            panel.into_any_element()
        };
        let backdrop = backdrop.child(panel).when(self.closing, |backdrop| {
            backdrop.child(
                div()
                    .id("modal-closing-input")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
            )
        });
        deferred(backdrop)
            .with_priority(MODAL_PRIORITY)
            .into_any_element()
    }
}

/// Marks content that keeps its own touch scrolling or dragging inside an expandable sheet.
/// Mount inside a positioned container; blank areas outside the region still resize the sheet.
pub fn sheet_gesture_exclusion() -> impl IntoElement {
    gpui::canvas(
        |bounds, window, cx| {
            if let Some(modal) = cx.try_global::<GlobalModal>() {
                let layer = modal.0.clone();
                layer.update(cx, |layer, _| {
                    layer
                        .sheet_exclusions
                        .push(bounds.intersect(&window.content_mask().bounds));
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

struct GlobalModal(Entity<ModalLayer>);

impl Global for GlobalModal {}

pub fn init(cx: &mut App) {
    init_with_appearance(ModalAppearance::default(), cx);
}

pub fn init_with_appearance(appearance: ModalAppearance, cx: &mut App) {
    if cx.has_global::<GlobalModal>() {
        set_appearance(appearance, cx);
    } else {
        let layer = cx.new(|cx| ModalLayer::new(appearance, cx));
        cx.set_global(GlobalModal(layer));
    }
}

pub fn set_appearance(appearance: ModalAppearance, cx: &mut App) {
    layer(cx).update(cx, |layer, cx| {
        layer.appearance = appearance;
        cx.notify();
    });
}

/// Mount this once as the last child of each window root.
pub fn layer(cx: &App) -> Entity<ModalLayer> {
    cx.global::<GlobalModal>().0.clone()
}

pub fn show(modal: Modal, window: &mut Window, cx: &mut App) {
    layer(cx).update(cx, |layer, cx| layer.show(modal, window, cx));
}

pub fn dismiss(window: &mut Window, cx: &mut App) {
    layer(cx).update(cx, |layer, cx| layer.dismiss(window, cx));
}

pub fn is_open(cx: &App) -> bool {
    layer(cx).read(cx).is_open()
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Entity, Focusable, IntoElement, Keystroke, Render, TestAppContext,
        VisualTestContext, Window, div, px, size,
    };

    use super::*;
    use crate::components::input::{Input, TextInput};

    struct ModalInputTest {
        input: Entity<TextInput>,
        modal_layer: Entity<ModalLayer>,
    }

    impl Render for ModalInputTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.modal_layer.clone())
        }
    }

    #[gpui::test]
    fn animated_sheet_retains_focus_until_exit_and_replacement_cancels_exit(
        cx: &mut TestAppContext,
    ) {
        use crate::components::bottom_sheet::BottomSheet;
        cx.update(|cx| {
            crate::components::input::init(cx);
            init(cx);
        });
        let window = cx.open_window(size(px(420.), px(640.)), |_, cx| ModalInputTest {
            input: cx.new(TextInput::new),
            modal_layer: layer(cx),
        });
        window
            .update(cx, |view, window, cx| {
                let focus = view.input.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
                BottomSheet::new(|_, _| div())
                    .animation_duration(Duration::from_millis(240))
                    .show(window, cx);
                dismiss(window, cx);
                assert!(is_open(cx));
                assert_ne!(window.focused(cx), Some(focus));
            })
            .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
        window
            .update(cx, |view, window, cx| {
                assert!(!is_open(cx));
                assert_eq!(
                    window.focused(cx),
                    Some(view.input.read(cx).focus_handle(cx))
                );
                BottomSheet::new(|_, _| div())
                    .animation_duration(Duration::from_millis(240))
                    .show(window, cx);
                dismiss(window, cx);
                show(Modal::new(|_, _| div()), window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        window.update(cx, |_, _, cx| assert!(is_open(cx))).unwrap();
    }

    #[gpui::test]
    fn sheet_fits_content_and_handle_switches_detents_before_dismissing(cx: &mut TestAppContext) {
        use crate::components::bottom_sheet::BottomSheet;
        use std::{cell::Cell, rc::Rc};
        cx.update(|cx| {
            crate::components::input::init(cx);
            init(cx);
        });
        let window = cx.open_window(size(px(420.), px(640.)), |_, cx| ModalInputTest {
            input: cx.new(TextInput::new),
            modal_layer: layer(cx),
        });
        let content_height = Rc::new(Cell::new(120.));
        let content = content_height.clone();
        window
            .update(cx, |_, window, cx| {
                BottomSheet::new(move |_, _| {
                    div()
                        .flex()
                        .flex_col()
                        .flex_grow(1.)
                        .min_h_0()
                        .child(
                            div()
                                .relative()
                                .h(px(80.))
                                .flex_shrink_0()
                                .child(sheet_gesture_exclusion()),
                        )
                        .child(div().h(px(content.get() - 80.)).min_h_0().flex_grow(1.))
                })
                .expandable(true)
                .animation_duration(Duration::from_millis(240))
                .show(window, cx);
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let settle = |visual: &mut VisualTestContext| {
            visual.cx.run_until_parked();
            visual
                .cx
                .executor()
                .advance_clock(Duration::from_millis(280));
            visual.update(|window, cx| {
                window.draw(cx).clear();
            });
        };
        settle(&mut visual);
        let height = visual.update(|_, cx| layer(cx).read(cx).sheet_height);
        assert!(height >= px(120.) && height < px(300.));
        content_height.set(200.);
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
        });
        let taller = visual.update(|_, cx| layer(cx).read(cx).sheet_height);
        assert!((taller - height - px(80.)).abs() < px(1.));
        let drag = |visual: &mut VisualTestContext, start: gpui::Pixels, delta: gpui::Pixels| {
            let origin = gpui::point(px(210.), start);
            let end = gpui::point(px(210.), start + delta);
            visual.simulate_mouse_down(origin, MouseButton::Left, Default::default());
            visual.update(|_, cx| {
                assert!(
                    layer(cx).read(cx).sheet_drag.is_some(),
                    "handle did not start a drag"
                )
            });
            visual.simulate_mouse_move(end, Some(MouseButton::Left), Default::default());
            visual.update(|_, cx| assert_eq!(layer(cx).read(cx).sheet_offset, delta));
            visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
            settle(visual);
        };
        let touch = |visual: &mut VisualTestContext, phase, position| {
            visual.update(|window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::Touch(gpui::TouchEvent {
                        id: gpui::TouchId(17),
                        phase,
                        position,
                        force: None,
                    }),
                    cx,
                );
            });
        };
        let protected = visual.update(|_, cx| layer(cx).read(cx).sheet_exclusions[0].center());
        touch(&mut visual, gpui::TouchPhase::Started, protected);
        touch(
            &mut visual,
            gpui::TouchPhase::Moved,
            protected - gpui::point(px(0.), px(90.)),
        );
        visual.update(|_, cx| assert!(layer(cx).read(cx).sheet_drag.is_none()));
        touch(&mut visual, gpui::TouchPhase::Ended, protected);
        let blank = gpui::point(px(210.), px(630.));
        touch(&mut visual, gpui::TouchPhase::Started, blank);
        touch(
            &mut visual,
            gpui::TouchPhase::Moved,
            blank - gpui::point(px(0.), px(100.)),
        );
        touch(
            &mut visual,
            gpui::TouchPhase::Ended,
            blank - gpui::point(px(0.), px(100.)),
        );
        settle(&mut visual);
        visual.update(|_, cx| {
            let sheet = layer(cx);
            let sheet = sheet.read(cx);
            assert!(sheet.sheet_expanded);
            assert!((sheet.sheet_height - px(640.)).abs() < px(1.));
        });
        drag(&mut visual, px(18.), px(100.));
        visual.update(|_, cx| {
            let sheet = layer(cx);
            let sheet = sheet.read(cx);
            assert!(sheet.is_open());
            assert!(!sheet.sheet_expanded);
            assert!((sheet.sheet_height - taller).abs() < px(1.));
        });
        drag(&mut visual, px(640.) - taller + px(18.), px(100.));
        visual.update(|_, cx| assert!(!is_open(cx)));
    }

    #[gpui::test]
    fn nested_sheet_scroll_preserves_drag_handoffs_and_fling_ownership(cx: &mut TestAppContext) {
        use crate::components::bottom_sheet::BottomSheet;
        use gpui::{ScrollHandle, TouchPhase, point};
        cx.update(|cx| {
            crate::components::input::init(cx);
            init(cx);
        });
        let window = cx.open_window(size(px(420.), px(640.)), |_, cx| ModalInputTest {
            input: cx.new(TextInput::new),
            modal_layer: layer(cx),
        });
        let scroll = ScrollHandle::new();
        let content_scroll = scroll.clone();
        window
            .update(cx, |_, window, cx| {
                BottomSheet::new(move |_, _| {
                    div()
                        .flex()
                        .flex_col()
                        .flex_grow(1.)
                        .min_h_0()
                        .child(div().h(px(76.)).flex_shrink_0())
                        .child(
                            div()
                                .h(px(960.))
                                .flex_grow(1.)
                                .min_h_0()
                                .child(SheetScrollArea::new(
                                    content_scroll.clone(),
                                    div()
                                        .id("nested-scroll")
                                        .size_full()
                                        .overflow_y_scroll()
                                        .track_scroll(&content_scroll)
                                        .children(
                                            (0..15).map(|_| div().h(px(64.)).flex_shrink_0()),
                                        ),
                                )),
                        )
                        .child(div().h(px(52.)).flex_shrink_0())
                })
                .expandable(true)
                .animation_duration(Duration::from_millis(240))
                .show(window, cx);
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(300));
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let touch = |visual: &mut VisualTestContext, phase, y: f32, elapsed| {
            visual
                .cx
                .executor()
                .advance_clock(Duration::from_millis(elapsed));
            visual.update(|window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::Touch(gpui::TouchEvent {
                        id: gpui::TouchId(93),
                        phase,
                        position: point(px(210.), px(y)),
                        force: None,
                    }),
                    cx,
                );
                window.draw(cx).clear();
            });
        };
        let collapsed = visual.update(|_, cx| layer(cx).read(cx).sheet_height);
        // A sheet-owned flick must not route excess movement or release velocity into content.
        scroll.set_offset(point(px(0.), -scroll.max_offset().y));
        let pinned = scroll.offset();
        touch(&mut visual, TouchPhase::Started, 510., 0);
        touch(&mut visual, TouchPhase::Moved, 498., 90);
        touch(&mut visual, TouchPhase::Moved, 175., 20);
        assert_eq!(scroll.offset(), pinned);
        touch(&mut visual, TouchPhase::Ended, 175., 0);
        for _ in 0..8 {
            visual
                .cx
                .executor()
                .advance_clock(Duration::from_millis(50));
            visual.update(|window, cx| {
                window.draw(cx).clear();
                assert!(layer(cx).read(cx).scroll_motion.is_none());
            });
            assert_eq!(scroll.offset(), pinned);
        }
        visual.update(|window, cx| {
            layer(cx).update(cx, |sheet, _| {
                sheet.release_scroll_hold();
                sheet.sheet_expanded = false;
                sheet.resize_motion = None;
            });
            window.refresh();
            window.draw(cx).clear();
        });
        // A short flick stays in the viewport even when it reaches its far edge.
        scroll.set_offset(point(px(0.), -scroll.max_offset().y + px(24.)));
        touch(&mut visual, TouchPhase::Started, 510., 0);
        touch(&mut visual, TouchPhase::Moved, 420., 20);
        visual.update(|_, cx| {
            let entity = layer(cx);
            let sheet = entity.read(cx);
            assert_eq!(sheet.sheet_height, collapsed);
            assert!(sheet.edge_pull < -px(40.).as_f32());
            assert!(sheet.sheet_drag.is_none());
        });
        touch(&mut visual, TouchPhase::Ended, 420., 0);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(350));
        visual.update(|window, cx| {
            window.draw(cx).clear();
            assert!(
                layer(cx)
                    .read(cx)
                    .edge_visual(cx.background_executor().now())
                    .abs()
                    < 0.1
            );
        });
        // Resizing cannot clamp a scrolled viewport or start list inertia during detent settling.
        let pinned_offset = scroll.offset();
        touch(&mut visual, TouchPhase::Started, 510., 0);
        touch(&mut visual, TouchPhase::Moved, 458., 110);
        assert_eq!(scroll.offset(), pinned_offset);
        visual.update(|_, cx| assert!(layer(cx).read(cx).sheet_height > collapsed));
        touch(&mut visual, TouchPhase::Ended, 458., 0);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(90));
        visual.update(|window, cx| {
            window.draw(cx).clear();
            let entity = layer(cx);
            let sheet = entity.read(cx);
            assert!(sheet.scroll_motion.is_none());
            assert!(sheet.sheet_height > collapsed && sheet.sheet_height < px(640.));
        });
        assert_eq!(scroll.offset(), pinned_offset);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(300));
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(300));
        visual.update(|window, cx| {
            window.draw(cx).clear();
            assert_eq!(layer(cx).read(cx).sheet_height, px(640.));
            layer(cx).update(cx, |sheet, _| {
                sheet.release_scroll_hold();
                sheet.sheet_expanded = false;
                sheet.resize_motion = None;
            });
            scroll.set_offset(point(px(0.), px(0.)));
            window.refresh();
            window.draw(cx).clear();
        });
        visual.update(|window, cx| {
            layer(cx).update(cx, |sheet, _| {
                sheet.sheet_expanded = true;
                sheet.resize_motion = None;
            });
            scroll.set_offset(point(px(0.), px(0.)));
            window.refresh();
            window.draw(cx).clear();
        });
        touch(&mut visual, TouchPhase::Started, 510., 0);
        touch(&mut visual, TouchPhase::Moved, 460., 110);
        touch(&mut visual, TouchPhase::Ended, 460., 0);
        let before_inertia = scroll.offset().y;
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(16));
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert!(scroll.offset().y < before_inertia);
        visual.update(|window, cx| {
            layer(cx).update(cx, |sheet, _| {
                sheet.scroll_motion = None;
                sheet.release_scroll_hold();
                sheet.sheet_expanded = false;
                sheet.resize_motion = None;
            });
            scroll.set_offset(point(px(0.), px(0.)));
            window.refresh();
            window.draw(cx).clear();
        });
        scroll.set_offset(point(px(0.), px(0.)));
        // A sustained drag crosses the expansion and scroll boundaries without a new touch.
        touch(&mut visual, TouchPhase::Started, 510., 0);
        for y in [458., 406., 354., 302., 250.] {
            touch(&mut visual, TouchPhase::Moved, y, 110);
            let height = visual.update(|_, cx| layer(cx).read(cx).sheet_height);
            if height < px(640.) {
                assert_eq!(scroll.offset().y, px(0.));
            }
        }
        visual.update(|_, cx| assert_eq!(layer(cx).read(cx).sheet_height, px(640.)));
        assert!(scroll.offset().y < px(-60.));
        let before_scroll = scroll.offset().y;
        touch(&mut visual, TouchPhase::Moved, 290., 110);
        assert!((scroll.offset().y - before_scroll - px(40.)).abs() < px(0.1));
        visual.update(|_, cx| assert_eq!(layer(cx).read(cx).sheet_height, px(640.)));
        touch(&mut visual, TouchPhase::Moved, 400., 220);
        assert_eq!(scroll.offset().y, px(0.));
        visual.update(|_, cx| {
            let height = layer(cx).read(cx).sheet_height;
            assert!(height > collapsed && height < px(640.));
        });
        touch(&mut visual, TouchPhase::Moved, 650., 300);
        visual.update(|_, cx| assert_eq!(layer(cx).read(cx).sheet_height, collapsed));
        // Reversing after crossing the dismissal threshold must keep the sheet attached.
        touch(&mut visual, TouchPhase::Moved, 600., 20);
        visual.update(|_, cx| {
            let entity = layer(cx);
            let sheet = entity.read(cx);
            assert!(sheet.sheet_offset >= px(72.));
            assert!(!sheet.closing);
        });
        touch(&mut visual, TouchPhase::Ended, 600., 0);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(300));
        visual.update(|window, cx| {
            window.draw(cx).clear();
            assert!(is_open(cx));
            assert_eq!(layer(cx).read(cx).sheet_height, collapsed);
        });
        touch(&mut visual, TouchPhase::Started, 510., 0);
        touch(&mut visual, TouchPhase::Moved, 630., 220);
        touch(&mut visual, TouchPhase::Ended, 630., 0);
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(300));
        visual.cx.run_until_parked();
        visual.update(|_, cx| assert!(!is_open(cx)));
    }

    #[gpui::test]
    fn multiline_enter_does_not_accept_the_modal(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::components::input::init(cx);
            init(cx);
        });
        let window = cx.open_window(size(px(420.), px(320.)), |_, cx| ModalInputTest {
            input: cx.new(|cx| TextInput::new(cx).multiline()),
            modal_layer: layer(cx),
        });
        window
            .update(cx, |view, window, cx| {
                let input = view.input.clone();
                show(Modal::new(move |_, _| Input::new(&input)), window, cx);
            })
            .unwrap();

        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        window
            .update(&mut visual.cx, |view, window, cx| {
                let focus_handle = view.input.read(cx).focus_handle(cx);
                window.focus(&focus_handle, cx);
            })
            .unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        visual.update(|window, cx| {
            window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
        });

        window
            .update(&mut visual.cx, |view, _, cx| {
                assert!(is_open(cx));
                assert_eq!(view.input.read(cx).value().as_ref(), "\n");
            })
            .unwrap();
    }
}
