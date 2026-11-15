# Liquid glass

`LiquidGlass` is a screen-space refractive material. It samples content already
painted in the same GPUI window, bends it near curved edges, and adds a narrow
reflection rim, top reflection, inner shading, and a soft drop shadow. The center
can magnify background detail. Foreground children remain sharp.

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

`optical()` is an untinted height-field lens. `frosted()` blends a light frost
over the sampled background; `dark()` uses neutral dark frost colors. `clear()`
disables background blur. These are material configurations, not automatic
foreground contrast adaptations.

| Field | Optical default | Meaning |
| --- | --- | --- |
| `blur_radius` | `2.2 px` | Maximum Gaussian background blur radius |
| `refraction` | `37 px` | Maximum inward edge displacement |
| `thickness` | `62 px` | Bevel width, limited by the local corner radius |
| `dispersion` | `0.12` | Relative red/blue displacement gain |
| `frost` | `0` | Frost coverage, from clear `0` to opaque `1` |
| `frost_center`, `frost_edge` | Light neutral colors | Frost gradient across the bevel |
| `tint` | Transparent | Color wash over the sampled material |
| `edge_color` | Transparent | Curved edge color; alpha controls strength without changing the flat center |

Lengths use logical pixels and follow the window scale factor. The height field
has a flat interior and a rounded bevel. Refraction is confined to the outer
35% of the bevel, with a smooth falloff. Scale lengths to the control size;
`refraction = 0` disables bending without removing surface lighting.

`optics` controls interior magnification, rim intensity, top reflection,
inner shading, caustic strength, and directional lighting. `center_blur` selects
how much of the Gaussian blur reaches the interior. The rim uses the full blur.
`frost` also softens the interior, suppresses refraction and reflections, and
blends `frost_center` into `frost_edge` toward the boundary. For interactive
controls, paint a resting frosted surface before labels and a clear pressed lens
after labels when the text should participate in refraction.

`shape.roundness` selects a superellipse exponent from 2 through 7.
`shape.merge` supplies a circle center relative to the pane center, radius,
and smooth-union distance, all in logical pixels. The circle and connecting
region extend paint coverage without changing layout or pointer hit regions.
Set the same `shape` on `LiquidGlassRegion` when recoloring foreground content.

`shadow` controls near and ambient drop shadows with spread, strength, and
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
frost, magnification, rim lighting, inner shading, shadow, and corner parameters. Redraws are requested
on interaction rather than by a continuous animation loop.

Backdrop sampling is limited to previously painted content in the same window;
it does not capture the desktop or another native window. Rendering requires
backend support for custom backdrop shaders. Android uses the same `LiquidGlass`
API without additional host modules or permissions. When a native WGPU window
surface cannot be copied, frames containing backdrop effects use a copyable GPU
render target and a final presentation pass. No CPU pixel readback is involved.

Each surface adds backdrop capture and filtering work; share a surface where appropriate rather than painting
many redundant overlapping materials. The frosted and clear presets share the same optical model.
