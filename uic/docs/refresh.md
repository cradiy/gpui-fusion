# Pull to refresh

`RefreshContainer` owns a vertical scroll viewport. Give it a bounded height,
or use `flex_1().min_h_0()` in a column with a bounded height. Add list rows as
children; do not wrap them in another vertical scroll viewport.

Create one state per viewport and retain it across renders:

```rust
let refresh = cx.new(|cx| RefreshState::new(window, cx));
```

Subscribe to `RefreshRequested` to perform application work. The state enters
`Refreshing` before emitting the event and ignores further requests until the
application calls `finish(cx)`. Call `finish` after both success and failure;
the application owns errors, retries, and cancellation.

```rust
RefreshContainer::new("documents", &refresh)
    .flex_1()
    .min_h_0()
    .rounded_lg()
    .bg(surface)
    .children(rows)
```

On completion, update the displayed data or error and end the refresh:

```rust
refresh.update(cx, |state, cx| state.finish(cx));
```

The outer surface implements `Styled`. Indicator content inherits typography
and can be replaced with `.indicator(|status| ...)`; `RefreshStatus` exposes
pull progress, release readiness, and pending refresh. The default indicator
uses text. `state.scroll_handle()` exposes the list's scroll position.

Pulling starts only when a single touch begins at the top of the viewport.
Horizontal gestures, upward scrolling, and multi-touch do not trigger refresh.
Releasing below the threshold or cancelling returns the indicator without
emitting a request. The list remains scrollable during refresh. Pulls have
resistance, and the indicator settles when released or completed.

Mouse wheels and trackpads retain normal scrolling behavior. Provide a refresh
button or action for keyboard and accessibility users and call
`state.request(cx)` from it. A raw-touch-capable host is required for pulling;
the Android host suppresses synthesized scrolling and clicks once the gesture
is claimed. Nested Android-native scrolling is not provided by this component.

Run `cargo run -p uic --example refresh` for the desktop example. The Android
example's Files section includes a Documents sheet with a refreshable list.
The [desktop example](../examples/refresh.rs) includes event subscription and
asynchronous completion with weak entity handles.
