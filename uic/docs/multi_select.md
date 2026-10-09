# MultiSelect

`MultiSelect` chooses multiple options by stable ID and displays removable labels.
It shares `SelectOption`, `SelectState`, `SelectMenu` and `SelectAppearance` with
[Select](select.md). Create its state with `SelectState::multiple`.

```rust
use uic::components::select::{MultiSelect, MultiSelectChanged, SelectOption, SelectState};

let categories = cx.new(|cx| {
    SelectState::multiple(vec![
        SelectOption::new("design", "Design").keywords("creative art"),
        SelectOption::new("research", "Research"),
        SelectOption::new("archive", "Archived").disabled(true),
    ], window, cx).max_selected(3)
});
let subscription = cx.subscribe(&categories, |this, _, event: &MultiSelectChanged, cx| {
    this.category_ids = event.selected.clone();
    cx.notify();
});
// Retain the state and subscription in the view.

MultiSelect::new("categories", &self.categories)
    .label("Categories")
    .placeholder("Choose categories")
    .clearable(true)
    .w_full()
    .rounded_xl();
```

Initialize `input::init(cx)` for search input bindings. Each state belongs to one
trigger in one window. `SelectState::new` is for single selection; pairing that
state with MultiSelect is an error.

## Selection

Clicking a row or pressing Enter toggles the highlighted option and keeps the
dropdown open. Search text is retained between choices and cleared on reopening.
The search matches labels and keywords without case sensitivity. Input-method
pre-edit confirmation does not select an option.

Selections are ordered by when they were chosen. `selected_ids()` reads all IDs;
`is_selected(id)` checks membership. `select(id, cx)` adds an enabled option and
returns false for unavailable IDs or a full selection. Selecting an existing ID
does not duplicate it. `deselect(id, cx)` removes an ID; `clear(cx)` removes all.

`set_selected_ids(ids, cx)` atomically replaces selection. It returns false
without changes if IDs are duplicated, missing, disabled, or exceed the limit.
All successful selection changes, including programmatic ones, emit
`MultiSelectChanged`; single-selection `SelectChanged` is not emitted.

The count is unrestricted unless `.max_selected(n)` is specified when creating
the state. At the limit, unselected options are unavailable; selected options
can still be unchecked. The dropdown footer displays the selected count.

`set_options(options, cx)` preserves IDs that still exist, including options that
become disabled, and emits one change if selections disappear. Labels follow the
current option text. Disabled rows cannot be toggled, but their selected labels
can still be removed individually or cleared. Option IDs must be unique.

## Interaction and styling

Up/Down navigate available rows; Enter toggles. Space toggles when search is
disabled. Escape and outside clicks close the dropdown without reverting changes.
Tab closes and moves focus. On the closed trigger, Backspace removes the last
selected label. `.clearable(true)` adds Clear to the dropdown footer and enables
Delete to clear all.
`.disabled(true)` prevents user interaction and closes the dropdown.

Selected labels wrap and the trigger grows with them. Use `min_h` instead of a
fixed `h` if multiple rows are possible. Overlong labels are visually truncated;
the full ID and label remain in state. Remove controls expose accessible names.

Styled methods customize the trigger. `.menu(SelectMenu::new()...)` styles the
dropdown, and `.appearance(...)` sets semantic colors for highlighted rows,
labels, focus and disabled states. `.render_option(...)` customizes option
content while preserving row behavior. `.searchable(false)` hides search.
`.search_placeholder(...)` and `.empty_text(...)` customize search prompts.

The dropdown is constrained to the window's available area, including safe-area
and keyboard insets. Options are rendered eagerly; use moderate choice lists.

```sh
cargo run -p uic --example multi_select
```
