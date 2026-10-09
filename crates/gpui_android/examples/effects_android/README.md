# Effects lab

A touch-enabled preview of GPU particles, fluid, Bloom, border light and liquid
glass and page turning. The same Rust entry point runs on Android and desktop.

From this directory, run `gpuiforge run android` and select a device. Build an APK
without installing it with `gpuiforge build android --abi arm64-v8a`. No optional
Android host modules or permissions are required. For desktop, run
`cargo run -p effects_android` from the workspace root.

Select Particles or Fluid and drag across the surface to inject particles or dye.
Bloom toggles the post-processing pass. Materials shows an animated border light
and liquid glass over moving shapes and a grid. Pause stops the animation; Clear
resets the selected simulation. Flip shows a single-page book: drag the right
edge inward to advance or the left edge inward to return. Vertical movement does
not claim the turn; cancelling a drag returns the page to rest.

The header reports the window's particle, fluid and backdrop capabilities. An
unsupported simulation shows a message instead of starting its GPU pipelines.
Availability depends on the selected backend and device limits; a successful
Vulkan initialization alone does not imply every optional effect is available.

To check lifecycle behavior, paint into a surface, rotate the device, then move
the app into the background and resume it. Surface size changes and GPU device
recovery may reset simulation contents. Draw again to verify input and rendering.
Use a physical device to assess animation smoothness and sustained GPU cost.
