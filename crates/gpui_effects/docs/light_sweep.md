# Light sweep

`light_sweep` paints a soft moving highlight over a container's background and
beneath its children. Text remains sharp and child controls retain normal input
handling. Layout, background, border and corner radii use GPUI's `Styled` methods.

## Usage

```rust
use gpui::{prelude::*, px, rgb};
use gpui_effects::{LightSweepOptions, light_sweep};

let card = light_sweep(LightSweepOptions {
    progress: 0.5,
    ..Default::default()
})
.w(px(280.))
.h(px(200.))
.rounded(px(20.))
.bg(rgb(0x151a24))
.child("Ready to publish");
```

## Configuration

- `progress`: position along the sweep, clamped to `0..=1`. Both endpoints are
  fully outside the surface and skip painting. Default: `0`.
- `angle`: direction of travel in degrees, clockwise from rightward motion.
  `0` moves right, `90` moves down and `180` moves left. Default: `20`.
- `width`: full light-band width perpendicular to the band, in logical pixels.
  Default: `100 px`.
- `color`: light color, including alpha. Default: white.
- `opacity`: additional light opacity, clamped to `0..=1`. Default: `0.18`.

Zero or negative width, non-finite progress, angle, width or opacity disables
the light. The band respects the container's rounded corners and ancestor clips.
It uses a procedural draw without capturing or blurring child content.

## Animation

Animate `progress` from `0` to `1` using elapsed time, then stop requesting
animation frames. The component owns no timer. For a repeating sweep, the
application controls both the sweep duration and the pause between passes.
Reversing progress reverses motion. Keep progress at an endpoint to disable
motion while retaining the normal surface.

The `light_sweep` example demonstrates hover-triggered passes, replay, and light
and dark surfaces.
