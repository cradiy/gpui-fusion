# Frosted glass

[`FrostedGlass`](crate::FrostedGlass) is a layout-neutral container that blurs
content already painted behind it. Its children are painted afterward, so
labels and icons stay sharp.

For a normal panel, size and round it with the standard GPUI style API:

```rust,ignore
use gpui::{hsla, px, rgb};
use gpui_effects::{FrostedGlass, FrostedGlassAppearance};

FrostedGlass::with_appearance(
    FrostedGlassAppearance::dark()
        .blur_radius(px(12.0))
        .tint(hsla(0.61, 0.30, 0.12, 0.38)),
)
    .w(px(420.0))
    .p_4()
    .rounded(px(24.0))
    .text_color(rgb(0xf8fafc))
    .child("Sharp foreground content")
```

The default single-panel shape follows the element bounds. To join two local
rounded shapes, define them in the coordinate space of a containing field:

```rust,ignore
use gpui::{point, px, size};
use gpui_effects::{FrostedGlass, FrostedGlassShape};

let toolbar = FrostedGlassShape::new(
    point(px(220.0), px(80.0)),
    size(px(320.0), px(88.0)),
    px(44.0),
);
let button = FrostedGlassShape::new(
    point(px(430.0), px(80.0)),
    size(px(88.0), px(88.0)),
    px(44.0),
);

FrostedGlass::new()
    .merge(toolbar, button)
    .relative()
    .w(px(640.0))
    .h(px(180.0))
```

Use `FrostedGlassAppearance::dark()` or `::light()` as a starting point, or
construct the public appearance struct directly. `blur_radius` controls the
renderer’s separable Gaussian blur; `sheen` controls the subtle glass surface;
`merge_distance` controls how close two shapes must be before their masks join.

`FrostedGlassAppearance` contains only glass-material parameters. Use the
normal `Styled` methods for shared element styling, including `text_color`,
`opacity`, `border`, `shadow`, sizing, spacing, flex layout, and typography.

Run the interactive example from the workspace root:

```sh
cargo run -p gpui_effects --example frosted_glass
```
