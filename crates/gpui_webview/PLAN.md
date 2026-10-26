# WebView architecture and implementation plan

## Scope

`gpui_webview` embeds browser content as a rectangular native surface in a GPUI window. Applications use `WebView`, `WebViewOptions`, and `WebViewEvent` without importing engine types. Backend dependencies are opt-in Cargo features. Backend selection happens at view construction; changing engines requires creating another view.

Servo is the first backend. WPE WebKit is the planned second Linux backend. WPE is not implemented and has no feature flag yet. A backend must report unavailable configurations explicitly; there is no automatic engine substitution.

## Ownership and boundaries

| Layer | Responsibility |
| --- | --- |
| Public component | Layout, focus, navigation commands, lifecycle state, application events |
| Internal engine contract | Placement, painting, event processing, input, navigation |
| Servo backend | Servo runtime, WebView delegates, engine event conversion, OpenGL presentation |
| Wayland surface adapter | Child surface ownership, viewport sizing, placement, native handles |
| Application | Navigation controls, error presentation, policy and product UI |

The Servo runtime is shared through GPUI application state. Individual views own their browser instance and rendering context. Rendering contexts must be destroyed before their native child surfaces. Engine wakeups are coalesced into the GPUI event loop; animation polling runs only while the engine reports active animation. Initialization waits for the parent window to present a frame.

Pointer events use GPUI window-relative logical coordinates. The backend subtracts the native surface's rounded origin and maps the viewport destination to the physical render target. Wayland child positioning and viewport destination use logical coordinates; the render target uses device pixels. Engine cursor changes are translated to GPUI cursor styles. Mouse presses beginning in the view retain movement and release delivery across its boundary within the host window; losing component focus releases tracked buttons.

## Current implementation

- Linux Wayland native child surface with fractional-scale-aware render sizing.
- Local-file and URL navigation, back, forward, and reload.
- Readiness, loading, title, URL, and failure events.
- Mouse movement and buttons, wheel input, basic keyboard events and focus forwarding.
- IME preedit, commit, cancellation, and field-bound candidate placement through GPUI's input handler.
- Engine keyboard shortcuts and text clipboard access through GPUI.
- A browser example using the public component API.

This is an experimental native surface implementation. GPUI overlays, arbitrary clipping, transforms, partial occlusion, hidden views, and scrolling parents are not supported integration contracts yet. The backend does not implement download handling, file pickers, JavaScript dialogs, permission prompts, direct touch/pinch gestures, or accessibility tree grafting. Text caret geometry across writing modes and font metrics requires broader validation. Cross-platform backends are not implemented.

The input handler exposes composition-local offsets. Surrounding-text queries and deletion, arbitrary document-range replacement, and IME-selected subranges within preedit are not supported. Candidate placement uses the focused field rectangle rather than an exact glyph caret. Undo and redo require editing-history support in Servo.

The Servo backend and its Stylo dependencies use pinned Git forks. Engine fixes belong in those repositories. Servo forwards mouse releases without a hit target to clear pressed-button state without synthesizing a click on an in-page target. Frame presentation waits for the current document's first layout to reach WebRender, retaining the previous surface contents during navigation.

## Delivery stages

### 1. Native surface and input correctness

Validate pointer hit testing and text caret placement independently. Verify wheel direction, horizontal scrolling, DPI transitions, resize, focus loss, and mouse release outside the rectangle. Extend text input with synchronized surrounding text, preedit selection, and exact caret bounds. Keep native input events separate from text commits to avoid duplicate insertion.

Acceptance requires actual Wayland interaction on fractional and integer scale factors. A successful build is not graphical acceptance.

### 2. Lifecycle and embedding

Exercise multiple simultaneous views, repeated create/drop cycles, window closure, hidden views, and shared-runtime teardown. Define visible-rectangle clipping and synchronization with GPUI overlays. Validate CPU quiescence when idle and release of per-view surfaces and renderer resources. Browser runtime caches are measured separately from per-view retention.

### 3. Application integration

Add navigation policy, new-window requests, clipboard, dialogs, file selection, downloads, permission requests, and an explicit JavaScript messaging API. Define asynchronous response ownership and cancellation when a view is dropped. Make backend capabilities queryable where behavior differs. Document the process-isolation and trust contract before treating the component as suitable for arbitrary remote content.

### 4. WPE backend

Implement WPE behind its own feature using the same public component and lifecycle events. Decide the native-surface adapter and event-loop bridge from a working Wayland prototype. Verify input, IME, navigation, resize, and teardown with both engines enabled in one application. Unsupported capabilities must remain explicit rather than being emulated silently.

### 5. Distribution

Keep the dependency chain pinned and self-contained rather than requiring consumers to supply root Cargo patches. Verify builds with no backend, each backend individually, and both together. Record runtime dependencies, licensing, and packaging requirements. Add supported platforms only after native interaction and lifecycle validation.
