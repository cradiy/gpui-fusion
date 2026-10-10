# Command palette

`CommandPalette` searches a collection of commands and emits the selected command
to the application. Use it inside a page or open it through the shared modal layer.
Initialize UIC with `uic::init(cx)` and retain the state and event subscription in
your view.

```rust
use uic::components::command_palette::{
    CommandItem, CommandPalette, CommandPaletteEvent, CommandPaletteState,
};

let commands = cx.new(|cx| CommandPaletteState::new(vec![
    CommandItem::new("new-document", "New document")
        .group("Workspace")
        .description("Start with a clean page")
        .keywords("create write")
        .shortcut("Ctrl N"),
    CommandItem::new("settings", "Open settings").group("Preferences"),
], window, cx));

let subscription = cx.subscribe(&commands, |this, _, event, cx| {
    if let CommandPaletteEvent::Invoked(command) = event {
        this.execute_command(&command.id, cx);
    }
});

// Inside Render:
CommandPalette::new(&self.commands)
    .label("Workspace commands")
    .placeholder("Find an action")
    .w_full();
```

IDs must be unique. An item can provide a description, search keywords, a group,
an SVG asset path through `.icon(path)`, a shortcut hint and a disabled flag.
Icon paths resolve through the application's asset source. Shortcut hints are
display text; the application registers its own actions and key bindings.

## Search and execution

Local search is case-insensitive and requires every whitespace-separated query
word to match the label, description, keywords or group. Groups follow the order
of their first matching item, with item order preserved within each group.

The first enabled result is highlighted. Up/Down move between enabled results,
stopping at either end. Enter or a row click emits `Invoked(CommandItem)` without
replacing the query. Empty results, disabled items and loading states cannot be
invoked. IME pre-edit text does not emit `QueryChanged`, and composition keystrokes
do not navigate, invoke or dismiss the palette.

`QueryChanged` carries committed search text. `set_query(text, cx)` changes the
input and emits this event only when the query changes. `input()` exposes the
underlying `TextInput` for additional configuration; keep it single-line.

## Keyboard shortcuts

Use `.key_binding(shortcut, action)` to add or override a key combination for this
palette. The default bindings are Up/Down, Enter and Escape. The last matching
binding takes precedence, including over a default binding.

```rust
use uic::components::command_palette::CommandPaletteAction;

CommandPalette::new(&self.commands)
    .key_binding("ctrl-p", CommandPaletteAction::Previous)
    .key_binding("ctrl-n", CommandPaletteAction::Next)
    .key_binding("ctrl-enter", CommandPaletteAction::Confirm)
    .key_binding("alt-escape", CommandPaletteAction::Dismiss);
```

Bindings use GPUI key syntax and accept one combination, such as `ctrl-n` or
`secondary-j`, rather than a multi-step sequence. They apply while focus is inside
the palette, in both embedded and dialog presentation, and are ignored during IME
composition. Unbound keys retain normal input behavior. A custom Tab binding takes
precedence over the dialog's default Tab focus handling.

## Dialog presentation

Mount `modal::layer(cx)` as the last child of the window root. Open the palette
from a click or your application's shortcut handler:

```rust
CommandPalette::new(&self.commands)
    .w(px(600.))
    .h(px(440.))
    .shadow_lg()
    .show(window, cx);
```

The dialog focuses search and retains its current query. Call `set_query("", cx)`
before opening when you want a fresh search. It fits within the available viewport
and keyboard insets. Confirming closes the dialog and restores prior focus before
emitting `Invoked`, allowing the application to open another dialog from that event.
Escape and the close control emit `Dismissed`; backdrop clicks use the modal layer's
normal dismissal. Tab/Shift-Tab retain search focus while presented as a dialog.

### Placement

The default dialog is horizontally centered, with its top 24px below the top safe
area. `.placement(...)` uses the shared `ModalPlacement` settings:

```rust
use uic::components::modal::ModalPlacement;

CommandPalette::new(&self.commands)
    .placement(ModalPlacement::Center)
    .show(window, cx);

CommandPalette::new(&self.commands)
    .placement(ModalPlacement::Top { offset: px(80.) })
    .show(window, cx);

CommandPalette::new(&self.commands)
    .placement(ModalPlacement::Bottom {
        avoid_safe_area: true,
        drag_to_dismiss: false,
    })
    .show(window, cx);
```

`Center` centers within the available area after safe-area and keyboard insets.
An explicit `Top` offset is measured from the viewport top; include the safe-area
inset yourself if needed. `Bottom` follows the modal's bottom placement and optional
drag handle. All positions limit the palette height to available space, including
space reserved for the drag handle. Placement affects only `.show(...)`; embedded
palettes follow their parent layout. Custom surface materials use the same placement.

Embedded palettes stay mounted after confirmation and let Tab move focus normally.
They emit `Dismissed` on Escape so the application can decide what to hide. Use a
separate state for each mounted presentation in each window.

## Application-provided results

Create the state with `.filter(false)` to use results already filtered or ranked
by the application. Set `set_loading(true, cx)` while fetching, replace commands
with `set_items(items, cx)`, then call `set_loading(false, cx)`.

Replacing commands preserves the active ID if still enabled and visible; otherwise
the first enabled result becomes active. Loading clears selection. The application
owns debouncing, request cancellation and stale-response checks. Results render
eagerly, so provide a bounded collection.

## Appearance

Styled properties control the whole palette surface, including dimensions, padding,
background, borders, corner radius and typography. `CommandPaletteAppearance`
contains input semantic colors, the active row background, muted text, divider
color and disabled opacity.

Use `.render_item(...)` to customize row content; the palette retains navigation
and activation. The closure receives the item, `CommandItemState` (`highlighted`
and `disabled`), window and application context. `.empty_text(...)` and
`.loading_text(...)` customize status messages.

## Surface materials

Use `.surface(...)` to replace the outer container with a glass material or another
styled element. The callback receives the search and command-list content; mount
that content exactly once in the returned container.

```rust
use gpui::{prelude::*, px};
use gpui_effects::{FrostedGlass, FrostedGlassAppearance};

CommandPalette::new(&self.commands)
    .w(px(600.))
    .h(px(420.))
    .rounded(px(20.))
    .border_0()
    .shadow_lg()
    .surface(|content, _, _| {
        FrostedGlass::with_appearance(FrostedGlassAppearance::light())
            .child(content)
    });
```

The returned element must implement `Styled` and `IntoElement`. Palette styles
are applied to that element, including dimensions, padding, typography, corner
radii, borders and shadows. Set these properties on `CommandPalette`; its styles
take precedence over the container's styles. The container uses a clipped column
layout, and dialog viewport limits also apply to it.

Custom surfaces do not receive the plain palette's white background. An explicit
`.bg(...)` still applies to the surface, so omit opaque fills when using backdrop
materials. For liquid glass, return
`LiquidGlass::with_appearance(LiquidGlassAppearance::regular()).child(content)`.
The material paints before the content, keeping text and controls sharp. Backdrop
effects sample previously painted content; place visual content behind the palette
to make the effect visible.

The same surface callback is used for embedded palettes and `.show(...)`. Custom
materials retain search, keyboard navigation, row activation and dismissal.

```sh
cargo run -p uic --example command_palette
```
