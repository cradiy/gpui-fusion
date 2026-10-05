# Staggered presence

`staggered_presence` schedules entrance and exit animations for keyed items in
a normal flex or grid container. Builders receive the same `PresenceFrame` as
`animated_presence` and choose their own opacity, offsets and other styles.

```rust,ignore
use std::time::Duration;
use gpui::{div, prelude::*, px};
use gpui_effects::staggered_presence;

let mut list = staggered_presence("results", visible)
    .flex().flex_col().gap(px(8.))
    .duration(Duration::from_millis(240))
    .interval(Duration::from_millis(60));

for (id, title) in items {
    list = list.item(id, move |frame| {
        div().opacity(frame.progress)
            .relative().top(px(12. * (1. - frame.progress)))
            .child(title)
    });
}
```

IDs must be unique within the group and stable across renders. Entries start
in supplied order. Exits use `StaggerOrder::Reverse` by default; choose
`.exit_order(StaggerOrder::Forward)` to retain entrance order.

Each item has a full travel duration of 240 ms and consecutive starts are spaced
60 ms apart by default. Reversals start at each item's current sampled progress,
with a fresh delay based on the new direction. Remaining travel time scales with
the remaining distance. Interpolation uses cubic smoothstep; position is
continuous, but velocity is not preserved. Changing duration, interval or exit
order reschedules from current values. Reordering alone retains active schedules;
the next transition uses the new order.

Initially visible groups appear immediately unless `.animate_initial(true)` is
set. New IDs added to an existing visible group enter using their current index.
Directly removed IDs disappear immediately; this component does not retain
removed data. To animate an entire list out, keep supplying its items and set
`visible` to false. Removing the group itself also skips exit animation.

Every item reserves its layout slot while the group enters or exits, including
items waiting to enter or already fully faded out. The whole container leaves
layout after the last exit completes. Fully hidden groups skip their builders.
The wrapper does not animate list reordering or the surrounding layout.

Items with zero progress are neither prepainted nor painted. Ordinary mouse
hit testing is blocked within each transitioning item's bounds, including during
its delay. Fully entered items become interactive individually. The application
owns keyboard focus and global listeners; move focus out when hiding the group.

`.enabled(false)` or zero duration snaps the whole group and skips delays. No
frames are requested at rest. The group itself performs no texture capture;
styles and effects supplied by builders determine additional rendering costs.

```sh
cargo run -p gpui_effects --example stagger
```
