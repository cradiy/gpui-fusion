# Scroll reveal

`scroll_reveal` fades and slides a styled container into view. Place it inside a
scrollable parent and keep its ID stable across renders.

```rust,ignore
use gpui::{div, point, prelude::*, px, rgb};
use gpui_effects::scroll_reveal;

scroll_reveal(("card", index))
    .w_full().p(px(24.)).rounded(px(20.))
    .bg(rgb(0x1c293e))
    .threshold(0.3)
    .offset(point(px(0.), px(24.)))
    .child("A thought at a time")
```

## Visibility and playback

The trigger compares the container's unanimated layout area with the intersection
of the window viewport and ancestor content clips. The default threshold is 15%.
`.threshold(...)` accepts a fraction in `0..=1`; zero still requires a positive
visible area. Large elements need a threshold they can reach inside the clip.
Rounded clip corners, sibling occlusion and captured transforms do not participate
in the rectangular visibility calculation.

The default entrance lasts 360 ms with cubic smoothstep easing, no delay and an
18 logical-pixel downward starting offset. Set `.duration(...)`, `.delay(...)`
and `.offset(...)` to customize it. Duration and delay are sampled at the start
of each entrance. Layout establishes visibility; playback begins on the next
frame without first displaying the final content.

By default, an entrance starts only once per mounted element ID. `.once(false)`
rearms it after the layout bounds leave the visible clip completely. Falling
below the threshold during playback does not restart it. An entrance that leaves
the clip in once mode keeps elapsed time and may be complete when it returns.
Changing the ID starts a fresh lifecycle.

`.enabled(false)` or zero duration shows content immediately, without offset or
delay. Visible content remains revealed when motion is re-enabled. Supply the
application's reduced-motion preference through this option.

## Layout and interaction

The container always keeps its normal layout size. Its visual offset does not
move siblings or alter the trigger area. Explicit container opacity multiplies
the reveal opacity. Attach event handlers to child elements; ordinary mouse
input except scrolling is blocked within the container while entering. The
application owns keyboard focus and global listeners.

Children are still constructed and laid out while unrevealed. Their prepaint
and paint are skipped when fully clipped or at zero reveal progress. The wrapper
requests animation frames only while visible and waiting or animating; it does
not poll offscreen content. This is not a virtual list or lazy-loading mechanism.
An ancestor that unmounts or skips the element's lifecycle cannot provide exit
observations or preserve its reveal history; virtual lists should own that state
separately when necessary.

```sh
cargo run -p gpui_effects --example scroll_reveal
```
