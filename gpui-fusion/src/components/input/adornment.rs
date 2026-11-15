use super::*;
use gpui_icons::GpuiIcons;

/// Placement of an adornment relative to editable text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSide {
    Leading,
    Trailing,
}

type IconCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// An SVG adornment, optionally activated as a labeled button.
#[derive(Clone)]
pub struct InputIcon {
    path: SharedString,
    action: Option<(SharedString, IconCallback)>,
}
impl InputIcon {
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            action: None,
        }
    }
    /// Adds mouse, keyboard, and accessibility activation with an accessible label.
    pub fn on_click(
        mut self,
        label: impl Into<SharedString>,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(callback)));
        self
    }
}

#[derive(Clone, Copy)]
pub(super) enum InputAction {
    Visibility,
    Clear,
}
impl Input {
    pub fn clearable(mut self, enabled: bool) -> Self {
        self.clearable = enabled;
        self
    }
    /// Sets the prefix or suffix slot to a themed SVG icon.
    pub fn icon(mut self, side: InputSide, icon: InputIcon) -> Self {
        *match side {
            InputSide::Leading => &mut self.prefix,
            InputSide::Trailing => &mut self.suffix,
        } = Some(Adornment::Icon(icon));
        self
    }
    /// Sets an icon slot to a search action that submits committed input.
    pub fn search_icon(self, side: InputSide) -> Self {
        let editor = self.editor.downgrade();
        self.icon(
            side,
            InputIcon::new(GpuiIcons::LucideSearch).on_click("Search", move |_, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    if !editor.disabled && !editor.is_composing() {
                        cx.emit(super::super::InputEvent::Submit(editor.value()));
                    }
                });
            }),
        )
    }
    fn activate_adornment(
        &mut self,
        action: InputAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.read(cx).disabled {
            return;
        }
        match action {
            InputAction::Visibility => self.toggle_password(cx),
            InputAction::Clear => self.editor.update(cx, |editor, cx| {
                if !editor.read_only {
                    window.focus(&editor.focus_handle(cx), cx);
                    let end = editor.value().encode_utf16().count();
                    editor.replace_text_in_range(Some(0..end), "", window, cx);
                }
            }),
        }
    }
    pub(super) fn adornment(
        &self,
        action: InputAction,
        style: &ComputedStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editor = self.editor.read(cx);
        let (id, label, path, focus) = match action {
            InputAction::Visibility => (
                "fusion-input-visibility",
                if editor.is_password_visible() {
                    "Hide password"
                } else {
                    "Show password"
                },
                if editor.is_password_visible() {
                    GpuiIcons::LucideEyeOff
                } else {
                    GpuiIcons::LucideEye
                },
                &self.visibility_focus,
            ),
            InputAction::Clear => (
                "fusion-input-clear",
                "Clear input",
                GpuiIcons::LucideX,
                &self.clear_focus,
            ),
        };
        let weak = cx.weak_entity();
        let icon = InputIcon::new(path).on_click(label, move |window, cx| {
            let _ = weak.update(cx, |this, cx| this.activate_adornment(action, window, cx));
        });
        self.render_icon(&icon, id, focus, style, cx)
    }
    pub(super) fn render_adornment(
        &self,
        adornment: &Adornment,
        side: InputSide,
        style: &ComputedStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match adornment {
            Adornment::Custom(render) => render(window, cx),
            Adornment::Icon(icon) => {
                let (id, focus) = match side {
                    InputSide::Leading => ("fusion-input-leading-icon", &self.leading_focus),
                    InputSide::Trailing => ("fusion-input-trailing-icon", &self.trailing_focus),
                };
                self.render_icon(icon, id, focus, style, cx)
            }
        }
    }
    fn render_icon(
        &self,
        icon: &InputIcon,
        id: &'static str,
        focus: &FocusHandle,
        style: &ComputedStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.editor.read(cx).disabled;
        let focus_color = color(&style.border_color);
        let button = div()
            .id(id)
            .debug_selector(move || id.into())
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .w(length(&style.width, px(24.)))
            .h(length(&style.height, px(24.)))
            .rounded(length(&style.radius, px(4.)))
            .border_1()
            .border_color(transparent_black())
            .child(
                svg()
                    .path(icon.path.clone())
                    .size(length(&style.font_size, px(18.)))
                    .text_color(color(&style.foreground)),
            );
        let Some((label, callback)) = &icon.action else {
            return button.into_any_element();
        };
        let mut button = button
            .track_focus(focus)
            .tab_stop(!disabled)
            .role(Role::Button)
            .aria_label(label.clone())
            .aria_disabled(disabled)
            .focus_visible(move |s| s.border_color(focus_color));
        if !disabled {
            let click = callback.clone();
            let accessible_click = callback.clone();
            button = button
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        window.focus(&this.editor.focus_handle(cx), cx);
                        cx.stop_propagation();
                    }),
                )
                .on_click(move |_, window, cx| {
                    click(window, cx);
                    cx.stop_propagation();
                })
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    accessible_click(window, cx);
                });
        }
        button.into_any_element()
    }
}
