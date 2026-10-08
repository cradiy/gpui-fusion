# Text inputs

Create a `TextInput` entity to hold editing state, then render it with `Input`:

```rust
let name = cx.new(|cx| {
    TextInput::new(cx)
        .aria_label("Display name")
        .placeholder("Enter your name")
});

Input::new(&name).w_full()
```

Use `.multiline()` for a message field and `.password()` for a password field.
Apply layout, typography, borders, and backgrounds through `Input`'s `Styled`
methods. Use `InputAppearance` for caret, selection, placeholder, and focus colors.

## Accessibility

Set `TextInput::aria_label` to a persistent, descriptive field name. The placeholder
is used as the accessible name when no label is supplied. Change a label at runtime
with `set_aria_label(label, cx)`.

When the platform activates accessibility, inputs expose their role, text, selected
range, placeholder, and disabled state. Supported system actions focus the field,
set its selection, replace selected text, or replace its complete value. Available
actions depend on the platform adapter. Edits follow the same input transactions
and `InputEvent::Change` notifications as keyboard edits; changing a selection
does not emit `Change`. Disabled fields reject editing actions.

Password fields expose masked text instead of the underlying value. Text runs
provide visual-row bounds and grapheme boundaries for text navigation. Individual
character bounds are not exposed, so accessibility tools requiring precise
per-character screen geometry have limited positioning information.
