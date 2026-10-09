# Range slider

`RangeSlider` selects a horizontal numeric interval. Both endpoints support
pointer dragging, keyboard focus and accessibility increment/decrement actions.

```rust
use uic::components::slider::{RangeSlider, RangeSliderEvent, RangeSliderState};

let range = cx.new(|cx| {
    RangeSliderState::new(20.0..=80.0, 0.0..=100.0, cx)
        .step(5.0)
        .min_gap(10.0)
});
let subscription = cx.subscribe(&range, |this, _, event: &RangeSliderEvent, cx| {
    match event {
        RangeSliderEvent::Changing(values) => this.preview = values.clone(),
        RangeSliderEvent::Changed(values) => this.apply(values.clone()),
    }
    cx.notify();
});
// Retain the state and subscription in the view.

RangeSlider::new(&self.range)
    .label("Price")
    .thumb_labels("Minimum price", "Maximum price")
    .w_full()
    .h(gpui::px(44.));
```

## Values and events

The first constructor argument is the selected interval; the second is the
allowed domain. Reversed selections are sorted, out-of-domain values are
clamped, and nonfinite selected values use the domain minimum. Domain bounds
must be finite and ordered.

`step(0.0)` allows continuous dragging. Positive steps are measured from the
domain minimum. The domain maximum remains reachable even if it is not an exact
step. `min_gap` is capped at the domain span. When necessary, endpoints move
outward to the next step to preserve that minimum distance. Step and gap must
be finite and nonnegative.

`values()` reads the interval. `set_values(values, cx)` normalizes a replacement
without emitting an interaction event. `set_disabled(true, cx)` disables both
endpoints. Subscribe to changes or observe the state to refresh displayed values
after programmatic updates.

Dragging emits `Changing` when values change, then one `Changed` on release.
Keyboard and accessibility adjustments emit `Changed` when the interval changes.
Endpoints stop at their allowed limits and do not exchange identities.

## Interaction and appearance

Clicking the track adjusts the nearest endpoint. With coincident endpoints,
click to their left to adjust the lower value or to their right to adjust the
upper value. Pointer capture keeps a drag active outside the track.
Touch dragging waits for horizontal intent; vertical gestures remain available
to the surrounding scroll view. A cancelled touch retains the last preview
values and does not emit a release event.

Tab traverses the two endpoints in lower/upper order. Arrow keys adjust by one
step, Page Up/Down by ten steps, and Home/End move to the selected endpoint's
allowed limits. Continuous sliders use one percent of the domain for keyboard
and accessibility adjustments. `thumb_focus_handle` can focus either endpoint
programmatically. The group and both endpoints expose accessible names.

`Styled` controls the outer surface and layout. `SliderAppearance` is shared with
`Slider`: its Styled methods control the track, and its semantic fields control
the active track, thumb and focus colors. Secondary-track settings are unused.
Use a taller outer surface, such as `.h(px(44.))`, for a larger touch target.

```sh
cargo run -p uic --example range_slider
```
