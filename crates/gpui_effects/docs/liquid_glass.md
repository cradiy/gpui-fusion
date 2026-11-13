# Liquid glass

`LiquidGlass` is a screen-space refractive material. It samples content already
painted in the same GPUI window, bends it near curved edges, and adds LCH Fresnel
reflection, directional glare, and a contour shadow. The center displays the
blurred background. Foreground children remain sharp.

The material provides no component behavior, popup management, or automatic
foreground contrast selection. It supports rounded rectangles with independent
corner radii, superellipse corners, and smooth union with an optional circle. Arbitrary path silhouettes are not supported.

## Material configuration

```rust
use gpui::{point, px};
use gpui_effects::LiquidGlassStyle;

let material = LiquidGlassStyle::frosted()
    .blur_radius(px(8.0))
    .refraction(px(18.0))
    .thickness(px(16.0))
    .dispersion(0.14);
```

`optical()` is the default untinted refractive surface, with superellipse corners, Fresnel reflection, directional glare, and a contour shadow. `frosted()` is a softened surface. `clear()` retains more background
detail with less blur and tint. `dark()` adds a dark neutral wash. These are
starting configurations, not automatic adaptations to the background.

| Field | Frosted default | Meaning |
| --- | --- | --- |
| `blur_radius` | `18 px` | Nonnegative background blur radius |
| `refraction` | `70.71 px` | Refraction distance multiplier; `0` disables bending |
| `thickness` | `20 px` | Width of the curved edge region; `0` disables curvature |
| `dispersion` | `0.14` | Relative red/blue displacement, `0..=1`; `0` disables separation |
| `tint` | White, alpha `0.06` | Color wash mixed into the sampled background |

Lengths use logical pixels and follow the window scale factor. Refraction uses
Snell's law with `optics.refractive_index`, `refraction`, and `thickness`.
A refractive index of one disables edge bending. The interior is flat. Strong edge refraction can fold background detail.

`optics` controls Fresnel range, hardness and strength, and glare range,
hardness, convergence, opposite-side strength, orientation and intensity.
The reflection colors are evaluated in LCH through sRGB/Lab conversion.
`optics.blur_edge` applies blur to the refracted rim; disabling it progressively
restores sharp samples toward the boundary. Tint alpha contributes at 80% strength.

`shape.roundness` selects a superellipse exponent from 2 through 7.
`shape.merge` supplies a circle center relative to the pane center, radius,
and smooth-union distance, all in logical pixels. The circle and connecting
region extend paint coverage without changing layout or pointer hit regions.
Set the same `shape` on `LiquidGlassRegion` when recoloring foreground content.

`shadow` controls an exponential contour shadow with spread, strength, and
translation. It is painted before glass, so refraction and blur include it.
Zero strength disables the shadow pass. Ancestor clipping applies to the pane,
merged circle, and shadow.

The background uses separable horizontal and vertical Gaussian blur with
sigma equal to one third of `blur_radius`. The material accepts radii from
0 through 200 logical pixels. WGPU uses full-resolution intermediate targets
for this kernel, allocated only when needed; this retains fine detail at small
radii but costs more texture memory than half-resolution blur. Native Metal
uses its Gaussian blur implementation with the same sigma.

Optical coordinates retain subpixel movement independently of the pixel-aligned
capture rectangle. Material effects sample previously painted content, while
children retain their own layout and sharpness.

## Paint into an existing element

`paint_liquid_glass` is independent of layout and interaction. Call it before
painting foreground content:

```rust
use gpui::{div, prelude::*, px};
use gpui_effects::{LiquidGlassStyle, paint_liquid_glass};

let material = LiquidGlassStyle::frosted();
let surface = div()
    .w(px(320.0))
    .h(px(200.0))
    .rounded(px(28.0))
    .on_paint_before_children(move |bounds, style, window, _| {
        let corners = style.corner_radii.to_pixels(window.rem_size());
        paint_liquid_glass(bounds, corners, material, window);
    })
    .child("Foreground content");
```

The painter clamps corner radii to the supplied bounds. It owns no element
state, input handling, clipping of children, or foreground styling.

## Content coloring through glass

`liquid_glass_content(content, color, region)` colors only the portions of text
and monochrome SVGs covered by a `LiquidGlassRegion`. The region uses logical
window coordinates and the same corner radii and deformation as the glass
painter. Its resolver runs during painting, after ancestor paint hooks, so it
can read the current animated region. Return `None` to render content normally.

```rust
use std::{cell::Cell, rc::Rc};
use gpui::{div, prelude::*, px, rgb};
use gpui_effects::{LiquidGlassStyle, LiquidGlassRegion, liquid_glass_content, paint_liquid_glass};

let region = Rc::new(Cell::new(None::<LiquidGlassRegion>));
let painted_region = region.clone();
let surface = div()
    .rounded(px(20.0))
    .p_4()
    .text_color(rgb(0x344a60))
    .on_paint_before_children(move |bounds, style, window, _| {
        let corners = style.corner_radii.to_pixels(window.rem_size());
        painted_region.set(Some(LiquidGlassRegion::new(bounds, corners)));
        paint_liquid_glass(bounds, corners, LiquidGlassStyle::clear(), window);
    })
    .child(liquid_glass_content("Overview", rgb(0x005cce).into(), move |_, _| region.get()));
```

Uncovered pixels retain each glyph or SVG's original color. Inside the region,
the supplied RGB replaces that color and its alpha multiplies the original
alpha. Element opacity applies once afterward. Emoji and colored images are
unaffected. This path reuses glyph/SVG atlas masks without capturing a content
texture. The mask itself changes color only. To refract the recolored content as well,
paint that content before a later `paint_liquid_glass` call. That glass then
samples the background and content together. Content painted after the glass
remains in the foreground.

## Styled surface wrapper

`LiquidGlass` supplies the same material with a Div-backed surface:

```rust
use gpui::{prelude::*, px, rgb};
use gpui_effects::{LiquidGlass, LiquidGlassStyle};

let surface = LiquidGlass::with_style(LiquidGlassStyle::clear())
    .w(px(360.0))
    .h(px(220.0))
    .rounded(px(32.0))
    .p_6()
    .text_color(rgb(0x1c3049))
    .child("Sharp foreground content");
```

Layout, typography, individual corner radii, shadows, borders, and child
clipping use `Styled`. Assign `.id(...)` before stateful interaction such as
`.on_click(...)` or `.hover(...)`. The wrapper does not add padding, radius,
foreground colors, or a drop shadow by default. A normal Div background is
painted before the material and participates in its backdrop; prefer the
material's `tint` when the goal is to color the glass itself.

## Material study

```sh
cargo run -p gpui_effects --example liquid_glass --features gpui_platform/runtime_shaders
```

The study contains an independently draggable rounded pane and circle. Their
shared distance field joins and separates continuously as either shape moves.
The checkerboard exposes edge refraction and dispersion; a gradient background
is also available. Presets adjust diffusion and tint while retaining the same
optical model and current shape configuration. Controls expose refraction,
Fresnel reflection, glare, shadow, and corner parameters. Redraws are requested
on interaction rather than by a continuous animation loop.

Backdrop sampling is limited to previously painted content in the same window;
it does not capture the desktop or another native window. Rendering requires
backend support for custom backdrop shaders. Android uses the same `LiquidGlass`
API without additional host modules or permissions. When a native WGPU window
surface cannot be copied, frames containing backdrop effects use a copyable GPU
render target and a final presentation pass. No CPU pixel readback is involved.

Each surface adds backdrop capture and filtering work; share a surface where appropriate rather than painting
many redundant overlapping materials. The frosted and clear presets share the same optical model.
