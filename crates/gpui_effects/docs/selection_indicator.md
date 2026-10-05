# Selection indicator

`selection_indicator` measures keyed items and animates a decoration behind the
selected item. Items keep their normal layout and input handling throughout the
transition. The group supports standard `Styled` layout and appearance methods.

```rust,ignore
use gpui::{div, prelude::*, px, rgb};
use gpui_effects::selection_indicator;

let mut tabs = selection_indicator(
    "tabs",
    div().size_full().rounded(px(12.)).bg(rgb(0x344762)),
)
.selected(selected_id)
.flex().gap(px(4.)).p(px(6.))
.rounded(px(18.)).bg(rgb(0x182230));

for (id, label) in items {
    tabs = tabs.item(id, div().id("tab").px(px(16.)).py(px(10.)).child(label));
}
```

Keys must be unique within the group and stable across renders. Supply click
handlers, selected text styles, focus and keyboard navigation on the items.
The component does not select items in response to input or assign tab semantics.

The decoration fills the selected item's border box by default. Use
`.inset(px(2.))` to leave space on all sides, or `.underline(px(3.))` to use a
bottom line of the specified thickness. Insets apply before the underline is
positioned. Insets and thickness must be finite and nonnegative and are capped
to fit the item's bounds. Set color and corner radius on the decoration itself;
use `.size_full()` to fill its animated area. Keep it decorative and keep item
backgrounds transparent or translucent so they do not cover it.

The first valid selection appears immediately. Changing the selection, item
width, layout or order retargets from the displayed rectangle. The default
duration is 260 ms with cubic ease-out; use `.duration(...)` to change it.
Rapid switches preserve position and size continuity, but not velocity.
Parent movement and scrolling carry the group and decoration together.

Omitting `.selected(...)`, selecting a missing key, or selecting an item with
zero layout size hides the decoration immediately and clears its motion state.
The next valid selection appears immediately. `.enabled(false)` and zero duration
snap to the current target. There are no animation requests at rest.

Items participate directly in the group's flex/grid layout. Measurement uses
the current frame's layout bounds; it does not track separate visual transforms
inside an item or deferred overlays. The decoration paints behind the items
inside the same container clipping and opacity. This component performs no
texture capture and does not animate item layout or retain outgoing items.

```sh
cargo run -p gpui_effects --example selection_indicator
```
