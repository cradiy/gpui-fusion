mod appearance;
mod modal;

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
    sheet_offset: gpui::Pixels,
}

impl ModalLayer {
    fn new(appearance: ModalAppearance, cx: &mut Context<Self>) -> Self {
        Self {
            active: None,
            focus_handle: cx.focus_handle(),
            appearance,
            sheet_drag: None,
            sheet_offset: gpui::px(0.),
        }
    }

    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    fn show(&mut self, modal: Modal, window: &mut Window, cx: &mut Context<Self>) {
        self.sheet_drag = None;
        self.sheet_offset = gpui::px(0.);
        self.active = Some(ActiveModal {
            modal,
            window_id: window.window_handle().window_id(),
            previous_focus: window.focused(cx),
        });
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.sheet_drag = None;
        self.sheet_offset = gpui::px(0.);
        cx.notify();
    }

    fn finish_sheet_drag(&mut self, cancelled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sheet_drag.take().is_none() {
            return;
        }
        if !cancelled && self.sheet_offset >= gpui::px(72.) {
            self.dismiss(window, cx);
        } else {
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
        let Some(active) = self.active.as_ref() else {
            return div().into_any_element();
        };
        if active.window_id != window.window_handle().window_id() {
            return div().into_any_element();
        }

        let modal = &active.modal;
        let appearance = modal.appearance.unwrap_or(self.appearance);
        let close_on_escape = modal.close_on_escape;
        let close_on_backdrop = modal.close_on_backdrop;
        let ok_on_enter = modal.ok_on_enter && matches!(&modal.footer, ModalFooter::Default);
        let styled = modal.styled;
        let placement = modal.placement;
        let panel_style = modal.style.clone();
        let title = modal.title.as_ref().map(|slot| slot(window, cx));
        let close_button = modal.close_button.as_ref().map(|slot| slot(window, cx));
        let content = (modal.content)(window, cx);

        let mut panel = div()
            .id("global-modal-content")
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

        if styled {
            panel.style().refine(&panel_style);
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
                            layer.sheet_drag = Some((None, event.position.y));
                            layer.sheet_offset = gpui::px(0.);
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
                                            if event.phase == gpui::TouchPhase::Started
                                                && hitbox.bounds.contains(&event.position)
                                                && layer.sheet_drag.is_none()
                                            {
                                                layer.sheet_drag =
                                                    Some((Some(event.id), event.position.y));
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
                                                    layer.sheet_offset = (event.position.y - start)
                                                        .max(gpui::px(0.));
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
            .flex_1()
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
                .top(self.sheet_offset)
                .child(div().flex_shrink_0().h(bottom));
        }

        let backdrop = div()
            .id("global-modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .track_focus(&self.focus_handle)
            .bg(appearance.backdrop)
            .occlude()
            .on_mouse_move(cx.listener(|layer, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some((None, start)) = layer.sheet_drag {
                    if event.pressed_button != Some(MouseButton::Left) {
                        layer.sheet_drag = None;
                        layer.sheet_offset = gpui::px(0.);
                    } else {
                        layer.sheet_offset = (event.position.y - start).max(gpui::px(0.));
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

        deferred(backdrop.child(panel))
            .with_priority(MODAL_PRIORITY)
            .into_any_element()
    }
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
