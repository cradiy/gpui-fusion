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
Handler and background work uses Rust workers. Visible, resumed, focused Views
schedule Choreographer callbacks for invalidations, requested animation frames,
inertial scrolling and selection-handle dragging. Idle Views stop scheduling frames;
input, asynchronous updates and resuming the host wake them as needed.
If a frame cannot be presented, GPUI schedules another frame to retry it.
Drawing and Surface configuration run on the main Looper; swapchain recreation
can wait for in-flight GPU work.

`Window::appearance()` follows the hosting View's Android night-mode
configuration. Theme changes notify GPUI and redraw the window, including when
an Activity recreates its View around a retained session. Embedded hosts that
handle configuration changes receive updates through the View as well.
The generated application supplies light and dark Android themes. Applications
choose their GPUI colors from the reported appearance; custom colors are not
automatically recolored.

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
window. Resizing an existing native window retains its WGPU surface.
`GpuiView` completes `SurfaceHolder.Callback2` redraws before returning control
to Android. Hiding the View or backgrounding the host stops frame callbacks.
On Android 11 and later, the full-page host keeps the Surface at its safe-area
size and animates GPUI's layout viewport with the keyboard insets. Keyboard
visibility does not resize the swapchain. Embedded hosts can use
`GpuiView.setWindowInsets(insets, viewportBottomInset)` to publish geometry and
exclude a physical-pixel bottom region from layout. Android 8 through 10 use
the system's resize behavior.

`Window::insets()` reports system occlusion in logical pixels. `safe_area` and
`ime` are measured from the host window edges; `consumed` records the space
already excluded by host placement, padding, or viewport resizing. Use
`insets.effective()` for additional padding inside GPUI. The full-page host
already avoids system bars and the bottom keyboard, so these edges need no
additional padding. Floating keyboards do not necessarily produce edge insets.
`Context::observe_window_insets` observes changes, and inset changes refresh
the window even when its viewport size is unchanged.

Embedded hosts supply `GpuiWindowInsets` with physical-pixel `EdgeInsets` for
`safeArea`, `ime`, and `consumed`, all measured from the same host window edges.
Include the View's placement and any `viewportBottomInset` in `consumed`.
Passing a zero bottom inset leaves the full Surface available for GPUI layout;
the application can then use `effective()` to avoid remaining occlusion.
Android 11 and later publish separate safe-area and IME geometry throughout
keyboard animations. The Android 8–10 full-page host publishes the legacy
combined system insets as consumed safe area; separate IME geometry is unavailable.

Text rendering loads the device's available system font files, including CJK
and emoji fonts, alongside the embedded default font. Android 10 and later use
`SystemFonts`; Android 8 and 9 use `/system/fonts`. Applications can also register
their own fonts through GPUI's text system.

The host handles its own embedding insets. SurfaceView hosting does not provide
arbitrary Android View clipping or rotation semantics.

Raw touches retain pointer IDs and include cancellation. A short single-finger
tap also produces a mouse down/up pair for existing GPUI click handlers. A
single-finger drag beyond Android's touch slop produces pixel scroll events,
with velocity-based inertial scrolling after release. Scroll events remain
anchored at the gesture's starting position. Positions and deltas are converted
from physical pixels to GPUI logical pixels.

`Window::on_touch_event`, registered during paint, receives raw contact phases
with pointer-mapped coordinates. Handlers track ownership by touch ID and
hit-test when accepting a contact; GPUI does not automatically capture contacts
to elements.

Preventing the default action in a raw touch handler suppresses synthesized
clicking and scrolling for the rest of that contact sequence. Multiple fingers
also suppress both. Multi-contact scaling uses Android's `ScaleGestureDetector`
and produces `PinchEvent` through `on_pinch`, with a logical-pixel focus position
and incremental `delta` (`scale *= 1.0 + event.delta`). Begin and end events have
zero delta. Quick-scale and stylus-button scaling are disabled. Raw touch
handlers that prevent the default action cancel scaling for the rest of the
contact sequence. Lifting back to one finger does not resume scrolling or
produce a click; a fresh touch starts a new interaction. Pinch routing follows
the current focus position and the standard GPUI hit-test rules.

Touch cancellation, focus loss, Surface replacement, and
backgrounding stop the gesture and its inertia. Touching during inertia stops
it without activating a button. Long holds do not synthesize clicks. There is
no mouse drag emulation or Android nested-scrolling integration.

