# Gradient map

`subtree_gradient_map` maps source luminance to a GPUI color ramp. Black maps to
the first color, white to the last, and intermediate values sample the ramp.
The luminance calculation uses the source's display RGB with weights
0.2126, 0.7152 and 0.0722.

```rust,ignore
use gpui::{img, linear_color_stop, prelude::*, px, rgb};
use gpui_effects::{GradientMapPalette, subtree_gradient_map};

let palette = GradientMapPalette::new([
    linear_color_stop(rgb(0x18223c), 0.),
    linear_color_stop(rgb(0xb75998), 0.5),
    linear_color_stop(rgb(0xf6dfb1), 1.),
]);
let artwork = subtree_gradient_map(img(cover).w_full().h(px(300.)), palette)
    .strength(0.8);
```

The palette uses `LinearColorStop` and GPUI's standard gradient storage and
interpolation. It requires at least two stops, ordered within 0–1; equal
positions are allowed. It imposes no separate stop-count limit. Use
`.color_space(...)` on the palette to select GPUI's interpolation space.

Keep the palette in view state. `set_stop(index, stop)` updates a stop through
GPUI's copy-on-write gradient path; positions must remain between neighboring
stops. Notify the view to display edits. `background()` supplies the same ramp
for an editor's color strip. Construct a new palette when adding or removing stops.

`strength` blends from the original at zero to the mapped colors at one.
Opaque stops preserve source alpha. Translucent stops multiply source alpha,
and blending uses premultiplied colors to avoid transparent-color fringes.
Zero or non-finite strength bypasses capture. Unsupported subtree backends
paint the original content.

The wrapper retains the child's layout, pointer and accessibility geometry.
Only wrap the artwork to keep surrounding text unchanged. It captures the
source and a horizontal GPU-rendered ramp as two equally sized textures.
Intermediate ramp precision follows capture width; close stops and hard edges
are filtered at that resolution. It schedules no animation frames.

```sh
cargo run -p gpui_effects --example gradient_map
```
