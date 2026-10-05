# Fluted glass

`fluted_glass` is a styled container that refracts previously painted window
content through a repeating rib profile. Its children render normally above
the material. It uses the shared backdrop capture and blur pipeline.

```rust
use gpui::{prelude::*, px};
use gpui_effects::{FlutedGlassOptions, fluted_glass};

let panel = fluted_glass(FlutedGlassOptions {
    spacing: px(24.),
    refraction: px(6.),
    ..Default::default()
})
.w(px(320.))
.h(px(240.))
.rounded(px(24.))
.p_6()
.child("A different lens.");
```

| Option | Default | Meaning |
| --- | --- | --- |
| `spacing` | 24 px | Rib pitch in logical pixels, clamped to 4–128 |
| `angle` | 0° | Clockwise direction from vertical; 90° produces horizontal ribs |
| `refraction` | 6 px | Maximum background displacement, clamped to 0–45% of the pitch |
| `blur_radius` | 3 px | Background blur radius, clamped to 0–64 |
| `clarity` | 0.8 | Sharp contribution, clamped to 0–1 |
| `tint` | White at 7% | Color wash mixed into the background |
| `highlight` | 0.12 | Rib lighting strength, clamped to 0–1 |

Zero refraction produces flat glass without rib lighting. Strong refraction can
fold and repeat background detail. Compressed regions reduce sharp sampling in
favor of the blurred backdrop; use nonzero blur for smoother results.
Non-finite spacing uses 24 px; other non-finite scalar controls use zero.

Sizing, foreground styling, borders, corner radii and interaction use standard
Div methods. The rib pattern stays in local surface coordinates and schedules
no animation. Paint the background first and place the glass over it; content
in another window is not sampled.

```sh
cargo run -p gpui_effects --example fluted_glass
```
