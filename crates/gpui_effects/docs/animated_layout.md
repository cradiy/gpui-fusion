# Animated layout

`animated_layout` measures keyed children in a normal flex or grid container and
animates their positions when layout changes. It does not require target rectangles.

```rust,ignore
use gpui::{div, prelude::*, px};
use gpui_effects::animated_layout;

let mut cards = animated_layout("cards")
    .w_full().grid().grid_cols(3).gap(px(16.));

for item in items {
    cards = cards.item(item.id, div().p(px(20.)).child(item.title));
}
```

Use unique, stable item IDs tied to data identity, not the current array index.
The group gives each item its own element namespace. Child styles such as grid
spans, flex growth, margins and minimum sizes participate in normal layout.

## Motion

The first appearance uses the measured position immediately. Sorting, filtering,
insertion, removal and changes to the layout constraints move existing items
toward their new positions. The default duration is 260 ms with cubic ease-out.
Use `.duration(...)` to change it. Retargeting starts from the current interpolated
position, preserving position continuity but not velocity.

Positions are relative to the group. Ancestor movement and scrolling apply
immediately without restarting the transition. Nested groups keep independent
local motion. `.enabled(false)` or zero duration snaps to the current layout;
re-enabling motion does not replay skipped changes. No frames are requested at rest.

## Layout and lifetime

Only position is animated. Child sizes, text wrapping, container dimensions and
scroll extents use the final layout immediately. Contents remain live and are not
scaled or captured into textures. Mouse hit geometry follows the displayed
position, so controls remain usable during movement.

New IDs appear at their final positions. Removed items leave immediately and their
data is not retained for an exit animation. A returning item starts a new motion
lifecycle after its previous element state is discarded. Zero-sized items reset
their motion history. The group keeps only numeric animation state for each item.

Items paint in the supplied order; later items appear above earlier ones while
paths overlap. Normal ancestor clips apply, so a shrinking container may clip a
moving item before it reaches its destination. Allow overflow or reserve enough
space when the whole motion path must remain visible.

Keep keyboard focus and selection in application state. The component does not
implement dragging, sorting, virtualized lists or removed-item retention.

```sh
cargo run -p gpui_effects --example animated_layout
```
