macro_rules! input_appearance {
    ($ty:ty) => {
        impl $ty {
            pub fn placeholder(mut self, placeholder: gpui::Hsla) -> Self {
                self.appearance.placeholder = placeholder;
                self
            }

            pub fn focus_border(mut self, focus_border: gpui::Hsla) -> Self {
                self.appearance.focus_border = focus_border;
                self
            }

            pub fn caret(mut self, caret: gpui::Hsla) -> Self {
                self.appearance.caret = caret;
                self
            }

            pub fn selection(mut self, selection: gpui::Hsla) -> Self {
                self.appearance.selection = selection;
                self
            }

            pub fn caret_width(mut self, caret_width: gpui::Pixels) -> Self {
                self.appearance.caret_width = caret_width;
                self
            }

            pub fn caret_height(mut self, caret_height: gpui::Pixels) -> Self {
                self.appearance.caret_height = caret_height;
                self
            }

            /// Sets the number of visible rows for a multi-line input.
            ///
            /// An explicit Styled height overrides the derived row height.
            pub fn rows(mut self, rows: usize) -> Self {
                self.rows = Some(rows.max(1));
                self
            }
        }
    };
}

mod actions;
mod appearance;
mod element;
#[allow(clippy::module_inception)]
mod input;
mod read_only;
mod state;

use gpui::SharedString;

pub(crate) fn row_height(
    style: &gpui::StyleRefinement,
    rows: usize,
    rem_size: gpui::Pixels,
) -> gpui::Pixels {
    let font_size = style
        .text
        .font_size
        .unwrap_or_else(|| gpui::px(16.).into())
        .to_pixels(rem_size);
    let line_height = style
        .text
        .line_height
        .unwrap_or_else(|| gpui::px(24.).into())
        .to_pixels(font_size.into(), rem_size);
    let padding_top = style
        .padding
        .top
        .unwrap_or_else(|| gpui::px(10.).into())
        .to_pixels(font_size.into(), rem_size);
    let padding_bottom = style
        .padding
        .bottom
        .unwrap_or_else(|| gpui::px(10.).into())
        .to_pixels(font_size.into(), rem_size);
    let border_top = style
        .border_widths
        .top
        .unwrap_or_else(|| gpui::px(1.).into())
        .to_pixels(rem_size);
    let border_bottom = style
        .border_widths
        .bottom
        .unwrap_or_else(|| gpui::px(1.).into())
        .to_pixels(rem_size);
    line_height * rows.max(1) as f32 + padding_top + padding_bottom + border_top + border_bottom
}

pub(crate) use actions::Submit;
pub use actions::init;
pub use appearance::InputAppearance;
pub use input::Input;
pub use read_only::ReadOnlyInput;
pub use state::TextInput;

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
    /// IME pre-edit updates are rendered by [`TextInput`] but do not emit this
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
