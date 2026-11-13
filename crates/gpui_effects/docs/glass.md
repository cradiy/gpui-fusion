# Frosted glass

`LiquidGlass` renders clear and frosted surfaces with one optical model.
`LiquidGlassStyle::frosted()` diffuses background detail, while `dark()`
adds a dark neutral tint. Both preserve Snell refraction, chromatic dispersion,
Fresnel reflection, directional glare, and contour shadows.

The backdrop uses separable horizontal and vertical Gaussian blur. The radius
is the three-sigma support: `sigma = blur_radius / 3`. WGPU evaluates this kernel
at full resolution to preserve small-radius detail; Metal uses its native
Gaussian filter with the same sigma. Intermediate textures are allocated on
demand and are larger than half-resolution blur targets.

```rust,ignore
use gpui::{prelude::*, px, rgb};
use gpui_effects::{LiquidGlass, LiquidGlassStyle};

LiquidGlass::with_style(LiquidGlassStyle::frosted())
    .w(px(420.0))
    .p_4()
    .rounded(px(24.0))
    .text_color(rgb(0x172430))
    .child("Sharp foreground content")
```

Children retain normal GPUI layout and remain sharp. Layout, typography,
foreground color, borders, and opacity use `Styled`.

For a merging circle, set `material.shape.merge` to `Some(LiquidGlassMerge)`.
Its `offset` is relative to the pane's center; `radius` controls the circle,
and `smoothness` controls the joining distance. Updating the offset produces
continuous joining and separation. The pane and circle share a single optical
pass rather than compositing translucent surfaces over each other. Place
interactive foreground elements over the two regions to control their motion.

Run the interactive example:

```sh
cargo run -p gpui_effects --example frosted_glass
```
