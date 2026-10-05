# Animated presence

`animated_presence` drives a child's entrance and exit from a boolean target.
Keep the wrapper in the element tree with a stable ID, including while hidden.
The builder receives a visibility weight and chooses the animated styles.

```rust,ignore
use std::time::Duration;
use gpui::{div, prelude::*, px};
use gpui_effects::animated_presence;

let panel = animated_presence("details", show_details, |frame| {
    div()
        .opacity(frame.progress)
        .relative()
        .top(px(12. * (1. - frame.progress)))
        .child("Details")
})
.duration(Duration::from_millis(240));
```

The wrapper applies no visual styling itself. `PresenceFrame::progress` moves
between zero and one with cubic smoothstep interpolation. `phase` distinguishes
`Entering`, `Visible` and `Exiting`. The builder runs while entering, visible
and exiting; after exit it is skipped and the child leaves layout and painting,
including flex gaps. The wrapper does not retain a snapshot or an entity: keep
view entities and application state owned by the parent when they must survive
hiding. Supply the outgoing data until exit finishes.

The default duration is 240 ms. A reversal starts at the currently sampled
progress and takes a proportion of the full duration. Position is continuous;
velocity is not preserved. A duration change retargets from the current value.
Initially visible children appear immediately unless `.animate_initial(true)`
is set. `.enabled(false)` or zero duration adopts the target immediately, for
example when the application requests reduced motion.

Ordinary mouse hit testing is blocked within the child's bounds during the
transition. Fully visible children receive normal input. The application owns
keyboard focus, key handlers and global mouse listeners; move focus out of a
panel when hiding it. Keep animated content inside its root bounds if it needs
the transition's mouse blocker.

The child reserves its full layout size until exit finishes; the wrapper does
not animate surrounding layout. An entering child reserves its size from the
start. Style the child for dimensions, layout and animation; relative offsets
move its layout and hit geometry together. Different visual transformations
may need their own interaction mapping.

The container requests frames only while its progress is changing. It requires
no shader or offscreen capture; costs of styles and effects in the builder are
determined by those children.

```sh
cargo run -p gpui_effects --example presence
```
