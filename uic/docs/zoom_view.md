# ZoomView

`ZoomView` displays bounded preview content with pointer-centered zoom and pan.
Keep one `ZoomState` for each preview whose position should survive navigation.

```rust
use uic::components::zoom_view::{ZoomState, ZoomView};

let preview = cx.new(|cx| {
    ZoomState::new(size(px(1200.), px(800.)), window, cx)
});

// In render; image may be any GPUI ImageSource.
ZoomView::new("preview", &preview, img(image).size_full())
    .w_full()
    .h(px(480.))
    .rounded_xl()
    .bg(rgb(0x16212e));
```

The positive, finite content dimensions define the fitted rectangle's aspect
ratio. Content is laid out inside that rectangle at fit size, then the complete
preview is magnified. Use `.size_full()` on an image or other content that should
fill the rectangle. Fixed-size children must fit that initial layout; this is not
an unbounded canvas or a document virtualization mechanism.

Use `Styled` for viewport dimensions, padding, background, border and corner
radius. Supply a finite viewport, either with an explicit height or a bounded
`.flex_1().min_h_0()` area. The viewport clips magnified content and preserves its
corner radii. Retain interactive child entities outside the preview's render path.

## Navigation

- Wheel and trackpad pinch zoom around the pointer or gesture center.
- Touch supports two-finger scaling and translation, including adding a second
  finger after starting a pan.
- Double-click or double-tap switches between 2.5 times fit and fit.
- At zoom greater than 1, one-finger or left-button dragging pans the content.
- Bounds keep large content covering the viewport and center an axis that is
  smaller than the viewport.

`zoom()` is relative to fit: 1 means the whole content is visible, not native image
resolution. The default maximum is 8. Programmatic changes are immediate:

```rust
preview.update(cx, |state, cx| {
    state.zoom_to(2.0, cx); // Zoom around the viewport center.
    // state.reset(cx); // Fit and center.
    // state.set_max_zoom(4.0, cx);
    // state.set_content_size(size(px(1600.), px(900.)), cx);
});
```

`set_content_size` refits when dimensions change. `offset()` reports pan in logical
viewport pixels. Observe the state to update a toolbar or zoom indicator. Resizing
recalculates fit and constrains the current pan. Touch cancellation, a third
finger, or window deactivation ends the active manipulation. After a multi-touch
gesture, lift all fingers before starting another pan.

Use `.wheel_zoom(false)` when wheel scrolling should remain with surrounding
content. Preview children that consume the initiating input take priority.

## Pager integration

Place a `ZoomView` in each `Pager` page and retain separate zoom states. At fit,
horizontal dragging remains available to `Pager`. When zoomed, dragging belongs
to the preview even at its pan boundary; reset to fit to swipe between pages.
Multi-touch zoom cancels an in-progress page drag.

The `zoom_view` example combines `Tabs`, `Pager` and two independently retained
previews:

```sh
cargo run -p uic --example zoom_view
```

## Rendering

ZoomView uses `gpui_effects::transform_group` with automatic raster density. It
inherits that effect's capture size and GPU memory limits; large magnifications
can become softer once raster density reaches its budget. A backend without
subtree effects displays the fitted content and leaves zoom disabled. ZoomView
does not load files or manage an image gallery's data.
