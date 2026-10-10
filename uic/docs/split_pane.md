# Split pane

`SplitPane` divides a bounded area into two panes with a draggable divider. Each
pane accepts any GPUI element, including another `SplitPane` for nested layouts.

```rust
use gpui::{prelude::*, *};
use uic::components::split_pane::{SplitPane, SplitPaneEvent, SplitPaneState};

let split = cx.new(|cx| SplitPaneState::new(0.3, window, cx));

SplitPane::new(&split, sidebar, content)
    .first_min_size(px(120.))
    .first_max_size(px(320.))
    .second_min_size(px(200.))
    .label("Sidebar width")
    .size_full();
```

Retain each state for one split in one window. The default horizontal axis places
panes side by side. `.axis(Axis::Vertical)` places the first pane above the second.
Give the split a definite size or a parent that supplies one; child content does
not determine the split's intrinsic size. Put scrolling inside the pane content.

## Size constraints

The ratio is the first pane's share of available space, excluding the divider.
It must be finite and between 0 and 1. `ratio()` returns the preferred ratio;
layout clamps actual sizes without overwriting it, so shrinking and enlarging
the window can restore the preferred layout.

Each pane defaults to a zero minimum and no maximum. Use `first_min_size`,
`first_max_size`, `second_min_size` and `second_max_size` to constrain its size
along the split axis. Sizes must be nonnegative and maximum must be at least minimum.

Both panes always fill the available space. When their minima cannot fit, space
is divided proportionally to the minima. When both maxima together cannot fill
the container, they become lower bounds and the preferred ratio determines the
remaining distribution. The divider itself shrinks if the container is smaller
than its handle size. Pane content is clipped to its assigned area.

## Interaction and persistence

Drag the divider with a mouse or one touch. Touches starting on the divider are
reserved for resizing; touches in pane content keep their normal behavior.
`handle_size` controls both its reserved space and interaction target (default 8 px).
Use a larger handle on touch-oriented layouts.

Double-click the divider or press Enter while it is focused to restore the initial
ratio. Arrow keys along the split axis move by 8 px, or 32 px with Shift. Home/End
move to the allowed limits. Escape cancels a drag. Touch cancellation, window
deactivation, and a change to axis or available size also cancel the current drag.

Subscribe to state events to persist user adjustments:

```rust
let subscription = cx.subscribe(&split, |this, _, event: &SplitPaneEvent, cx| {
    if let SplitPaneEvent::Changed(ratio) = event {
        this.saved_ratio = *ratio;
        cx.notify();
    }
});
// Retain the subscription in your view.
```

`Changing` reports pointer previews. `Changed` reports a completed drag or a
keyboard, accessibility or reset adjustment when the ratio actually changes.
Cancelled drags restore the original ratio without emitting `Changed`.
`set_ratio(ratio, cx)` restores a saved preference without emitting interaction
events; `reset(cx)` restores the constructor ratio.

## Styling

Styled methods configure the outer surface, including size, padding, background,
border, corner radius and inherited text style. Style each pane's content directly.
`SplitPaneAppearance` supplies divider, active/hover and focus colors. Hovering or
dragging highlights only the short grip, leaving the divider background transparent. The divider
exposes an accessible label, orientation, current size and allowed interval, with
increment/decrement actions.

```sh
cargo run -p uic --example split_pane
```
