# Point gradient

`point_gradient` blends four independently positioned color sources beneath its
children. Positions use normalized surface coordinates. Each radius is measured
relative to the shorter edge, so influence remains circular on rectangular surfaces.

```rust
use gpui::{point, prelude::*, px, rgb};
use gpui_effects::{GradientPoint, point_gradient};

let surface = point_gradient([
    GradientPoint::new(point(0.15, 0.2), rgb(0xffb178)),
    GradientPoint::new(point(0.8, 0.15), rgb(0xf36c98)),
    GradientPoint::new(point(0.25, 0.85), rgb(0x6053cc)),
    GradientPoint::new(point(0.85, 0.8), rgb(0x91cce5)).radius(0.8),
])
.w_full()
.h(px(400.))
.rounded(px(24.));
```

Larger radii give a color more influence. Smaller radii produce tighter regions;
the weighted blend fills the surface rather than fading to black between points.
Colors blend with premultiplied alpha, allowing transparent points without dark
fringes. Use normal `Styled` methods for size, corners and overall opacity.

Positions are clamped to 0..=1. Positive finite radii are clamped to 0.01..=4.
Invalid positions and nonpositive or non-finite radii disable their points. If all
points are disabled or transparent, the gradient is transparent.

The component owns no timer. Update the supplied points and notify the view when
an edit occurs. Rendering uses one fragment pass without a subtree capture.

## Interactive preview

```sh
cargo run -p gpui_effects --example point_gradient
```

Drag a color handle, select a swatch, or adjust the selected point's influence
and opacity. Hide the handles to view the surface without editing controls.
