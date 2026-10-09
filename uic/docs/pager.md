# Pager

`Pager` displays arbitrary page content in a horizontal viewport. Users drag with
one finger or the left mouse button; release selects the adjacent page when the
swipe covers a quarter of the viewport or has sufficient velocity. Short drags
return to the selected page. Each gesture advances at most one page.

```rust
use uic::components::pager::{PageChanged, Pager, PagerState};

// Retain the state and page entities in your view.
let pager = cx.new(|cx| PagerState::new(pages.len(), window, cx));

// In render: provide a bounded viewport through ordinary Styled methods.
let pages = pages.clone();
Pager::new("pages", &pager, move |index, _, _| pages[index].clone())
    .w_full()
    .h(px(400.))
    .rounded_xl();
```

Only pages intersecting the viewport are rendered, normally one at rest and two
while moving. The page callback may run on every animation frame. Keep persistent
page entities, input state and scroll handles outside the callback. Page indices
identify slots; the application manages identity when reordering its data.

## Navigation

```rust
pager.update(cx, |state, cx| {
    state.scroll_to(2, cx); // Animate to page 3.
    // state.jump_to(2, cx); // Select immediately.
});
```

Both methods return `false` for an out-of-range index. `current_page()` returns the
selected destination, or `None` when empty. `PageChanged { page }` is emitted when
that selection changes, including programmatic navigation; it is not an animation
completion event. Subscribe to it to update a separately rendered tab bar or page
indicator. Calling `scroll_to` during an animation continues from its displayed
position.

Use [Tabs](tabs.md) for a keyboard-accessible tab bar; the interactive example
connects its selection callback to `scroll_to`.

`set_page_count(count, cx)` updates the number of pages, clamps the selection and
cancels motion. For a resizable application, pages follow the viewport dimensions;
resizing during a drag cancels that gesture.

## Interaction

Vertical gestures remain available to scrollable page content. Descendants that
consume the initiating gesture take precedence. Once horizontal dragging wins,
it suppresses the corresponding click. Cancellation, long press or a second
finger returns to the selected page without committing a page change.

Use `.drag_enabled(false)` when the page needs exclusive horizontal interaction;
programmatic navigation remains available. Mouse-wheel scrolling stays with the
page content. There is no automatic cycling or wraparound. When the system
requests reduced motion, release and programmatic navigation settle immediately.

The root uses `Styled` for sizing, padding, typography, backgrounds, borders and
corner radius. Each page fills the viewport inside that padding. Give the pager
an explicit height or a bounded flex area such as `.flex_1().min_h_0()`.

Run the interactive example:

```sh
cargo run -p uic --example pager
```
