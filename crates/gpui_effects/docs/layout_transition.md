# Layout transitions

`layout_transition` animates a container toward a target rectangle relative to a
positioned parent. Each frame uses real layout: text reflows with the width, and
pointer targets follow the displayed controls. No image scaling or subtree
capture is involved.

## Usage

```rust
use gpui::{Bounds, div, point, prelude::*, px, rgb, size};
use gpui_effects::layout_transition;

let target = Bounds::new(point(px(24.), px(16.)), size(px(280.), px(160.)));
let panel = div().relative().w(px(600.)).h(px(400.)).child(
    layout_transition("details", target)
        .rounded(px(16.))
        .bg(rgb(0x202736))
        .p(px(20.))
        .child("Content follows the panel width."),
);
```

Rebuild with the same ID and a new target to start a transition. Keep IDs tied
to item identity when reordering a collection, not to its current index. Each
ID must be unique within its element scope.

The first appearance starts at its target. Later changes use a cubic ease-out
over 260 ms. `.duration(...)` changes the duration; zero disables interpolation.
`.enabled(false)` immediately adopts the target and stops requesting frames.
Re-enabling does not replay a skipped transition.

When the target changes during motion, the new transition starts at the current
interpolated rectangle. Position remains continuous; velocity may change.
Redraws are requested only while the rectangle is moving. A removed element is
removed immediately; no outgoing content or handles are retained.

## Layout contract

- The application supplies target rectangles. Flex and grid changes are not
  measured automatically.
- The container is absolutely positioned. Give the parent an explicit extent;
  it does not reserve flow space for transitioning children.
- The target controls position, width, height and minimum/maximum size. Use the
  styling API for padding, background, borders, typography and overflow. Keep
  margins and positional insets on the parent rather than this container.
- Width and height must be nonnegative and all coordinates must be finite.
- Children lay out at each intermediate size. Use normal overflow clipping when
  content should stay inside a shrinking panel. Text may change line breaks.
- Nested transitions use parent-local targets, so moving the parent does not
  independently animate an unchanged child target.

The `layout_transition` example includes card reordering, insertion/removal,
column changes and a collapsible side panel. The motion toggle demonstrates
immediate layout updates without animation.
