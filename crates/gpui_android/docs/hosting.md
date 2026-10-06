# Android hosting

## Build

Requirements:

- JDK 17 or newer, as supported by the bundled Gradle/Android Gradle plugin.
- Android SDK platform 36.1 and build tools.
- Android NDK r29 or newer.
- The Rust targets for the selected Android ABIs (listed below).
- Android 8.0/API 26 or newer with a compatible Vulkan or OpenGL ES driver.

Install GPUiForge from the repository root with
`cargo install --path tools/gpuiforge`. Set `ANDROID_HOME` to the SDK
and `ANDROID_NDK_HOME` to the NDK. From
`crates/gpui_android/examples/hello_android`:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
gpuiforge run
```

External applications enable the bundled Android support in `gpuiforge.toml`:

```toml
[platforms.android]
application-id = "dev.example.app"
```

GPUiForge derives the native library name from the application's Cargo package.
No recipe or local GPUI checkout path is required.

The platform menu offers desktop and Android. Android run prompts for a device
and builds its ABI. `gpuiforge build android` packages both configured ABIs;
`gpuiforge run android --device emulator-5554` selects a device explicitly.

`gpuiforge.toml` belongs to the Rust application. GPUiForge generates the
Kotlin host and Gradle application under
`target/gpuiforge/android`. The debug APK is written to
`target/gpuiforge/android/app/build/outputs/apk/debug/app-debug.apk`.

Use `gpuiforge platform eject android` to export the generated application to
`platforms/android` and switch the TOML configuration to manual management.
Subsequent builds preserve user-owned Kotlin, Manifest, and Gradle files.
See [GPUiForge configuration](../../../tools/gpuiforge/docs/usage.md) for recipes,
template variables, and ownership rules.

The application is a Cargo binary package with `src/main.rs`. The Android build
generates a library manifest under `target/android` from the application's
dependencies and workspace settings, then produces the native library required
by the APK. The application's manifest does not need a `[lib]` target.
The shell build helper supports Linux and macOS hosts. The generated project's
`:host` module builds the Kotlin host library alongside the Rust application.

The packaging helper currently accepts a binary package with `src/main.rs` and
no companion library or explicit `[[bin]]` targets.

The configured `abis` list uses Android ABI names:

| Architecture | Android ABI | Rust target |
| --- | --- | --- |
| `aarch64` | `arm64-v8a` | `aarch64-linux-android` |
| `x86_64` | `x86_64` | `x86_64-linux-android` |

A multi-ABI APK is larger; Android loads the library matching the device.
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

The host library is written in Kotlin and can also be called from Java. A
full-page Kotlin host only selects its Rust library:

```kotlin
class MainActivity : GpuiActivity() {
    override fun nativeLibraryName() = "my_app"
}
```

Load the application library before creating a `GpuiSession`. `GpuiActivity`
does this through its `nativeLibraryName()` override and hosts a full-page View.
It forwards lifecycle events and retains the session during configuration
changes. The page is laid out inside system-bar, display-cutout, and keyboard insets.

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

## Text input

Focused GPUI input handlers are exposed through Android's `InputConnection`.
Text stays in the Rust component. The connection supports text commitment,
composition updates and completion, composing regions, directed UTF-16
selections, and surrounding deletion in UTF-16 units or Unicode code points.
Batch edits defer selection notifications until the batch ends. Connections
are invalidated when their View detaches or their input focus changes.

Input components must implement `EntityInputHandler::set_selected_text_range`
to accept cursor and selection changes from Android. UIC's `TextInput` implements
this contract. Tapping a focused input requests the soft keyboard. Hardware
text keys and common editing shortcuts are forwarded to GPUI; system Back is
left to Android.

Surrounding text queries are bounded around the selection and composition.
Handlers that withhold `surrounding_text` expose no text snapshot to the IME.
The host uses a generic multiline text keyboard, or a password keyboard when
no snapshot is available, and disables personalized learning. Per-field keyboard
types, native selection handles, cursor-anchor updates, rich IME content, and
hardware dead-key composition are not implemented.

`GpuiActivity` resizes its content for the keyboard. Embedded hosts must apply
their own keyboard insets. Register fonts covering the languages your UI uses;
the bundled Latin font is not a complete CJK or emoji font collection.

## Clipboard and links

The standard GPUI clipboard APIs read and write plain text through the Android
system clipboard. The asynchronous variants report host errors. Synchronous
variants log errors and return no item on a failed read. Access requires an
attached View and follows Android's clipboard access restrictions; a read can
return no item when the application lacks focus.

Text entries are concatenated when writing. Reading multiple Android text items
joins them with newlines. Empty strings are supported. Text metadata and spans
are not preserved. Image and file writes, mixed text/non-text writes, and items
without entries are rejected without replacing the clipboard. URI and Intent
items are not resolved to text.

`App::open_url` sends an Android `ACTION_VIEW` intent using the attached View's
context. The URL must include a scheme, and an installed application must handle
it. Failures are logged without closing the GPUI session. Inbound deep links
and URL scheme registration are not implemented by the backend.

## Device verification

Open GPUI Android and tap the counter. Swipe through the list, release a fast
swipe to check inertia, and touch again to stop it. Scrolling across a clickable
row must not count as a tap. Verify that count and scroll position survive
rotation, locking/unlocking the device, and switching to another app and back.
Adding a second finger must not count as a tap or continue synthesized scrolling.
Repeatedly open and finish the Activity to check teardown.

Tap the text field, type and delete text, move the cursor, and replace a
selection. With a composing IME, check preedit updates, candidate commitment,
and deletion around emoji. Dismiss the keyboard with Back, then tap the field
to reopen it. Check that the visible content resizes when the keyboard opens.

Use Copy count and Paste text to check clipboard round trips, then copy text
between GPUI and another application. Include multiline text and non-ASCII
characters. Open website should launch a browser or Android's app chooser;
returning to GPUI must preserve the counter and scroll position.

These checks require an Android device or a compatible emulator.
Cross-compilation and APK assembly do not establish driver or lifecycle
correctness on a device.
