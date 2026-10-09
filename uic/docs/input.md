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

A primary click outside the input clears focus without consuming the click.
Prefix, suffix and scrollbar content belong to the input's focus boundary.
Use `.blur_on_click_outside(false)` when a containing composite manages focus
across a larger surface. Ordinary inputs do not emit Submit merely on blur.

## Autofill

Give each participating field a stable name and an explicit content hint:

```rust
let username = cx.new(|cx| {
    TextInput::new(cx)
        .placeholder("Username or email")
        .autofill("login-username", gpui::AutofillHint::Username)
});
let password = cx.new(|cx| {
    TextInput::new(cx)
        .password()
        .autofill("login-password", gpui::AutofillHint::Password)
});
```

Names must be nonempty, unique within the window, and stable across redraws. They
identify fields; hints describe their meaning. A password manager does not need
to infer the field type from its name or placeholder. Use `NewPassword` when
creating or changing a password, and `Email` for an email field that is not an
account identifier. Other hints cover names, phone numbers, addresses, and
one-time codes. Keyboard layout remains controlled by `input_purpose`.

Autofill is opt-in. `set_autofill(Some(AutofillOptions::new(name, hint)), cx)` changes
the configuration; `set_autofill(None, cx)` disables it. Disabled fields do not
participate. A selected account can fill multiple rendered fields, including
unfocused fields. Fills replace the complete value using normal input editing
and `InputEvent::Change` notifications. Password values remain masked for
accessibility and are withheld from IME surrounding text.

After the application accepts a form, call `window.commit_autofill()` to finish
its autofill context, then replace the form with the next application screen.
Use `window.cancel_autofill()` when abandoning a form.
Neither operation authenticates a user, changes input values, or stores
credentials in GPUI. Do not commit merely because an input loses focus.

| Platform | Integration |
| --- | --- |
| Android | Virtual fields supplied to the user's enabled Autofill service. No extra GPUiForge feature or permission is required. |
| Web/WASM | Real form inputs with stable `name` and HTML `autocomplete` attributes receive clicks and synchronize with GPUI. Completion notifies form observers and offers password credentials to the browser's Credential Management API where supported; it never posts credentials to a server. |
| Native Linux, Windows, macOS | The same input configuration compiles, but these backends do not expose a system autofill adapter. Manual editing and paste remain available. |

`window.supports_autofill()` reports adapter availability, not whether the user
has enabled a service or saved an account. Completing or cancelling a context
returns an error on platforms without an adapter. On Web, cancellation drops
pending bridge fills; browsers provide no API to cancel a password manager's
own session. Browser password-manager
heuristics vary; HTML hints do not force a suggestion or a save prompt. Use HTTPS
or localhost for browser credential storage. Password-manager extensions must be
enabled and unlocked for the page; their inline suggestions and save prompts
are controlled by the extension's settings.

On Android, focusing a participating field starts a system autofill request.
When a service is enabled, the native long-press editing menu also offers
**Autofill** to request suggestions explicitly. The service decides whether to
return suggestions or offer saving after `commit_autofill()`.

Run `cargo run -p uic --example autofill` to inspect the shared form. For browser
password-manager testing, run `trunk serve` from
`crates/gpui_web/examples/autofill_web` and open `http://127.0.0.1:8080`.
Use a saved test account for that origin or your password manager's manual fill.

Custom input
components can register their fields during paint with `Window::handle_autofill`,
using a stable focus handle and a weak entity reference in the fill callback.

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