`on_long_press` receives one `LongPressEvent` after a stationary single touch
reaches Android's long-press timeout. Movement past touch slop, additional
fingers, raw-touch default prevention, cancellation, or loss of the active
Surface cancels a pending long press. It does not synthesize a click or right
mouse button. Use `capture_long_press` to observe before descendants and
`cx.stop_propagation()` to exclude ancestors. Call `window.prevent_default()`
to claim the interaction and suppress native text selection; otherwise text
inputs retain their selection menu and handles. A claimed long press suppresses
scrolling and pinch recognition until every finger is lifted.

External mice provide hover, button presses, dragging, double/triple clicks, and
horizontal/vertical wheel scrolling. GPUI cursor styles use Android system pointer
icons. Wheel distances follow Android's scroll factors and the View's density.
Focus loss, cancellation, and Surface replacement release pressed buttons without
activating click or drop handlers. Mouse input does not synthesize touch gestures
or open the finger-selection handles.

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
text keys and common editing shortcuts are forwarded to GPUI.

Holding a finger inside an input focuses it, selects a word using Android's
locale-aware word boundaries, and opens its floating edit menu. Input components
accept `TextInputFocusEvent` through an occlusion-aware hitbox, without dispatching
a click to surrounding controls. UIC's `TextInput` supports this request.
Select all, copy, cut, and plain-text paste dispatch the component's existing
editing shortcuts. Copy and cut require a selection and are hidden for password
fields; paste appears when the clipboard advertises plain text. Select all
keeps the menu open for a subsequent action. Sliding beyond touch slop, adding
another finger, leaving the window, or changing the input focus dismisses it.

Tapping an input shows a draggable insertion handle. Selected text exposes two
endpoint handles using the Android theme's drawables. Handle positions and touch
indices come from GPUI's input handler; text and selection remain GPUI-rendered.
Dragging a handle shows Android's magnifier on API 28 and later and temporarily
hides the toolbar. Password fields expose cursor movement without magnifying text.
Handles are dismissed on focus loss, surface detachment, or a new gesture in the
content. Holding a handle at an editor edge scrolls its text while extending the
selection. Single-line fields scroll horizontally; multiline fields scroll
vertically. Scrolling stops on release or cancellation and does not propagate
to parent containers. Input components provide their visible viewport through
`EntityInputHandler::element_bounds` and opt into scrolling through
`EntityInputHandler::scroll_text_input`. UIC's `TextInput` implements both.
Semantic actions such as text classification are not provided.

Input methods can request immediate or monitored `CursorAnchorInfo` updates.
GPUI supplies the selection, available composing text, insertion-marker bounds,
and (on Android 13 and later) editor bounds in screen coordinates. Geometry
tracks rendered layout and View placement; unchanged reports are suppressed.
Password fields do not expose composing text. The caret baseline remains
unspecified because GPUI's input-handler contract supplies a rectangle only.
Character-bound, visible-line, and text-appearance filters are unsupported and
return false. Closing the input connection stops its geometry subscription.

Call `Window::show_soft_keyboard()` after focusing a text input to request the
keyboard without tapping the field. `Window::hide_soft_keyboard()` dismisses
it without clearing input focus or text. Requests are applied after the next
GPUI frame so the input handler reflects the current focus. The latest pending
request wins; requests are discarded when the View loses focus, detaches, or
the session becomes inactive. In-app Back cancels pending keyboard requests and
dismisses the keyboard. Showing requires an active, visible Surface and
an input handler accepting text. Android and the selected IME decide whether
to show an on-screen keyboard when a hardware keyboard is connected.

Surrounding text queries are bounded around the selection and composition.
Handlers that withhold `surrounding_text` expose no text snapshot to the IME.
`EntityInputHandler::text_input_mode` describes the field as `SingleLine`,
`Multiline`, or `Password`; custom editors default to `Multiline`. UIC maps its
existing input modes automatically. By default, single-line and password fields
request Done, which dispatches Enter (UIC emits `InputEvent::Submit`); multiline
fields request a newline key. UIC's multiline Enter inserts a newline.

`EntityInputHandler::text_input_action` can override this with `Done`, `Go`,
`Search`, `Send`, `Next`, or `Previous`. Handle the action in
`perform_text_input_action`, returning true when accepted. Composition completes
before dispatch. Actions inconsistent with the current connection are rejected;
unhandled Done falls back to Enter, while other actions require a handler.
Done hides the keyboard if the original field still has focus. Other actions
leave keyboard visibility and navigation to the application.

