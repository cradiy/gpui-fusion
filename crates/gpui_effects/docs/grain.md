# Surface grain

`surface_grain` paints a static texture above a styled container's background and
below its children. Text and icons retain their normal rendering.

```rust
use gpui::{prelude::*, px, rgb};
use gpui_effects::{GrainOptions, surface_grain};

let panel = surface_grain(GrainOptions {
    size: px(0.75),
    strength: 0.12,
    colored: false,
})
.p_6()
.rounded(px(24.))
.bg(rgb(0x202b38))
.child("Room to breathe.");
```

Standard styling controls layout, background, border and corner radii. The
texture follows the resolved corners. It can sit over a solid or gradient
background; over transparent areas, the grain itself remains visible.

`size` is the cell size in logical pixels, clamped to 0.5–8 and at least one
device pixel. `strength` is the maximum overlay opacity, clamped to 0–1. A value
of zero skips the effect. Non-finite sizes or strengths disable it.
`colored` selects independent RGB noise instead of monochrome noise.

The overlay mixes light and dark flecks into the surface and can slightly
change its average tone. It does not sample or blur the background. Its pattern
is fixed in local coordinates, uses no image assets, and schedules no animation
frames or subtree captures.

Run the comparison:

```sh
cargo run -p gpui_effects --example surface_grain
```
