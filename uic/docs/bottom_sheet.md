# Bottom sheet

Initialize UIC and mount `modal::layer(cx)` as the last child of a full-window,
relative-positioned root. Sheets share the modal layer's backdrop, focus handling,
and dismissal behavior. Showing a sheet replaces the current modal.

```rust,ignore
use gpui::{div, prelude::*, px};
use uic::components::{bottom_sheet::BottomSheet, modal};

BottomSheet::new(|_, _| {
    div().flex().flex_col().gap_3()
        .child("Choose an action")
        .child("Content can contain ordinary GPUI elements or views.")
})
.title("Actions")
.h(px(360.))
.show(window, cx);
```

The sheet sits at the bottom of the window. Its background reaches the bottom
edge while content is padded above the navigation bar or home indicator by
default. Safe-area padding accounts for space already consumed by the host.
Use `.avoid_safe_area(false)` when the application manages this space itself.
Keyboard avoidance remains enabled independently of this option.

Use normal `Styled` methods for width, height, background, text, padding, borders,
and corner radii. The default maximum width is 640 logical pixels and the default
maximum height is 90% of the available area. The body scrolls when necessary.

Drag the handle downward by 72 logical pixels to dismiss; shorter or cancelled
drags return to the resting position. Body gestures scroll the content rather
than dragging the sheet. `.drag_to_dismiss(false)` removes the handle.
Escape and backdrop clicks close the sheet unless disabled with
`.close_on_escape(false)` or `.close_on_backdrop(false)`.

For Android system Back, route the application's existing `on_system_back`
handler to `modal::dismiss(window, cx)` while a modal is open, before navigating
the underlying page. Keep `set_back_enabled` synchronized with modal and navigation
state. The component does not replace application Back callbacks.