UIC configures actions on its input state:

```rust
TextInput::new(cx).input_action(gpui::TextInputAction::Next)
```

Subscribe to `InputActionEvent` to receive the configured action and committed
text. It is separate from `InputEvent::Submit`; physical Enter bindings are
unchanged. A Next/Previous handler selects and focuses the destination field
and can call `window.show_soft_keyboard()`. Use
`set_input_action(Some(action), cx)` to update the action or `None` to restore
the default. Changing the effective action restarts the connection. Explicit
multiline actions replace the soft keyboard's newline key; physical Enter
still inserts a newline.

`EntityInputHandler::text_input_purpose` requests a single-line keyboard layout:
`Text`, `Email`, `Url`, `Phone`, or `Number { decimal, signed }`. UIC configures
it on the input state:

```rust
TextInput::new(cx).input_purpose(gpui::TextInputPurpose::Email)
```

Use `TextInput::set_input_purpose(purpose, cx)` to change it while editing.
The value and selection are retained; the input connection restarts and finishes
composition. These hints do not filter typing or pasted content. Validate values
in the application. Password and multiline modes take precedence over purpose.
The keyboard's available keys depend on the selected IME.

Password fields never export surrounding text, even when their handler supplies
it. Personalized learning is disabled for all modes. Changing mode restarts the
input connection, completes composition, and invalidates callbacks from the old
connection. Missing surrounding text alone does not change the keyboard type.
Custom action labels, rich IME content, and hardware dead-key composition are
not implemented.

## Permissions

Declare Android permissions in the application's `gpuiforge.toml`:

```toml
[platforms.android]
application-id = "dev.example.app"
permissions = ["android.permission.RECORD_AUDIO"]
```

Capture `gpui_android::current_platform().permissions()` during application
startup and retain the resulting `AndroidPermissions` handle in the application.
It belongs to one session and is used on the GPUI foreground thread:

```rust
let status = permissions.status("android.permission.RECORD_AUDIO")?;
let result = permissions.request("android.permission.RECORD_AUDIO").await?;
```

Request after the user invokes the relevant feature. `PermissionStatus` reports
`Granted` or `Denied { should_show_rationale }`. A false rationale hint does not
distinguish a first request from a denial without another prompt. Recheck before
accessing a resource because Android or the user can revoke grants.

The interface supports manifest-declared normal and dangerous permissions.
Unknown permissions, undeclared permissions, specialized authorization flows
(such as overlay access), unavailable Activities, cancellation, and concurrent
requests return errors. Already granted permissions return without a dialog.
Only one request can be pending per session. Dropping the future discards its
result; an existing system dialog remains until Android completes it.

`GpuiActivity` connects the permission host automatically. Embedded hosts call
`session.attachPermissionHost(activity)`, forward `onRequestPermissionsResult`
to the session, and call `detachPermissionHost(activity)` on destruction.
Request codes `0x4700..0x7fff` are reserved for GPUI. Detaching the Activity,
including configuration recreation, cancels pending requests; stale results
are ignored. Closing the session also completes pending requests with an error.
The requesting task should be cancelled with its owning UI when appropriate.

The example's Request microphone permission button only checks authorization;
it does not record audio.

## Files

Use `App::prompt_for_files(FilePromptOptions { multiple, ..Default::default() })` to open Android's
system document picker. It returns `Some(files)` after selection and `None`
after cancellation. Each `SelectedFile` exposes a display name and an asynchronous
`read()` method. Metadata queries, descriptor opening, and reads run on background
workers, including documents backed by a pipe or a remote provider.

Android documents do not expose a GPUI filesystem path or browser URL. Use
`SelectedFile::read()` instead; it loads the complete contents into memory. Each
read opens a fresh descriptor. Reading can fail if the provider is unavailable or
access has been revoked. Persistent access is explicit through file bookmarks.
The picker requires no broad storage permission. Directory selection
and `prompt_for_paths` are not supported.

To edit an existing document, set `FilePromptOptions::writable` to `true`.
To choose a new destination, call `App::prompt_for_file_save`:

