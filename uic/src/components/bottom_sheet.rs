use gpui::{App, Entity, IntoElement, Render, StyleRefinement, Styled, Window, px, relative};

use super::modal::{self, Modal, ModalPlacement};

/// A bottom-aligned modal with a drag handle and optional safe-area avoidance.
/// Mount `modal::layer` once in the window root before showing sheets.
pub struct BottomSheet {
    modal: Modal,
    avoid_safe_area: bool,
    drag_to_dismiss: bool,
}

impl BottomSheet {
    pub fn new<E: IntoElement>(content: impl Fn(&mut Window, &mut App) -> E + 'static) -> Self {
        Self {
            modal: Modal::new(content)
                .hide_footer()
                .ok_on_enter(false)
                .w_full()
                .max_w(px(640.))
                .h(px(360.))
                .max_h(relative(0.9))
                .rounded_t(px(24.))
                .rounded_b(px(0.)),
            avoid_safe_area: true,
            drag_to_dismiss: true,
        }
    }

    pub fn view<V: Render>(content: Entity<V>) -> Self {
        Self::new(move |_, _| content.clone())
    }

    /// Avoid system bars and display cutouts. Keyboard avoidance remains enabled.
    pub fn avoid_safe_area(mut self, enabled: bool) -> Self {
        self.avoid_safe_area = enabled;
        self
    }

    /// Allow downward dragging from the handle to dismiss the sheet.
    pub fn drag_to_dismiss(mut self, enabled: bool) -> Self {
        self.drag_to_dismiss = enabled;
        self
    }

    pub fn title(mut self, title: impl Into<gpui::SharedString>) -> Self {
        self.modal = self.modal.title_text(title);
        self
    }

    pub fn close_on_backdrop(mut self, enabled: bool) -> Self {
        self.modal = self.modal.close_on_backdrop(enabled);
        self
    }

    pub fn close_on_escape(mut self, enabled: bool) -> Self {
        self.modal = self.modal.close_on_escape(enabled);
        self
    }

    pub fn show(self, window: &mut Window, cx: &mut App) {
        modal::show(self.into(), window, cx);
    }
}

impl Styled for BottomSheet {
    fn style(&mut self) -> &mut StyleRefinement {
        self.modal.style()
    }
}

impl From<BottomSheet> for Modal {
    fn from(sheet: BottomSheet) -> Self {
        sheet.modal.placement(ModalPlacement::Bottom {
            avoid_safe_area: sheet.avoid_safe_area,
            drag_to_dismiss: sheet.drag_to_dismiss,
        })
    }
}
