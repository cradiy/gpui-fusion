# Tags input

`TagsInput` edits a wrapping list of text labels. It supports trimming,
deduplication, a tag-count limit, custom validation and keyboard removal.
Initialize `input::init(cx)` once for text-editing bindings.

```rust
use uic::components::tags_input::{
    TagsInput, TagsInputChanged, TagsInputOptions, TagsInputState,
};

let tags = cx.new(|cx| {
    TagsInputState::new(
        vec!["Design".into(), "Research".into()],
        TagsInputOptions { max_tags: Some(6), ..Default::default() },
        window,
        cx,
    ).validator(|tag| {
        if tag.chars().count() > 24 {
            Err("Keep each tag to 24 characters or fewer".into())
        } else {
            Ok(())
        }
    })
});
let subscription = cx.subscribe(&tags, |this, _, event: &TagsInputChanged, cx| {
    this.labels = event.tags.clone();
    cx.notify();
});
// Retain tags and subscription in the view.

// In render:
TagsInput::new(&self.tags)
    .label("Project tags")
    .placeholder("Add a tag…")
    .w_full()
    .rounded_xl();
```

## Editing

Enter or the software keyboard's Done action adds the draft. Leading and
trailing whitespace is removed; whitespace within a tag is preserved.
Commas are ordinary characters, and pasted text is one draft rather than a
batch of labels. Input-method confirmation does not add a tag while pre-edit
is active. Press Enter after confirming the candidate to add the label.

Click a label's remove button to delete it. With an empty draft, Backspace
selects the last label before a second Backspace removes it. Left/Right move
between labels; Right from the last label returns to the editor. Delete removes
the selected label, and Escape clears the selection or validation error.
Typing clears label selection. Ordinary text editing shortcuts remain with the
input when the draft is nonempty.

Clicking outside clears focus and retains the draft. It does not add a tag.
`set_disabled(true, cx)` disables editing and removal without clearing tags or
the draft.

## Validation and state

Tags must be nonempty after trimming and cannot contain control characters.
Duplicates are compared using Unicode lowercase strings by default. Set
`case_sensitive: true` to compare the trimmed strings exactly. `max_tags: None`
allows an unrestricted count. Duplicate checks precede the limit check.

`.validator(...)` receives a trimmed, otherwise valid tag and returns a custom
error message. It applies to future additions and replacements; it does not
retroactively validate initial tags. Invalid initial tags panic at construction.

Rejected submissions retain the draft, mark the input border, and expose an
error through `state.error()`. Display that message near the field:

```rust
let error = self.tags.read(cx).error().map(ToString::to_string);
div().child(TagsInput::new(&self.tags)).children(error)
```

`tags()` and `draft(cx)` provide read access. `add(tag, cx)` validates and adds a
tag; `remove(tag, cx)` removes an exact stored label. Successful changes emit
`TagsInputChanged` with the complete list. Programmatic `add` returns validation
errors to its caller; it does not set the editor's submission error.

`set_tags(tags, cx)` validates the entire replacement before applying it. On
success it resets the draft and error without emitting a user-change event; on
failure the existing state is untouched. Each state belongs to one rendered
TagsInput in one window.

## Styling

`Styled` methods control the input surface, spacing and inherited typography.
Labels wrap to new rows as the available width decreases, and the surface grows
with its content. A label wider than the field is visually truncated; its full
value remains in state and in the remove button's accessible name.

`TagsInputAppearance` configures label colors, selected-label and remove-button
states, validation borders, input caret/selection/focus colors and disabled
opacity. `.label(...)` names the group and text editor for accessibility.

Run the example:

```sh
cargo run -p uic --example tags_input
```