```rust
let selection = cx.prompt_for_file_save(FileSaveOptions {
    suggested_name: "note.txt".into(),
    mime_type: "text/plain".into(),
    ..Default::default()
});
// In a foreground task:
if let Some(file) = selection.await?? {
    file.write(contents).await?;
}
```

The save picker creates a document before returning its handle. A duplicate
filename creates a separate document with a system-selected suffix. Retain the
handle and call `write()` again to save subsequent edits to that document.
The `directory` option is a desktop hint and is ignored on Android.

Read-only selections reject writes. Writable selection fails if the provider
does not grant write access; `can_write()` reports permitted access, not whether
a later provider operation will succeed. `SelectedFile` is the GPUI name for
`gpui_io::FileHandle`. It supports metadata, independent reader sessions, writer
sessions, and incremental `write_stream()`. One writer can be open per document
handle; await completion before opening another.

Writes replace and truncate contents. An error can leave an ordinary picker
document partially written; provider completion does not imply cloud synchronization.
Use `App::file_system(app_id)` for app-private storage and supported public
collections. See [file I/O](../../gpui_io/docs/file_io.md) for location mappings,
streaming, cancellation, and publication of new collection items.

Only one file selection can be pending per session. Dropping its receiver discards
the result without dismissing the system picker. Closing the session completes
pending requests with an error.

`GpuiActivity` connects the picker automatically. Embedded hosts call
`session.attachFileHost(activity)`, forward `onActivityResult` to the session,
and call `detachFileHost(activity)` on destruction. Request codes
`0x8000..0xbfff` are reserved for GPUI. A retained session preserves its pending
selection across Activity configuration recreation; final host detachment
completes it with an error. Results from earlier requests are ignored.

### Open with another application

`App::open_file_with_system(&file)` dispatches `ACTION_VIEW` for a selected or
restored document, a published MediaStore file, or a private path exposed by the
host's FileProvider. Await the returned task to
observe dispatch errors, including a missing viewer, rejected access, or a
detached host view. Success does not report whether the receiving application
finished reading the file.

The intent includes the provider's MIME type and a temporary read-only URI grant.
It does not request editing, persist access, copy contents, or add storage
permissions. Finish writing before opening a file. Unpublished collection items
and documents with an active writer are rejected. Path-based handles do not track
other writers; the application must await its writes before opening them.

The bundled host exposes `AppData` (`filesDir/Data`) and `Cache` through a
non-exported, read-only `GpuiFileProvider` with authority
`${applicationId}.gpui.files`. Only the requested file receives a temporary grant;
other files in the directory remain inaccessible. Files are served in place,
without copying. Keep them available while the viewer is using them. Missing
files, directories, and paths resolving outside configured roots are rejected.
`AppConfig`, `noBackupFilesDir`, databases, and shared preferences are not exposed
by the default configuration. MIME types for path handles are inferred from the
file extension, with `application/octet-stream` as the fallback.

Hosts maintained outside GPUiForge must include the provider manifest entry,
AndroidX Core dependency, and `gpui_file_paths.xml`. An application can override
that XML resource with narrower directories. URI grants do not provide persistent
bookmarks or report when a viewer has finished reading.

### System sharing

`App::share(ShareOptions)` opens the Android Sharesheet for plain text, URLs,
files, or files accompanied by text. Other platform backends currently return
`Unsupported`. Empty requests return `InvalidInput`.

```rust
let request = cx.share(ShareOptions {
    text: Some("A note to accompany the document".into()),
    files: vec![file.clone()],
    ..Default::default()
});
// In a foreground task:
request.await?;
```

File preparation follows the same provider, directory, and write-completion
requirements as `open_file_with_system`. Files are not copied or loaded into
memory. Single-file requests use `ACTION_SEND`; multiple files use
`ACTION_SEND_MULTIPLE`. The MIME type is the files' common type, their shared
top-level type such as `image/*`, or `*/*` for unrelated types. Text-only requests
use `text/plain`. Receivers decide whether to use accompanying text.

All file URIs are included in the intent's clip data with temporary read-only
grants. Keep the source files available for the receiving application. Any file
preparation failure prevents the entire share request. Success only reports that
the Sharesheet was requested; choosing a target, cancelling the sheet, and
delivery completion are not reported. The optional title is a system UI hint.

### Receiving shares

