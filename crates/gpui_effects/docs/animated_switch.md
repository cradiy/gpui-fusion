# Animated switch

`animated_switch` owns playback and the outgoing value for a two-content
transition. Provide a stable element ID, a `Clone + Eq` value, and a builder that
can render both current and previous values. Give the wrapper explicit dimensions
or fill a bounded parent.

```rust,ignore
use gpui::{div, prelude::*, px};
use gpui_effects::{TransitionKind, animated_switch};

animated_switch("status", status, |status| {
    div().size_full().flex().items_center().child(status.label())
})
.w(px(180.)).h(px(40.))
.kind(TransitionKind::CrossFade)
.enabled(motion_enabled)
```

The first value appears immediately. Equality controls replacement: rebuilding
with an equal value updates its content without starting another transition.
Keep values small and immutable, such as enum variants, strings or lightweight
snapshots. A builder must still be able to render an outgoing value after the
application requests a replacement; looking it up in an already-deleted entry
will not work. Avoid values that own the containing view or application.

Crossfade is the default. `.kind(...)` accepts the existing `TransitionKind`
presets, including blur fade, directional wipes and dissolve. The wrapper uses
their default visual parameters. Style the wrapper for its bounds, background,
border and padding; style the builder's content for appearance that should
participate in the transition. Both inputs share the same content area, so the
wrapper does not measure or animate intrinsic height.

## Interruption and lifetime

Full travel takes 240 ms by default; configure it with `.duration(...)`.
Switching back to the outgoing value reverses from the current progress, taking
time proportional to the remaining distance. Progress stays continuous, while
velocity can change. Changing duration retargets from current progress.

Requesting a third value lets the active transition finish before starting the
next one. Only the latest requested value is used; intermediate requests are not
queued. Playback state retains at most two values, and the builder runs at most
twice in a frame. Once
settled, only the visible value is built. The outgoing value is released and the
incoming subtree keeps its element identity. New incoming content receives a
fresh identity rather than inheriting an old content slot's state.

Only values and playback state persist across frames. Builders and elements are
not stored in that state. Omitting the wrapper discards its state without an exit
animation. Keep the wrapper mounted while a transition should complete.

`.enabled(false)` or zero duration snaps to the latest requested value and drops
outgoing content. Settled switches request no frames and bypass the two-input
GPU capture. Unsupported subtree-effect backends use the existing transition's
midpoint swap behavior.

## Interaction

The content region blocks ordinary pointer hit testing for the full transition,
including its first frame. Put switching controls outside that region. The
application owns keyboard focus, accessibility and global input listeners; move
focus out of outgoing content when appropriate.

Rendering inherits the clipping and deferred-overlay limits described in
[subtree transitions](subtree_transition.md). Continuous third-value requests
may add up to one active transition's remaining time before the newest value
starts entering.

```sh
cargo run -p gpui_effects --example animated_switch
```
