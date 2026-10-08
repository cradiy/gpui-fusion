# Swipe actions

`SwipeActions` triggers an action when the user releases a horizontal drag past
its distance threshold. Keep one `SwipeActionsState` per stable row identity.

```rust,ignore
use uic::components::swipe_actions::{
    SwipeActions, SwipeActionsState, SwipeDirection, SwipeTriggered,
};

let state = cx.new(|cx| SwipeActionsState::new(window, cx));
let subscription = cx.subscribe(&state, |this, _, event: &SwipeTriggered, cx| {
    match event.direction {
        SwipeDirection::Left => this.archive_document(cx),
        SwipeDirection::Right => this.toggle_pin(cx),
    }
});
// Retain both state and subscription in the view.

SwipeActions::new("document", &state, document_row)
    .rounded_lg()
    .threshold(px(96.))
    .feedback(|swipe, _, _| {
        let left = swipe.direction == SwipeDirection::Left;
        div().size_full().px_4().flex().items_center()
            .when(left, |row| row.justify_end())
            .bg(rgb(0x347b79)).text_color(rgb(0xffffff))
            .child(if swipe.ready { "Release" } else { "Swipe" })
    })
```

Directions describe the user's movement: `Left` means dragging the content to
the left. Both directions are enabled by default. Use `left_enabled(false)` or
`right_enabled(false)` to leave a direction unhandled. The threshold defaults to
96 logical pixels; finite values are clamped to at least 16 pixels and nonfinite
values use the default.

The threshold controls when release triggers the action, not how far the row can
move. Content follows horizontal movement without a distance cap and can slide
fully out of the row. `SwipeProgress::displacement` gives the signed distance for
positioning feedback within the revealed area.

The content controls row height. The component implements `Styled` for layout,
background, rounding and typography. Give the content an opaque background when
using feedback. The optional feedback element fills the area behind the content
and should contain only noninteractive indicators. `SwipeProgress` reports the
direction, distance progress from zero to one, and whether release would trigger
an action. Feedback also renders during the return animation, with `ready` false.

An action fires once on release, never on crossing the threshold while dragging.
Moving back below the threshold cancels it. Short flicks, cancelled touches,
long presses and additional fingers do not trigger actions. By default the row
returns to its resting position after release.

For an operation that removes a row, mark it dismissed in application state and
keep rendering it with the same ID:

```rust,ignore
SwipeActions::new(("document", document.id), &document.swipe, document_row)
    .mb_3()
    .dismissed(document.archived.then_some(SwipeDirection::Left))
```

The content slides out from its current position over 210 ms, then its occupied
height collapses over 200 ms. Put list spacing in the component's margin, rather
than a parent `gap`, so spacing collapses along with the row. A fully dismissed
row has no layout or hitboxes. Do not filter it out of the rendered list before
the animation completes. Setting `dismissed(None)` restores the row with an
expansion animation. The component does not perform file operations or own the
list. Offer the same operations through an accessible menu or keyboard commands.

Horizontal movement claims the gesture after direction detection. Vertical
movement stays available for scrolling and cannot become a swipe later in the
same gesture. Descendants may prevent the initial event to reserve their own
interaction. A claimed drag suppresses row clicks.

Touch interaction requires a host that dispatches raw GPUI touch events. Android
suppresses native scrolling, taps and long presses once the row claims the
gesture. Mouse dragging is supported; wheels and trackpads retain normal scroll
behavior.

Run `cargo run -p uic --example swipe_actions` to try swiping left to archive and
right to toggle pinning. Restore archived rows with the example's restore control.