```rust,ignore
let app = gpui_platform::application();
app.on_receive_share(|result, cx| {
    match result {
        Ok(share) => { /* Present share.text and share.files for user review. */ }
        Err(error) => { /* Show the receive error. */ }
    }
});
app.run(|cx| { /* Open the application window. */ });
```

Enable the share target with GPUiForge's `platforms.android.share-mime-types`,
for example `["text/plain", "image/*"]`. The host accepts `ACTION_SEND` and
`ACTION_SEND_MULTIPLE`. `ReceivedShare` contains optional plain text, the
sender-declared MIME type, and read-only `SelectedFile` handles. URI entries
mirrored between `EXTRA_STREAM` and `ClipData` are delivered once in their
original order. Only `content://` file URIs are accepted.

Register the callback before `Application::run`. Cold-start requests and new
intents are queued in arrival order and delivered on the main thread. Provider
metadata is read on I/O workers. A malformed request or metadata failure returns
an error for that request; subsequent requests continue. File contents are not
read or copied automatically. MIME types, names, text, and contents are untrusted
input and must be validated by the application.

File access uses temporary grants from the sender. Holding a handle does not
extend permission beyond the receiving Activity task's lifetime. These handles
do not acquire persistent grants; applications needing durable content can copy
it explicitly while access remains available. Configuration changes retaining
the session do not replay a share. Process recreation creates a new session and
can redeliver its launch intent. Closing a session discards its pending requests.

Custom hosts forward each intent once through `GpuiSession.onOpenIntent` and
declare matching manifest filters. Other GPUI backends do not currently deliver
incoming shares.

### Persistent file access

Call `file.persist().await?` while a picked document's access is valid, then save
the returned `gpui_io::FileBookmark` using Serde. After restarting, obtain
`App::file_system(app_id)` and call `restore_file(&bookmark).await?`. Restore checks
the retained permissions and opens the document for reading before returning a
handle. It does not display a picker or request broader access. Missing files,
revoked permissions and unavailable providers return errors.

`release_file(&bookmark).await?` releases the bookmark's retained read/write
permissions without deleting the document. Releasing an already absent grant
succeeds. Grants belong to the Android application, not to a handle: releasing
one can invalidate other bookmarks for the same URI. Temporary picker grants or
already-open descriptors may remain usable. Dropping a handle does not release a
persistent grant.

The adapter persists only permissions requested by the handle and offered by the
picker. Providers without persistable grants return an error. Public collection
items and private path handles do not support this document-bookmark mechanism.
Bookmarks do not preserve access after uninstall, app-data clearing, document
removal, or movement to a provider that changes its URI. The application owns
bookmark storage and decides when to retain or release access; GPUI maintains no
recent-files list.

## Credentials

`App::write_credentials`, `read_credentials` and `delete_credentials` store one
username and binary secret per exact URL string. Writing replaces that entry;
reading a missing entry returns `None`, and deleting it succeeds. The UTF-8
username and secret together may occupy at most 1 MiB minus four bytes.

Android uses an application-scoped Android Keystore AES-GCM key. Encrypted
records live in private, backup-excluded storage; credential storage requires no
runtime permission or biometric prompt. Hardware protection depends on the
device's Keystore implementation. See [Android Keystore](https://developer.android.com/privacy-and-security/keystore).

Operations run on background workers. Await a write or deletion before issuing
a dependent operation. Dropping its task does not roll back an operation already
dispatched. Authentication failures, malformed records and unavailable keys
return errors without deleting records or silently replacing the key. Explicit
deletion remains available for an unreadable entry. Credentials are not portable
between installations or devices.

## System Back

Register a window callback with `Window::on_system_back(cx, callback)` and call
`Window::set_back_enabled(true)` while an in-app destination can go back.
Disable it at the navigation root. The callback should update navigation state
and disable Back when returning to the root; capture entities weakly or use
`Window::handler_for`.

`GpuiActivity` handles committed Back through `OnBackInvokedDispatcher` on
Android 13 and later and `onBackPressed` on older releases. Its callback uses
default priority so the IME can handle Back first. If Back reaches the host
while the IME is visible, it hides the keyboard without navigating or sending
preview events. The callback is unregistered while the Activity is paused or
neither application Back nor the IME needs it. With the keyboard hidden, the
navigation root keeps Android's default Back behavior.

