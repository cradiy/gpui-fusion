# Animated numbers

`animated_number` smoothly follows a numeric target. Its builder receives the
current `f64` value and returns ordinary GPUI content. Formatting, typography,
units and layout belong to the builder.

```rust,ignore
use std::time::Duration;
use gpui::{div, prelude::*, px};
use gpui_effects::animated_number;

animated_number("zoom", zoom_percent, |value| {
    div().text_size(px(24.)).child(format!("{value:.1}%"))
})
.duration(Duration::from_millis(240))
.enabled(motion_enabled)
```

Use a stable ID and supply the new target on each render. The first value appears
immediately. Subsequent target changes interpolate from the currently displayed
value, including reversals. Each change takes the configured duration (240 ms by
default). Changing duration during motion restarts from the current value.
Cubic smoothstep keeps values continuous when interrupted, but does not preserve
velocity. Repeatedly changing the target can delay arrival until updates stop.

Use `format!("{value:.0}")` for rounded integers, `format!("{value:.2}")` for two
decimal places, or a custom formatter for grouping and units. Negative values
are supported. Numeric precision follows `f64`. Use the original application
value for actions and calculations.

Non-finite targets are passed directly to the builder without animation. The
first finite value following a non-finite value also snaps; later updates animate
normally. The builder decides how to display these values.

`.enabled(false)` or zero duration snaps to the target. Settled values request no
animation frames. The wrapper captures no textures, retains no builder or child
between frames, and leaves child input handling unchanged.

Text is laid out normally on each frame. Reserve width or use tabular digits in
the child if changing digit counts should not shift adjacent content. The wrapper
does not animate its bounds.

```sh
cargo run -p gpui_effects --example animated_number
```
