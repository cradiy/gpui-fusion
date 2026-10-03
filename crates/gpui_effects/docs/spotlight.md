# Spotlight

`spotlight` paints local surface light and rounded-edge illumination beneath its
children, without capturing the subtree. Style its layout, background and corner
radii with GPUI's `Styled` methods.

## Usage

```rust
use gpui::{prelude::*, point, px};
use gpui_effects::{spotlight, SpotlightOptions};

let card = spotlight(SpotlightOptions {
    center: point(0.5, 0.2),
    radius: px(180.),
    ..Default::default()
})
.w(px(280.))
.h(px(200.))
.rounded(px(20.))
.child("Library");
```

`center` uses normalized coordinates relative to the surface. The caller updates
it from pointer events and animates `strength` for entry and exit. Zero strength
skips the light draw; child content stays visible and interactive. `edge_width`
controls the inner light band independently of the styled border. The light is
clipped to the surface's rounded bounds.