On Android 14 and later, `Window::on_system_back_gesture(cx, callback)` receives
`BackGestureEvent` with a `TouchPhase`, progress in `0.0..=1.0`, and the starting
display edge (`None` for non-edge sources). Use `Started` and `Moved` to draw a
navigation preview. Restore the preview on `Cancelled`; `Ended` precedes the existing `on_system_back`
callback, which commits navigation. Terminal events retain the last progress.
Buttons and older releases may commit Back without preview events. Registering
a preview listener does not enable Back interception or provide an animation.
Pausing, losing the Surface, disabling Back, or detaching the platform callback
cancels an unfinished preview.

Custom hosts use `GpuiSession.setOnBackEnabledChanged` to register or unregister
their navigation callbacks, then call `GpuiSession.handleSystemBack()` when Back
is committed. A `false` result leaves navigation to the host. Enable
`android:enableOnBackInvokedCallback` in the hosting Activity's manifest when
using the platform dispatcher. Clear the listener when detaching the host.
For predictive previews, forward `OnBackAnimationCallback` through
`startBackGesture(progress, GpuiBackEdge)`, `progressBackGesture(progress)`,
and `cancelBackGesture()` on the main thread. Commit through `handleSystemBack()`
and cancel any unfinished preview before unregistering the host callback.

## Touch feedback

Call `window.perform_haptic_feedback(HapticFeedback::Selection)` from an
interaction handler to request system feedback. The available intents are
`Selection`, `Confirm`, `Reject`, `LongPress`, `GestureStart`, and `GestureEnd`.
Android selects the waveform through AndroidX's compatible View feedback API;
availability and feel depend on the device and OS version.

Requests require an active, focused View with a live Surface and honor the
system and View feedback settings. They need no `VIBRATE` permission. `false`
means the request was unavailable or declined; `true` means it was accepted,
not that physical vibration was verified. Other current backends return false.
GPUI does not queue feedback for later activation or attach it to ordinary
buttons automatically. Accepted long presses already request their native
feedback, so handlers should not request a second pulse for the same action.

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
it. Failures are logged without closing the GPUI session.

Register `Application::on_open_urls` to receive inbound `ACTION_VIEW` URLs.
`GpuiActivity` forwards the launch intent and `onNewIntent`; a retained session
does not replay the launch URL when the Activity is recreated. URLs received
before the first Surface or before callback registration are queued and delivered
in order on the main thread before a frame. Repeated intents with the same URL
remain separate requests. Closing the session discards pending requests.

Custom hosts call `GpuiSession.onOpenIntent(intent)` once for each incoming intent.
It accepts `ACTION_VIEW` with a URI scheme and `ACTION_SEND`/`ACTION_SEND_MULTIPLE`
shares, returning false for other intents. Applications validate the content and
decide which page or document to open.

Declare custom schemes through GPUiForge's `platforms.android.url-schemes`.
Custom manifests can provide narrower filters or verified HTTPS App Links.
Android scheme registration is a build-time manifest setting;
`register_url_scheme` does not change the installed manifest.

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
Name's Done key must increment Name submissions without inserting a newline;
Message must allow newlines. Password must use password input settings and hide
its contents. Toggle Show / hide password while editing to check connection
refresh without losing the value.
In Details, type in the keyboard-layout field and use Change keyboard to cycle
through email, URL, phone, digits, signed decimal, and plain text. Check that the
layout changes and the field keeps its value; pasting text must remain possible
with a numeric layout.
Use Change action to cycle the layout field's action. Pressing its IME Next
key focuses Reply; Previous focuses Name. Search, Go, Send, and Done report
their action without inserting a newline. Reply's Send reports its character
count and clears the field. Physical Enter in Reply must still insert a newline.

Open details, focus its text field, and press Back: the keyboard closes first,
then another Back returns to the main page. At the main page, Back uses Android's
default navigation. Repeat with the edge gesture and after backgrounding the
details page; state and Back handling must survive Activity recreation as well.
Use Edit name to focus and open the keyboard without tapping the field. Hide
keyboard must dismiss it while preserving the text and input focus; Edit name
and tapping the field must both reopen it.

Use Copy count and Paste text to check clipboard round trips, then copy text
between GPUI and another application. Include multiline text and non-ASCII
characters. Open website should launch a browser or Android's app chooser;
returning to GPUI must preserve the counter and scroll position.

These checks require an Android device or a compatible emulator.
Cross-compilation and APK assembly do not establish driver or lifecycle
correctness on a device.
