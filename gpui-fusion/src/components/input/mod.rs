mod accessibility;
mod actions;
mod element;
mod field;
mod history;
mod resize;
pub use resize::InputResize;
mod scrollbar;
mod state;

pub use actions::init;
pub use field::{Input, InputIcon, InputPart, InputSide};
use gpui::SharedString;
pub use state::InputState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    /// A single-line plain-text field. Enter emits [`InputEvent::Submit`].
    Text,
    /// A single-line field that masks its displayed value.
    Password,
    /// A soft-wrapping multi-line editor. Enter inserts a newline and Ctrl/Cmd+Enter submits.
    Multiline,
}

/// A configured software keyboard action, separate from Enter's [`InputEvent::Submit`].
#[derive(Clone, Debug)]
pub struct InputActionEvent {
    /// The configured software keyboard action.
    pub action: gpui::TextInputAction,
    /// The committed value at the time of the action.
    pub text: SharedString,
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    /// The committed value changed.
    ///
    /// IME pre-edit updates are rendered by [`InputState`] but do not emit this
    /// event until the composition is committed.
    Change(SharedString),
    Submit(SharedString),
}

impl InputEvent {
    pub fn text(&self) -> &SharedString {
        match self {
            InputEvent::Change(shared_string) => shared_string,
            InputEvent::Submit(shared_string) => shared_string,
        }
    }
}
