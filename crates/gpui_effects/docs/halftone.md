# Halftone

`subtree_halftone` converts painted content into a static monochrome dot screen.
Dark regions receive larger ink dots, while light regions reveal more paper.

```rust,ignore
use gpui::{img, prelude::*, px, rgb};
use gpui_effects::{HalftoneOptions, subtree_halftone};

let artwork = subtree_halftone(
    img(cover).w_full().h(px(330.)),
    HalftoneOptions {
        spacing: px(6.),
        angle: 30.,
        strength: 1.,
        ink: rgb(0x243149),
        paper: rgb(0xf2e6cf),
    },
);
```

`spacing` is the distance between dot centers in logical pixels, clamped to
2–64. `angle` rotates the grid clockwise in degrees. `strength` blends between
the original at zero and the printed image at one; zero bypasses the effect.
Non-finite spacing, angle or strength disables the stage.

Ink and paper accept RGBA colors. With opaque colors, the source alpha is
preserved; translucent colors reduce coverage. The grid samples luminance at
each cell center and uses antialiased circular dots. It is an artistic screen,
not a color-managed printing simulation.

The effect captures its subtree and leaves layout and pointer coordinates
unchanged. Wrap only the artwork to keep surrounding text and controls crisp.
It schedules no animation. Use `EffectStage::halftone(options)` to compose it
with other subtree filters.

```sh
cargo run -p gpui_effects --example halftone
```
