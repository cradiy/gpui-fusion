# GPUI Android

GPUI interfaces hosted in Android Views, rendered with WGPU. The Rust
backend is paired with an Android library providing `GpuiSession`, `GpuiView`
and `GpuiActivity`.

The backend builds for Android's `arm64-v8a` and `x86_64` ABIs using Vulkan
or OpenGL ES, and supports bundled fonts, raw touch events,
single-finger taps and inertial scrolling, text input and IME composition,
text clipboard access, external links,
lifecycle notifications, and Surface replacement while
retaining the Rust application and GPU atlas. Each session hosts one GPUI window.

Native text selection handles, image/file clipboard data, file pickers,
TalkBack semantics, and GPU device-loss recovery are not implemented. System
font discovery is not configured; applications can register their own fonts.

See [Android hosting](docs/hosting.md) for application setup, embedding,
lifecycle ownership, and device verification.
