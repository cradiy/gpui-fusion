# gpui_effects

GPU-driven visual effects and reusable effect components for GPUI applications.
WGSL is the canonical shader implementation, with GPUI providing the render
pipeline and `gpui_effects` providing higher-level components and presets.

## Guides

- [Point gradient](docs/point_gradient.md): independently positioned colors with adjustable influence.
- [Border trail](docs/border_trail.md): a traveling edge light with a fading tail.
- [Layout transitions](docs/layout_transition.md): interruptible position and size changes with live child layout.
- [Spotlight](docs/spotlight.md): pointer-driven surface light and rounded-edge illumination.
- [Light sweep](docs/light_sweep.md): a soft animated highlight across a styled surface.
- [Frosted glass](docs/glass.md): strongly blurred panels and mergeable rounded surfaces.
- [Timed text](docs/timed_text.md): arbitrary character/word timings, gradient
  reveal, grouped lift/scale emphasis, and playback-clock integration.
- [Color flow](docs/color_flow.md): image-derived flowing light and brightness configuration.
- [Subtree effects](docs/subtree_effect.md): blur, wave, color adjustment and Bloom for element subtrees.
- [Subtree transitions](docs/subtree_transition.md): blur fades, crossfades, soft wipes and textured dissolves between two UI subtrees.
- [History feedback](docs/feedback.md): persistent trails, time-based decay and playback controls.
- [Motion blur](docs/motion_blur.md): velocity-driven directional blur for moving subtrees.
- [Depth parallax](docs/depth_parallax.md): pointer-driven image depth with paired depth maps.
- [Water ripple](docs/ripple.md): local radial refraction for text and images.
- [Local lens](docs/lens.md): smooth local magnification and compression.
- [Interaction mapping](docs/interaction_mapping.md): affine transform groups, pointer hit testing and dragging in deformed content.
- [Displacement maps](docs/displacement_map.md): external RG maps, local masks and texture-driven distortion.
- [GPU particles](docs/particles.md): light points, streaks, interactive forces and alpha-mask emission.
- [Particle transition](docs/particle_transition.md): reversible scattering and gathering of text and images.
- [GPU fluid](docs/fluid.md): interactive colored ink, momentum and vortices.
- [SDF shapes](docs/sdf.md): Boolean geometry, smooth blending, outlines and edge light.
- [Holographic material](docs/holographic.md): surface normals, directional lighting and foil reflections.
- [Contour light](docs/contour_glow.md): alpha-contour distance fields and edge-focused glow.
- [Contour relief](docs/contour_relief.md): raised and recessed bevels with directional lighting.
- [Contour shadow](docs/contour_shadow.md): directional soft shadows following text and image silhouettes.

## Progressive blur

`progressive_blur` smoothly reduces blur strength away from an edge of an element
subtree. Content outside the transition range remains sharp.

```rust
use gpui::px;
use gpui_effects::progressive_blur;

let content = progressive_blur(content).top(px(64.));
```

Use `.bottom(...)`, `.left(...)` or `.right(...)` to select another edge.
Each direction setter replaces the previous selection. `.radius(px(12.))`
sets the maximum filter support radius (0–24 logical pixels).
`.enabled(false)` bypasses offscreen rendering. Zero or invalid extents and
radii also disable the effect.

Style and size the wrapped content normally. Layout and pointer targets remain
unchanged; filtering stays within the capture bounds and clamps sampling at its
edges. The filter uses two spatially varying Gaussian passes and does not schedule
animation. Animate the extent or radius with the application's animation clock
when needed. Use `.into_effect().then(...)` to append other effects.

## Local deformation

`subtree_deformation` applies a smooth local displacement to text, images and
other painted descendants. `DeformationOptions` controls the normalized capture
anchor, influence radius and translation. Translation is limited to 35% of the
radius to prevent folded content.

`ElasticOffset` supplies an independent, caller-clocked spring: hold a translation
with `drag_to`, call `release`, and advance the return with `advance`. Frequency
and damping are configurable through `spring`.

Use `EffectStage::deformation` to compose deformation with other subtree effects.
Layout remains unchanged. Enable `.map_interaction(true)` to align child pointer
targets with the deformation. Leave transparent space around the content for displaced edges.

## License

MIT. See [LICENSE](LICENSE).
