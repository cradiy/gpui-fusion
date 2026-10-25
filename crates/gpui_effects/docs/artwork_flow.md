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

Use `.adaptive_layout(true)` for portrait or changing viewport aspect ratios.
The composition maps across both viewport axes. A mirrored base color field
covers the viewport throughout rotation, and overlay edges fade smoothly into it.
The default width-based composition is retained when this option is disabled.

Use `.downsample(divisor)` to reduce the source-compositing and blur-pass
resolution. The divisor is clamped to `1..=8` and defaults to one. A divisor of
four processes these stages at one quarter of each dimension. Pixel-space blur
and twist parameters follow the reduced raster density, while the final color
adjustment, dithering and rounded-edge clipping remain at native resolution.
Higher divisors trade background detail for lower texture and fill costs;
foreground content and layout are unaffected.

Artwork changes crossfade for 5/3 seconds; another change during a fade retains
the visible image mixture. Pending image loads retain the last prepared image.

The caller supplies elapsed seconds and schedules motion updates. Holding time
constant pauses motion; `motion(0.)` fixes the transforms. Artwork crossfades
request frames while the window is active. Offscreen windows request no frames.
For slow ambient motion, callers can use a 15 Hz timer and suspend it when paused
or hidden. Other interface animations may use their own frame scheduling.

`ArtworkFlowOptions` controls brightness, saturation, contrast, motion speed and
stable spatial dithering. Standard sizing and rounded-corner styles apply.

`prepare_artwork_flow(window, cx)` prepares the effect pipelines on the background
executor when the active renderer supports it. Schedule it after the initial
screen is displayed, before opening an artwork background. The returned task
reports whether preparation is supported. WGPU prepares the presentation and
premultiplied capture variants, including the capture renderer's base pipelines.
To keep early navigation responsive, display the foreground while preparation is
pending and reveal the background after the task completes. Pipelines share the
renderer's bounded device-local cache; prepare again after device recovery.
