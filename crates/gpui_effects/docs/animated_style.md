# Animated styles

`animated_style` is a styled, interactive container that interpolates selected
paint properties when their target values change. Use ordinary GPUI `Styled`
methods, child elements and event handlers.

```rust,ignore
use gpui::{div, prelude::*, px, rgb};
use gpui_effects::animated_style;

animated_style("action")
    .px(px(18.)).py(px(12.))
    .bg(rgb(if hovered { 0x344762 } else { 0x202736 }))
    .text_color(rgb(if hovered { 0xffffff } else { 0xb5c3d7 }))
    .border_1().border_color(rgb(if selected { 0x77dfc2 } else { 0x344762 }))
    .rounded(px(if selected { 18. } else { 10. }))
    .opacity(if disabled { 0.45 } else { 1.0 })
    .enabled(motion_enabled)
    .child("Create a note")
```

Keep a stable ID across renders. The first appearance uses the target immediately.
Later changes use a 180 ms cubic smoothstep transition by default. Set
`.duration(...)` to adjust timing. Interruptions start from the currently displayed
values, preserving continuity but not velocity. An unrelated redraw or layout
change does not restart the paint transition.

## Supported properties

- Solid `.bg(...)` colors.
- The base `.border_color(...)` and inherited `.text_color(...)`.
- Each corner radius, including pixel and rem lengths. Rem lengths are resolved
  to logical pixels for interpolation using the current root font size.
- `.opacity(...)`, including descendant content as in a normal GPUI container.

Colors interpolate in premultiplied sRGB, so fully transparent RGB does not tint
a fade. Radii and opacity interpolate numerically. Final values retain the exact
target style. Use finite values, nonnegative radii and opacity in `0..=1`.

Only properties explicitly set at both endpoints animate. Adding or removing a
property applies it immediately, preserving normal inheritance. To fade a
background in, set an explicit transparent color in the initial state; to fade
opacity out, explicitly use `1.0` in the initial state. Gradients, patterns,
per-edge border overrides, shadows, typography metrics and layout properties
apply immediately. They do not restart an otherwise unchanged paint transition.

## Interaction and motion

The application owns hovered, selected, pressed and disabled state. Use
`.on_hover(...)` to update application state and provide new base styles on the
next render. Native `.hover(...)`, `.active(...)`, focus and group refinements
still apply immediately; the component does not intercept those refinements.
Child styles can override inherited text colors as usual.

Input remains enabled during animation. Opacity does not disable clicks or hide
content from accessibility: implement disabled behavior and focus handling in
the application. The component does not scale or capture its content, and its
children retain normal layout and element identity.

`.enabled(false)` or zero duration snaps to the target without replaying skipped
motion when re-enabled. No animation frames are requested once settled.

```sh
cargo run -p gpui_effects --example animated_style
```
