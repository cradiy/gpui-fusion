# Artwork flow

`artwork_flow(id, source)` renders a dark, softly layered background from an
`ImageSource`. It retains the source image's broad color regions rather than
reducing it to a palette. The stable element ID preserves preprocessing and
artwork transitions across renders.

```rust
use gpui::prelude::*;
use gpui_effects::{ArtworkFlowOptions, artwork_flow};

let background = artwork_flow("ambient-artwork", image_source)
    .options(ArtworkFlowOptions::default().brightness(0.3).motion(0.8))
    .time(elapsed_seconds)
    .size_full();
```

A new decoded image is downsampled to 48 by 48 pixels, blurred once with
premultiplied alpha, and cached. Four transformed layers are composited in one
paint effect. Changing artwork crossfades for 700 ms; another change during a
fade retains the visible image mixture. Pending image loads retain the last
prepared image.

The caller supplies elapsed seconds and schedules motion updates. Holding time
constant pauses motion; `motion(0.)` fixes the transforms. Artwork crossfades
request frames while the window is active. Offscreen windows request no frames.
For slow ambient motion, callers can use a 15 Hz timer and suspend it when paused
or hidden. Other interface animations may use their own frame scheduling.

`ArtworkFlowOptions` controls brightness, saturation, contrast, motion speed and
stable spatial dithering. Standard sizing and rounded-corner styles apply.
