# Select

`Select` chooses one option by stable ID. It supports search, disabled options,
keyboard navigation, clearing and custom option content. `SelectState` owns the
options and selected ID; the application subscribes to `SelectChanged`.

```rust
use uic::components::select::{Select, SelectChanged, SelectOption, SelectState};

let select = cx.new(|cx| {
    SelectState::new(vec![
        SelectOption::new("personal", "Personal"),
        SelectOption::new("studio", "Design studio").keywords("creative art"),
        SelectOption::new("archive", "Archive").disabled(true),
    ], window, cx)
});
let subscription = cx.subscribe(&select, |this, _, event: &SelectChanged, cx| {
    this.workspace = event.selected.clone();
    cx.notify();
});
// Retain both select and subscription in the view.

// In render:
Select::new("workspace", &self.select)
    .label("Workspace")
    .placeholder("Choose a workspace")
    .clearable(true)
    .w_full()
    .h(px(48.))
    .rounded_xl();
```

Initialize `input::init(cx)` for input key bindings. Each Select state belongs
to one trigger in one window.

## Options and selection

`SelectOption::new(id, label)` separates persistent values from display text.
IDs must be unique. Optional `keywords(...)` participate in case-insensitive
substring search alongside the label. `.searchable(false)` hides the search
field. Query text is reset whenever the picker opens.

`state.select(id, cx)` selects an enabled option and returns false for missing or
disabled IDs. `state.clear(cx)` removes the selection. These methods emit
`SelectChanged` only when the selected ID changes. Opening, filtering, moving the
keyboard highlight and cancelling do not emit selection changes.

`set_options(options, cx)` preserves the selected ID while it exists. Removing
it clears the selection and emits a change. Making an already selected option
disabled preserves its displayed value but prevents choosing it again.
`selected_id()`, `selected_option()` and `options()` provide read access.

Up and Down move the highlight while skipping disabled options. Enter accepts;
Escape cancels. Input-method pre-edit remains owned by the input method. Search
filters on committed text, and confirming a pre-edit does not accept an option.
Tab dismisses a dropdown and continues normal focus navigation.

`.disabled(true)` prevents user interaction and closes an open picker.
`.clearable(true)` adds a clear control and allows Delete or Backspace to clear
the focused trigger. Applications can also clear selection
programmatically. `state.close(window, cx)` dismisses the current picker.

## Dropdown styling

Select displays an anchored dropdown on every platform.

`Styled` methods on `Select` style the trigger. `SelectMenu` styles the option
surface:

```rust
use uic::components::select::SelectMenu;

Select::new("workspace", &self.select)
    .font_family("sans-serif")
    .text_sm()
    .menu(SelectMenu::new().w(px(360.)).h(px(420.)).bg(rgb(0xffffff)));
```

Dropdown dimensions are capped to the available window space, accounting for
system bars and the keyboard. The trigger's explicit typography propagates
to the menu unless overridden there. `SelectAppearance` configures semantic
colors for highlighted rows, selected indicators, placeholder text and focus,
plus disabled opacity.

`.label(...)` sets the accessible trigger name;
`.empty_text(...)` customizes empty search results. `.search_placeholder(...)` customizes the search field placeholder.

Use `.render_option(...)` to customize each row's content. The surrounding row
retains pointer selection, disabled behavior and accessibility semantics:

```rust
Select::new("workspace", &self.select)
    .render_option(|option, flags, _, _| {
        div().flex().gap_2()
            .child(option.label.clone())
            .when(flags.disabled, |row| row.child("Unavailable"))
    });
```

`SelectItemState` exposes `selected`, `highlighted` and `disabled`. Options are
all rendered; this component is intended for moderate choice lists, not
virtualized datasets. A custom row's interactive children may consume input;
prefer display content inside options.

Run the custom-content example:

```sh
cargo run -p uic --example select
```
