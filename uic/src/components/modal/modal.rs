use std::rc::Rc;

use gpui::{
    AnyElement, App, Entity, IntoElement, Pixels, Render, StyleRefinement, Styled, Window, hsla,
    px, relative,
};

use super::ModalAppearance;

#[derive(Clone, Copy, Debug, Default)]
pub enum ModalPlacement {
    #[default]
    Center,
    Top {
        offset: Pixels,
    },
    Bottom {
        avoid_safe_area: bool,
        drag_to_dismiss: bool,
    },
}

pub(crate) type ModalCallback = Rc<dyn Fn(&mut Window, &mut App) -> bool>;
pub(crate) type ModalSlot = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

pub(crate) enum ModalFooter {
    Default,
    Custom(ModalSlot),
    Hidden,
}

/// Description of a modal shown by the global [`super::ModalLayer`].
pub struct Modal {
    pub(crate) content: ModalSlot,
    pub(crate) title: Option<ModalSlot>,
    pub(crate) close_button: Option<ModalSlot>,
    pub(crate) footer: ModalFooter,
    pub(crate) ok_button: Option<ModalSlot>,
    pub(crate) cancel_button: Option<ModalSlot>,
    pub(crate) ok_text: ModalSlot,
    pub(crate) cancel_text: ModalSlot,
    pub(crate) on_ok: Option<ModalCallback>,
    pub(crate) on_cancel: Option<ModalCallback>,
    pub(crate) appearance: Option<ModalAppearance>,
    pub(crate) placement: ModalPlacement,
    pub(crate) close_on_escape: bool,
    pub(crate) close_on_backdrop: bool,
    pub(crate) ok_on_enter: bool,
    pub(crate) styled: bool,
    pub(crate) style: StyleRefinement,
}

impl Modal {
    fn slot<E: IntoElement>(render: impl Fn(&mut Window, &mut App) -> E + 'static) -> ModalSlot {
        Rc::new(move |window, cx| render(window, cx).into_any_element())
    }

    /// Creates a modal from a render closure. The closure is called on every redraw.
    pub fn new<E: IntoElement>(content: impl Fn(&mut Window, &mut App) -> E + 'static) -> Self {
        Self {
            content: Self::slot(content),
            title: None,
            close_button: None,
            footer: ModalFooter::Default,
            ok_button: None,
            cancel_button: None,
            ok_text: Self::slot(|_, _| "OK"),
            cancel_text: Self::slot(|_, _| "Cancel"),
            on_ok: None,
            on_cancel: None,
            appearance: None,
            placement: ModalPlacement::Center,
            close_on_escape: true,
            close_on_backdrop: true,
            ok_on_enter: true,
            styled: true,
            style: StyleRefinement::default()
                .w(px(520.))
                .max_w(relative(0.9))
                .max_h(relative(0.85))
                .rounded(px(12.))
                .border_1()
                .border_color(hsla(0., 0., 0., 0.12))
                .bg(hsla(0., 0., 1., 1.))
                .text_color(hsla(0., 0., 0.08, 1.))
                .shadow_lg(),
        }
    }

    /// Creates a modal backed by an existing stateful GPUI view.
    pub fn view<V: Render>(content: Entity<V>) -> Self {
        Self::new(move |_, _| content.clone())
    }

    pub fn title<E: IntoElement>(
        mut self,
        title: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.title = Some(Self::slot(title));
        self
    }

    pub fn title_text(self, title: impl Into<gpui::SharedString>) -> Self {
        let title = title.into();
        self.title(move |_, _| title.clone())
    }

    pub fn title_view<V: Render>(self, title: Entity<V>) -> Self {
        self.title(move |_, _| title.clone())
    }

    /// Supplies the visual content of the top-right close control.
    /// No close control is rendered unless this is called.
    pub fn close_button<E: IntoElement>(
        mut self,
        button: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.close_button = Some(Self::slot(button));
        self
    }

    pub fn close_button_view<V: Render>(self, button: Entity<V>) -> Self {
        self.close_button(move |_, _| button.clone())
    }

    pub fn footer<E: IntoElement>(
        mut self,
        footer: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.footer = ModalFooter::Custom(Self::slot(footer));
        self
    }

    pub fn footer_view<V: Render>(self, footer: Entity<V>) -> Self {
        self.footer(move |_, _| footer.clone())
    }

    pub fn hide_footer(mut self) -> Self {
        self.footer = ModalFooter::Hidden;
        self
    }

    /// Replaces the visual content of the default OK control.
    pub fn ok_button<E: IntoElement>(
        mut self,
        button: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.ok_button = Some(Self::slot(button));
        self
    }

    pub fn ok_button_view<V: Render>(self, button: Entity<V>) -> Self {
        self.ok_button(move |_, _| button.clone())
    }

    /// Replaces the visual content of the default Cancel control.
    pub fn cancel_button<E: IntoElement>(
        mut self,
        button: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.cancel_button = Some(Self::slot(button));
        self
    }

    pub fn cancel_button_view<V: Render>(self, button: Entity<V>) -> Self {
        self.cancel_button(move |_, _| button.clone())
    }

    /// Replaces the contents of the default styled OK button.
    pub fn ok_text<E: IntoElement>(
        mut self,
        text: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.ok_text = Self::slot(text);
        self
    }

    pub fn ok_label(self, text: impl Into<gpui::SharedString>) -> Self {
        let text = text.into();
        self.ok_text(move |_, _| text.clone())
    }

    /// Replaces the contents of the default styled Cancel button.
    pub fn cancel_text<E: IntoElement>(
        mut self,
        text: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.cancel_text = Self::slot(text);
        self
    }

    pub fn cancel_label(self, text: impl Into<gpui::SharedString>) -> Self {
        let text = text.into();
        self.cancel_text(move |_, _| text.clone())
    }

    /// The modal closes when the callback returns `true`.
    pub fn on_ok(mut self, callback: impl Fn(&mut Window, &mut App) -> bool + 'static) -> Self {
        self.on_ok = Some(Rc::new(callback));
        self
    }

    /// The modal closes when the callback returns `true`.
    pub fn on_cancel(mut self, callback: impl Fn(&mut Window, &mut App) -> bool + 'static) -> Self {
        self.on_cancel = Some(Rc::new(callback));
        self
    }

    pub fn appearance(mut self, appearance: ModalAppearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn placement(mut self, placement: ModalPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Runs the same action as the OK control when Enter is pressed.
    ///
    /// Enabled by default. If the OK callback returns `false`, the modal remains open.
    pub fn ok_on_enter(mut self, enabled: bool) -> Self {
        self.ok_on_enter = enabled;
        self
    }

    /// Leaves only positioning, backdrop, focus and dismissal to the modal host.
    pub fn unstyled(mut self) -> Self {
        self.styled = false;
        self
    }
}

impl Styled for Modal {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
