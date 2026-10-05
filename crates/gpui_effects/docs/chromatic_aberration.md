# Chromatic aberration

`subtree_chromatic_aberration` separates red and blue samples in opposite
directions while keeping green and alpha at their original positions.

```rust,ignore
use gpui::{img, px};
use gpui_effects::{ChromaticAberrationOptions, subtree_chromatic_aberration};

let artwork = subtree_chromatic_aberration(
    img(cover),
    ChromaticAberrationOptions {
        amount: px(2.),
        ..Default::default()
    },
);
```

The default `Radial` mode increases separation quadratically from the capture
center toward its corners. `Directional { angle }` applies a constant offset,
with clockwise degrees measured from the horizontal. `amount` is the offset
of each outer channel, in logical pixels, clamped to 0–32. Red-to-blue
separation can therefore reach twice this value.

The filter preserves source alpha and blends uncovered offset samples back
toward the original channel. Sampling clamps to the capture's pixel centers;
the effect does not expand the silhouette or create fringes outside it.
Layout and pointer coordinates stay unchanged. Wrap only artwork to keep
surrounding labels sharp.

`EffectStage::chromatic_aberration(options)` composes with other image stages.
An active stage captures its source and performs three texture samples per
output pixel. Zero amount or non-finite settings bypass the stage. It does
not schedule animation frames.

```sh
cargo run -p gpui_effects --example chromatic_aberration
```
