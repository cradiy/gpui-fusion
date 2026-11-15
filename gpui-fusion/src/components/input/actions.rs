use gpui::{App, KeyBinding, actions};

actions!(
    fusion_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Undo,
        Redo,
        Paste,
        Cut,
        Copy,
        InsertNewline,
        Submit,
    ]
);

pub fn init(cx: &mut App) {
    if cx.has_global::<Initialized>() {
        return;
    }
    cx.set_global(Initialized);
    cx.bind_keys([
        KeyBinding::new("secondary-z", Undo, Some("InputState")),
        KeyBinding::new("secondary-shift-z", Redo, Some("InputState")),
        KeyBinding::new("secondary-y", Redo, Some("InputState")),
        KeyBinding::new("backspace", Backspace, Some("InputState")),
        KeyBinding::new("delete", Delete, Some("InputState")),
        KeyBinding::new("left", Left, Some("InputState")),
        KeyBinding::new("right", Right, Some("InputState")),
        KeyBinding::new("up", Up, Some("InputState && multiline")),
        KeyBinding::new("down", Down, Some("InputState && multiline")),
        KeyBinding::new("shift-left", SelectLeft, Some("InputState")),
        KeyBinding::new("shift-right", SelectRight, Some("InputState")),
        KeyBinding::new("shift-up", SelectUp, Some("InputState && multiline")),
        KeyBinding::new("shift-down", SelectDown, Some("InputState && multiline")),
        KeyBinding::new("secondary-a", SelectAll, Some("InputState")),
        KeyBinding::new("secondary-v", Paste, Some("InputState")),
        KeyBinding::new("secondary-c", Copy, Some("InputState")),
        KeyBinding::new("secondary-x", Cut, Some("InputState")),
        KeyBinding::new("home", Home, Some("InputState")),
        KeyBinding::new("end", End, Some("InputState")),
        KeyBinding::new("enter", Submit, Some("InputState && !multiline")),
        KeyBinding::new("enter", InsertNewline, Some("InputState && multiline")),
        KeyBinding::new("secondary-enter", Submit, Some("InputState && multiline")),
    ]);
}

struct Initialized;
impl gpui::Global for Initialized {}
