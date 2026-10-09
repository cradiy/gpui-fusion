# Number input

`NumberInput` edits a finite decimal value with optional step controls. The same
component and state work on desktop, Android and Web. Initialize
`input::init(cx)` once for text-editing key bindings.

```rust
use uic::components::number_input::{
    NumberInput, NumberInputChanged, NumberInputOptions, NumberInputState,
};

let width = cx.new(|cx| {
    NumberInputState::new(160., NumberInputOptions {
        min: Some(48.),
        max: Some(240.),
        step: 8.,
        precision: 0,
    }, window, cx)
});
let subscription = cx.subscribe(&width, |this, _, event: &NumberInputChanged, cx| {
    this.width = event.value;
    cx.notify();
});
// Retain the state and subscription in the view.

// In render:
NumberInput::new(&self.width)
    .label("Width")
    .suffix(div().text_sm().child("px"))
    .w(px(240.))
    .h(px(48.))
    .rounded_lg();
```

## Editing and events

Typing preserves the draft, including empty text, `-`, `0.` and unfinished
exponents. `value()` returns the committed number; `draft(cx)` returns the
current text. Enter, the software keyboard's Done action, or loss of focus
confirms the draft. Confirmation clamps to the configured bounds, rounds to
`precision` decimal places, and removes trailing fractional zeros. Invalid or
non-finite input reverts to the committed value. Escape discards the draft.
Input-method pre-edit is left to the input method; step controls and numeric
shortcuts do not confirm a candidate.

Up/Down and the stacked arrow buttons apply one `step` and commit immediately. They
use the current draft when it is a finite number, or the committed value
otherwise. A step is an increment, not a constraint that typed numbers must be
multiples of it. Controls stop at the bounds.

`NumberInputChanged` is emitted only when a user commits a different number.
Programmatic `set_value(value, cx)` replaces both the committed value and draft,
without emitting this event; it returns false for a non-finite argument.
This allows two-way binding to a Slider without a feedback loop:

```rust
// NumberInputChanged subscriber:
self.slider.update(cx, |slider, cx| slider.set_value(event.value, cx));

// SliderEvent subscriber (both Changing and Changed):
self.width.update(cx, |number, cx| { number.set_value(value, cx); });
```

Each state belongs to one rendered NumberInput in one window.

## Rules and appearance

`NumberInputOptions` defaults to no explicit bounds, step `1`, and precision
`2`. Values and bounds must be finite, minimum must not exceed maximum, and
step must be positive. Precision ranges from 0 to 12. Bounds and step must be
representable at that precision. Invalid configurations panic during state
construction or `set_options(options, cx)`. Changing options normalizes the
current value and discards the draft without emitting a user change.

`state.set_disabled(true, cx)` discards the draft and disables editing and step
controls. `.controls(false)` hides the buttons while preserving keyboard
stepping. Mobile keyboards receive numeric-purpose hints, including decimal
and sign keys when allowed by the options.

`Styled` methods style the outer input surface and its inherited typography.
`prefix(...)` and `suffix(...)` accept content such as a currency symbol or unit.
`NumberInputAppearance` configures the input's focus/caret/selection colors,
step-button hover color and disabled opacity. `.label(...)` supplies an
accessible name; the control exposes its numeric value, bounds and increment
and decrement actions.

The parser uses `.` as the decimal separator and accepts scientific notation.
It does not parse localized grouping separators or perform exact decimal
arithmetic for financial values.

Run the input and Slider example:

```sh
cargo run -p uic --example number_input
```
