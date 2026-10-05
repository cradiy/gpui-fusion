# Animated collapse

`animated_collapse` opens and closes content by animating its height in normal
layout. Following elements move with the container. Content keeps its natural
layout and is clipped to the currently displayed height.

```rust,ignore
use std::time::Duration;
use gpui::{div, prelude::*, px};
use gpui_effects::animated_collapse;

let details = animated_collapse("details", expanded, || {
    div().p(px(16.)).child("Details that can wrap to multiple lines.")
})
.duration(Duration::from_millis(260));
```

Keep the wrapper mounted with a stable ID while collapsed. It fills its parent's
width and owns its animated height. Set width constraints on the parent; place
padding, borders and backgrounds inside the child so they collapse with it.
Use intrinsically sized content, not a height relative to the collapsing parent.
The parent must allow the wrapper to determine its height.

Initially expanded content uses natural layout without an entrance animation.
Subsequent changes interpolate with cubic smoothstep over 260 ms by default.
Reversing starts at the current height. A new measured height, including one
caused by wrapping at a different width, retargets the animation. Measurements
are collected after layout and become animation targets on the following frame.
Retargeting preserves position, not velocity.

`.enabled(false)` or zero duration uses natural layout when expanded and removes
collapsed content immediately. Fully collapsed content is not built or painted,
and takes no layout space. Keep child entities owned by the parent if their state
must survive hiding. The builder must continue supplying outgoing content until
closing completes.

Ordinary mouse input is blocked inside the container while its height changes.
Clipping excludes hidden content from pointer hit testing. Keyboard focus and
global listeners remain application-owned; move focus out when closing an input
panel. External flex gaps disappear when the container is fully removed; place
animated spacing inside the child to avoid a final gap jump.

The container does not capture textures or scale text. It requests frames only
while moving or when a new content measurement needs to be applied.

```sh
cargo run -p gpui_effects --example collapse
```
