# Border trail

`border_trail` paints a short light segment with a fading tail along a container's
rounded edge, beneath its children. Background, base border, dimensions and corner
radii use ordinary `Styled` methods. The light does not alter layout or input handling.

```rust
use gpui::{prelude::*, px, rgb};
use gpui_effects::{BorderTrailOptions, border_trail};

let panel = border_trail(BorderTrailOptions {
    progress: 0.25,
    color: rgb(0xa2b8ff),
    ..Default::default()
})
.w(px(320.))
.h(px(180.))
.rounded(px(22.))
.bg(rgb(0x151c28))
.child("Working…");
```

## Gradient light

`BorderTrailOptions::gradient` accepts GPUI's `BorderGradient`. It supports the
same arbitrary number of color stops, custom positions, segment midpoints,
transparency, phase and interpolation color space. Reuse and clone the gradient
across frames to share its stop storage.

```rust
use gpui::{border_color_stop, border_gradient};
use gpui_effects::BorderTrailMode;

let gradient = border_gradient([
    border_color_stop(rgb(0x77dfc2), 0.),
    border_color_stop(rgb(0xad83ff), 0.4),
    border_color_stop(rgb(0xff7bad), 1.),
]);
let options = BorderTrailOptions {
    progress: 0.25,
    length: px(320.),
    mode: BorderTrailMode::Border,
    ..Default::default()
}
.gradient(gradient);
```

Gradient colors replace `color`; `gradient: None` uses the single color.

- `BorderTrailMode::Trail` distributes the gradient from head to tail. Colors
  travel with the light and retain their order when direction is reversed.
- `BorderTrailMode::Border` distributes the gradient along the complete border.
  Colors stay in place as the light passes. `base_opacity` controls the unlit
  border, while the trail raises its opacity toward `opacity`.

The gradient follows the resolved perimeter, including each corner radius. A
changed size or radius updates that mapping automatically. For a seamless full
border, place matching colors at positions zero and one.

## Motion and appearance

- `progress`: turns around the perimeter, starting at the top-left corner's top
  tangent. Values wrap, so `0` and `1` draw the same light. Increasing progress
  moves clockwise. The path uses arc length, including rounded corners.
- `length`: fading tail length in logical pixels. Default: `180 px`, capped at
  95% of the perimeter.
- `width`: light width inside the outer edge. Default: `2 px`; independent of the
  container's base border width.
- `color`: light RGBA. Default: `#A2B8FF`.
- `gradient`: optional `BorderGradient`. Default: none.
- `mode`: anchors colors to the moving trail or fixed border. Default: `Trail`.
- `base_opacity`: unlit border opacity in `Border` mode, relative to `opacity`.
  Clamped to `0..=1`; default: `0.25`.
- `opacity`: light opacity in `0..=1`. Default: `1`; zero skips painting.
- `reverse`: places the tail behind counterclockwise motion. Set it to `true`
  when decreasing progress. Default: `false`.

Animate progress using elapsed time and request frames only while active. For a
four-second loop, advance it by elapsed seconds divided by four. The component
owns no timer; holding progress keeps the light stationary. To move at the same
pixel speed on differently sized surfaces, divide traveled distance by each
surface's perimeter instead.

Zero or negative length/width and non-finite progress, length, width or opacity
disable the light. Resolved corner radii and ancestor clips apply. Single-color
light is procedural. Gradient light renders a border-only palette to an offscreen
texture and samples it through the trail; text and child content are not captured.
The palette uses GPUI's existing gradient renderer and stop buffer. Very closely
spaced stops remain limited by raster resolution. Platforms without subtree
effects show the static gradient border. To add outward glow, a separate
transparent border-trail layer can be wrapped in `subtree_bloom` behind the surface.

Run `cargo run -p gpui_effects --example border_trail` to compare rounded cards
and a pill-shaped surface, with pause, direction, moving/fixed colors, solid/gradient
and theme controls.
