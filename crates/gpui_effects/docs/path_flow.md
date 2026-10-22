# Path flow

`FlowPath::new(points)` accepts a normalized polyline with 2–4096 points and at
least one nonzero segment. `path_flow(path, progress, options)` draws the track,
traveling light and a continuous tapered trail beneath its children. The trail
fades toward its tail and has a soft layered glow. Style the returned `Div`
normally. `PathFlowOptions` controls track color/width, light color, head
half-width (`radius`) and trail length as a fraction of the route (`trail`).

Progress is clamped to `0..=1`. Position follows distance along the route after
scaling to the surface dimensions. `FlowPath::position(progress, size)` returns
the corresponding local point for coordinating other content. Duplicate points
are skipped when measuring segments.

The caller owns the clock, looping, pause, completion and node states. The path
element never schedules redraws. Freeze progress for reduced motion and stop
requesting frames when the containing view is hidden or playback completes.
Children can cover the route, allowing markers to travel behind cards.

Run `cargo run -p gpui_effects --example path_flow` to inspect the effect.
