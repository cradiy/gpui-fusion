# Autocomplete

`Autocomplete` combines a free-text `TextInput` with suggestions. It uses
`SelectOption` for stable IDs, labels, search keywords and disabled options, and
`SelectMenu` for dropdown styling.

```rust
use uic::components::{
    autocomplete::{Autocomplete, AutocompleteEvent, AutocompleteState},
    select::SelectOption,
};

let search = cx.new(|cx| AutocompleteState::new(vec![
    SelectOption::new("design", "Design system").keywords("UI library"),
    SelectOption::new("research", "Research notes"),
], window, cx));

let subscription = cx.subscribe(&search, |this, _, event: &AutocompleteEvent, cx| {
    match event {
        AutocompleteEvent::Change(value) => this.query = value.clone(),
        AutocompleteEvent::Selected(option) => this.open_workspace(&option.id, cx),
        AutocompleteEvent::Submit(value) => this.search(value, cx),
    }
});
// Retain the state and subscription in the view.

Autocomplete::new(&self.search)
    .label("Workspace")
    .placeholder("Search or enter a name")
    .w_full()
    .rounded_xl();
```

Initialize `input::init(cx)` once for input bindings. Each state belongs to one
input in one window. `value()` returns the committed text; IME pre-edit text is
not emitted as `Change`.

## Editing and selection

Focusing or editing opens suggestions. Local filtering matches labels and
keywords without case sensitivity. By default, no option is automatically highlighted:
Down selects the first enabled suggestion, Up selects the last, and subsequent
arrow keys move through enabled options. Arrow navigation does not edit text.

Use `AutocompleteState::new(options, window, cx).auto_highlight(true)` to highlight
the first enabled match after editing. Enter then confirms it without an arrow-key
step. Focusing the field alone and programmatic `set_value` calls do not activate
this behavior. Asynchronous results also receive a highlight after loading finishes
if the menu is still open for that edit; an existing valid highlight is preserved.
Highlighting does not replace text or emit `Selected` until confirmation.

Enter confirms the highlighted option. Clicking a row does the same. Its label
replaces the text, the menu closes, and input focus stays in the field. A changed
value emits `Change`, followed by `Selected` with the option's stable ID and label.
Selection does not also emit `Submit`. Enter without a highlighted option emits
`Submit` with the free text. Typing a matching label does not implicitly select its ID.

Escape dismisses suggestions without modifying text or moving focus. Tab closes
the menu and moves focus normally. Clicking outside both the field and menu closes
suggestions and clears input focus. Suggestions are hidden during IME composition,
and composition keystrokes do not navigate or confirm suggestions.

`set_value(value, cx)` changes text and emits `Change` if it differs, without opening
a closed menu. `set_disabled(true, cx)` disables editing and dismisses suggestions.
`input()` exposes the underlying input for configuring its input purpose and other
text-input options; keep it single-line.

## Keyboard shortcuts

Use `.key_binding(shortcut, action)` on `Autocomplete` to add shortcuts for one
input. Defaults are Up/Down, Enter and Escape. The last binding for a matching
key takes precedence. Shortcuts use GPUI key syntax and accept one combination,
such as `ctrl-n`, `alt-down` or `secondary-j`, rather than multi-step sequences.

```rust
use uic::components::autocomplete::AutocompleteAction;

Autocomplete::new(&self.search)
    .key_binding("ctrl-p", AutocompleteAction::Previous)
    .key_binding("ctrl-n", AutocompleteAction::Next)
    .key_binding("tab", AutocompleteAction::Confirm);
```

`Previous` and `Next` navigate enabled suggestions. `Confirm` selects the active
suggestion or submits free text when none is active. `Dismiss` closes the menu.
Unbound keys retain normal input behavior; Tab/Shift-Tab move focus unless explicitly
bound. Binding Tab to Confirm keeps focus in the input. Bindings are ignored during
IME composition and do not affect other autocomplete instances.

## Dynamic suggestions

Use `set_options(options, cx)` to replace suggestions without changing text or
reopening a dismissed menu. IDs must be unique. Removing or disabling the active
option clears its highlight, or activates the first enabled match when auto-highlighting
is enabled for the current edit. Options are rendered eagerly, so supply a bounded list.

Create the state with `.filter(false)` when the application already filters or
ranks results. Set `set_loading(true, cx)` while fetching; this clears the highlight
and shows a loading message instead of stale suggestions. Replace options and call
`set_loading(false, cx)` when the current request completes. Free-text submission
remains available while loading.

The application owns requests, debouncing, cancellation and result ordering. Before
applying an asynchronous response, verify that it belongs to the current query or
request generation. `Change` also fires after selecting a different label; applications
that fetch suggestions should account for the following `Selected` event.

## Styling

Styled methods apply to the complete input surface. `AutocompleteAppearance`
configures input semantic colors, highlighted rows, muted text and disabled opacity.
The default menu follows the input width and grows to its height limit, constrained
by the viewport, safe area and keyboard insets.

`.menu(SelectMenu::new().w_auto().h_auto().max_h(px(300.)))` customizes the dropdown.
An auto width follows the input. `.render_option(...)` renders row content while
preserving selection behavior; the shared `SelectItemState` has `highlighted` and
`disabled`, while `selected` is always false because free text has no retained ID.
Use `.empty_text(...)` and `.loading_text(...)` to customize status messages.

```sh
cargo run -p uic --example autocomplete
```
