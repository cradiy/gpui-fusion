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

A decoded image is downsampled to 40 by 40 pixels and cached without blur.
Four independently rotating layers use width ratios of 1.25, 0.8, 0.5 and 0.25;
the two smaller layers orbit the center. A radial twist remaps the composite
with radius 900 logical pixels and angle -3.25 radians. Eight ordered Kawase
passes then use diagonal offsets 5.5, 10.5, 20.5, 10.5, 40.5, 20.5, 80.5 and
40.5 logical pixels. Color adjustment follows the blur, with default saturation
2.75, contrast 1.9 and brightness 0.7. The filtered artwork is composited over
white, followed by 50% black and 5% white overlays. Dithering is disabled by default.

Artwork changes crossfade for 5/3 seconds; another change during a fade retains
the visible image mixture. Pending image loads retain the last prepared image.

The caller supplies elapsed seconds and schedules motion updates. Holding time
constant pauses motion; `motion(0.)` fixes the transforms. Artwork crossfades
request frames while the window is active. Offscreen windows request no frames.
For slow ambient motion, callers can use a 15 Hz timer and suspend it when paused
or hidden. Other interface animations may use their own frame scheduling.

`ArtworkFlowOptions` controls brightness, saturation, contrast, motion speed and
stable spatial dithering. Standard sizing and rounded-corner styles apply.
