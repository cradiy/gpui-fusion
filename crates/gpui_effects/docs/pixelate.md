# Pixelate

`subtree_pixelate` samples the center of each square cell to produce a blocky
image. The grid starts at the capture's top-left corner. Partial cells at the
right and bottom sample the center of their remaining area.

```rust,ignore
use gpui::{img, px};
use gpui_effects::{PixelateOptions, subtree_pixelate};

let artwork = subtree_pixelate(img(cover), PixelateOptions {
    cell_size: px(16.),
    strength: 1.,
});
```

`cell_size` uses logical pixels and is clamped to 1–128. Sizes at or below one,
zero strength, or non-finite settings bypass the stage. `strength` blends from
the original at zero to the sampled cells at one using premultiplied colors.
Cells sample both color and alpha, so transparent silhouettes become blocky
within the capture bounds. This is center sampling, not a cell-average filter.

`EffectStage::pixelate(options)` can be combined with other subtree stages.
The filter leaves layout and pointer geometry unchanged and does not request
animation frames. Applications can animate cell size and strength together,
fading strength to zero when returning to the original image. A changing cell
size moves sampling points and can produce visible steps on fine details.

```sh
cargo run -p gpui_effects --example pixelate
```

Hold the preview button to increase pixelation and release to restore the
original. The pinned study remains pixelated for comparison. Controls set the
maximum cell size and blend strength.
