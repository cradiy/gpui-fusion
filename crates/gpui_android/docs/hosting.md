# Android hosting

## Build

Requirements:

- JDK 17 or newer, as supported by the bundled Gradle/Android Gradle plugin.
- Android SDK platform 36.1 and build tools.
- Android NDK r29 or newer.
- The Rust targets for the selected Android ABIs (listed below).
- Android 8.0/API 26 or newer with a compatible Vulkan or OpenGL ES driver.

Set `ANDROID_HOME` to the SDK and `ANDROID_NDK_HOME` to the NDK. From
`crates/gpui_android/android`:

```sh
rustup target add aarch64-linux-android
./gradlew :example:assembleDebug
adb install -r example/build/outputs/apk/debug/example-debug.apk
```

The application is a Cargo binary package with `src/main.rs`. The Android build
generates a library manifest under `target/android` from the application's
dependencies and workspace settings, then produces the native library required
by the APK. The application's manifest does not need a `[lib]` target.
The shell build helper supports Linux and macOS hosts. The host library's AAR is
built with `./gradlew :host:assembleDebug`; the APK includes it alongside the
packaged Rust application.

The packaging helper currently accepts a binary package with `src/main.rs` and
no companion library or explicit `[[bin]]` targets.

ARM64 is the default. `gpuiAbi` accepts Android ABI names, architecture names,
or Rust target triples:

| Architecture | Android ABI | Rust target |
| --- | --- | --- |
| `aarch64` | `arm64-v8a` | `aarch64-linux-android` |
| `x86_64` | `x86_64` | `x86_64-linux-android` |

```sh
./gradlew :example:assembleDebug -PgpuiAbi=aarch64
```

For an x86_64 emulator, install Rust's `x86_64-linux-android` target and use
`-PgpuiAbi=x86_64`. To include both architectures in one APK:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
./gradlew :example:assembleDebug -PgpuiAbis=aarch64,x86_64
```

Use either `gpuiAbi` or `gpuiAbis`. A multi-ABI APK is larger; Android loads the
library matching the device. The APK is written to
`example/build/outputs/apk/debug/example-debug.apk` for either configuration.
Native libraries use 16 KB load-segment and RELRO alignment. The APK's native
library packaging uses the Android Gradle plugin's 16 KB alignment support.

The renderer tries Vulkan first, then OpenGL ES if Vulkan initialization fails
or no eligible Vulkan adapter is available. Drivers must meet WGPU's device
requirements; non-conformant Vulkan adapters are not enabled. Use an ABI that
matches the emulator's system image.

Debug builds write GPU initialization diagnostics to `adb logcat -s GPUI`.

## Rust application

Use `gpui` and `gpui_platform` dependencies and an ordinary `main` function:

```rust,ignore
use gpui::{prelude::*, *};

#[gpui_platform::main]
fn main() {
    gpui_platform::application().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| MyView))
            .expect("failed to open the GPUI window");
    });
}
```

The entry attribute generates Android's library loader; on desktop it leaves
the normal executable entry unchanged. The same example runs on desktop with
`cargo run -p hello_android`. Android retains the application automatically after
`Application::run` returns and releases it when the hosting session closes.
`main` runs once per new session; Surface recreation does not invoke it again.
Use a normal GPUI window; additional windows and native popups are unsupported.
In-window overlays remain available.

The application is created on the first nonzero Surface size, not in Activity
`onCreate`. It uses Android's main Looper. Foreground work is posted through a
Handler, background work uses Rust workers, and visible, resumed, focused Views
receive Choreographer ticks. Unchanged UI does not require a new GPU draw.
If a frame cannot be presented, the next active tick retries it.

## Host ownership

Load the application library before creating a `GpuiSession`. `GpuiActivity`
does this through its `nativeLibraryName()` override and hosts a full-page View.
It forwards lifecycle events and retains the session during configuration
changes. The page is laid out inside system-bar and display-cutout insets.

For an embedded host, construct `GpuiView(context, session)`, forward the host's
start/resume/pause/stop events through `session.setLifecycle(...)`, and call
`session.close()` when the Rust application is permanently finished. A session
can bind to one attached View at a time. Detach the previous View before
attaching its replacement. The session does not own an Activity; clear any
Activity-capturing close callback when that Activity is destroyed.

`session.setOnError(...)` receives terminal initialization or rendering errors
after the session has been closed. `GpuiActivity` displays an error page when no
usable GPU backend is available. An embedded host can supply its own error UI.

All public session and View operations run on the Android main Looper.
Surface callbacks release the old WGPU surface before releasing the native
window reference. Reattachment preserves the device, atlas, and logical GPUI
window. Hiding the View or backgrounding the host stops frame callbacks.

The host must handle its own embedding insets. `PlatformWindow::insets` does not
yet publish Android keyboard or safe-area geometry. SurfaceView hosting does
not provide arbitrary Android View clipping or rotation semantics.

Raw touches retain pointer IDs and include cancellation. A short single-finger
tap also produces a mouse down/up pair for existing GPUI click handlers. A
single-finger drag beyond Android's touch slop produces pixel scroll events,
with velocity-based inertial scrolling after release. Scroll events remain
anchored at the gesture's starting position. Positions and deltas are converted
from physical pixels to GPUI logical pixels.

Preventing the default action in a raw touch handler suppresses synthesized
clicking and scrolling for the rest of that contact sequence. Multiple fingers
also suppress both. Touch cancellation, focus loss, Surface replacement, and
backgrounding stop the gesture and its inertia. Touching during inertia stops
it without activating a button. Long holds do not synthesize clicks. There is
no mouse drag emulation or Android nested-scrolling integration.

Configuration changes and Surface recreation preserve in-process state.
Process death starts a new application; persistent document restoration is the
application's responsibility. A lost GPU device requires recreating the session.

## Device verification

Open GPUI Android and tap the counter. Swipe through the list, release a fast
swipe to check inertia, and touch again to stop it. Scrolling across a clickable
row must not count as a tap. Verify that count and scroll position survive
rotation, locking/unlocking the device, and switching to another app and back.
Adding a second finger must not count as a tap or continue synthesized scrolling.
Repeatedly open and finish the Activity to check teardown.

These checks require an Android device or a compatible emulator.
Cross-compilation and APK assembly do not establish driver or lifecycle
correctness on a device.
