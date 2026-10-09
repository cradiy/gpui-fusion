# ReorderableList

`ReorderableList` is a scrollable vertical list with touch long-press sorting,
mouse drag handles, animated placement and edge scrolling. Keep one
`ReorderState` per list and stable, unique IDs for its items.

```rust
use uic::components::reorderable_list::{ReorderHandle, ReorderState, ReorderableList};

// During view initialization:
let sorting = cx.new(|cx| ReorderState::new(window, cx));

// In render:
let mut list = ReorderableList::new("queue", &self.sorting)
    .flex_1()
    .min_h_0()
    .gap_3()
    .on_reorder(cx.listener(|this, event, _, cx| {
        let item = this.items.remove(event.from);
        this.items.insert(event.to, item);
        cx.notify();
    }));

for item in &self.items {
    list = list.item(
        item.id,
        div().w_full().flex().items_center()
            .child(div().flex_1().child(item.title.clone()))
            .child(ReorderHandle::new(&self.sorting, item.id, "⠿").p_3()),
    );
}
```

The application owns the item order. `on_reorder` runs once when a drag ends at a
different index. Its `id` identifies the moved item; remove `from` first, then
insert at `to`. Apply the move synchronously and notify the view to accept it.
Ignoring a proposal restores the original order. Item IDs identify data, not
positions; do not use the current index as an ID for mutable lists.

Rows may have different heights. Use normal `Styled` methods for the viewport's
size, padding, gap, background, border and typography. Give it a bounded height
or a bounded `.flex_1().min_h_0()` area. Place row styling inside each item's
content. The list owns its vertical layout and scrolling; do not wrap it in a
second vertical scroller. All rows are constructed, so this component is intended
for moderate lists rather than virtualized datasets.

## Interaction

Touch scrolling works normally until a stationary long press claims a row.
`ReorderHandle` starts mouse dragging immediately without making the entire row
a mouse drag target. Long press respects child controls that consume the event,
including text selection and context menus. Avoid a consuming child at every
possible long-press location if the row needs to be sortable on touch.

During sorting, neighboring rows move aside and the active row follows the
pointer. Holding near the viewport's top or bottom scrolls at up to 600 logical
pixels per second. Releasing settles into the accepted position. Reduced-motion
preferences disable placement interpolation without disabling direct dragging.

Escape, touch cancellation, adding another finger, window deactivation, changing
item IDs/order, or `.enabled(false)` cancels the current move. `state.cancel(cx)`
also cancels it. `dragged_item()` can style the active row;
`scroll_handle()` provides programmatic scrolling. Applications can expose
additional move-up/down controls for keyboard and accessibility operation using
the same data update as their reorder callback.

## SwipeActions integration

Place a retained `SwipeActions` inside each keyed row:

```rust
list = list.item(
    item.id,
    SwipeActions::new(("swipe", item.id), &item.swipe, row_content)
        .feedback(render_swipe_feedback),
);
```

Keep each swipe state with the item's stable identity when reordering. Horizontal
swipes trigger the row's actions; vertical movement before long press scrolls;
after long press, the list owns movement and release so no swipe action fires.
A mouse handle likewise takes priority over the row's swipe gesture. Sorting
does not decide what a swipe action does or mutate application data.

Run the combined example:

```sh
cargo run -p uic --example reorderable_list
```
