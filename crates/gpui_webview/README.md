# gpui_webview

Native browser views for GPUI, with an engine-independent component API and optional rendering backends.

The experimental Servo backend targets Linux Wayland. Enable `servo` to build it and select `WebViewBackend::Servo` when constructing a `WebView`.

The Servo backend keeps navigation history without retaining inactive documents
in the back-forward cache. Back and forward navigation reload the destination
document; in-memory page state is not preserved across those navigations.

On Linux, the Servo backend enables Servo's jemalloc allocator with background
page reclamation for the host process. Applications using this backend must not
declare another Rust global allocator. Native dependencies may still use the
system allocator.

SVG images resolve CSS custom properties from supported static stylesheet rules
and inline declarations, including inherited values and `var()` fallbacks.
SVG images support looping SMIL `animateMotion` with paths, local `mpath`
references, linear or paced timing, `keyPoints` / `keyTimes`, and rotation.
Frames are generated on demand at up to 60 updates per second. Animation is
limited to 1 MiB SVG sources, 64 motion elements, and 16 MiB raster frames.
Finite or event-based SMIL timing, additive motion, and CSS animations inside
SVG images are not supported.

CSS image masks support static image layers, interpreted as alpha or luminance,
with sizing, positioning, and `repeat`, `repeat-x`, `repeat-y`, `space`, and
`round` tiling. Multiple layers support `add`, `subtract`, `intersect`, and
`exclude` compositing, including transparent `none` layers. Each layer honors
the rounded border, padding, or content box selected by `mask-clip`.
Gradient masks, animated masks, and SVG `<mask>` references are not supported.

Generated mask textures are limited to 16 MiB each, with a 64 MiB pixel-data
budget and 1024 entries per document image cache. This budget is not a limit on
total CPU and GPU memory. Repeated and composite masks have an
8192-device-pixel limit on each dimension; composites support up to 32 layers.
Multi-layer results are generated on the CPU and cached for reuse.
Requests exceeding these limits produce a transparent mask. Cached textures are
released when their source image is invalidated or the document cache is cleared.

Servo uses the fork's `gpui-servo` Git branch, with the workspace lockfile recording
the resolved commit. Stylo uses a pinned Git fork revision. Sibling source
checkouts are not required.

Run the browser example with an optimized build from this workspace:

```sh
cargo run -p gpui_webview --features servo --example browser --profile release-fast
```

The lifecycle example starts with a GPUI window and no browser views. Use
**Add view**, **Remove view**, **Hide all** / **Show all**, and **Close all** to inspect initialization,
multiple views, and resource release without exiting the host process. The
toolbar displays the process ID. An optional URL selects the page for new
views; the default is `about:blank`.

```sh
cargo run -p gpui_webview --features servo --example lifecycle --profile release-fast
```

Views share one Servo runtime per GPUI application. Closing all views releases
their documents and renderers; the shared runtime remains available for new
views. Compare memory against the initial host window in the same build and
window size, and allow time for background page reclamation.

Call `WebView::set_visible(false, cx)` before retaining a view outside the visible
layout. Hidden views detach their native surface and stop host animation polling
and painting, while preserving the document and history. Showing the view resumes
rendering. Hiding does not suspend JavaScript or network activity and does not
release the renderer. Use `close` to permanently release a view's resources.

Pass an absolute URL as the first argument to open a remote page:

```sh
cargo run -p gpui_webview --features servo --example browser --profile release-fast -- https://www.rust-lang.org/
```

Enter a URL in the example's address bar and press **Enter** or click **Go** to
navigate. Addresses without a scheme use HTTPS. The address bar follows page
navigation, including redirects and history changes.

Requests to open auxiliary windows are delivered through
`WebViewEvent::NewWindowRequested`. Applications can pass the request to
`WebView::from_window_request` in a new GPUI window, preserving the engine's
destination and opener semantics. Requests are handled once; dropping all
unhandled clones rejects the request. The browser example opens these requests
in separate windows and closes them when the engine accepts a script close.

The example's **Console** button opens page logs in a separate window, with the
newest messages first. It clears on navigation, retains up to 500 messages, truncates
each displayed message to 16,384 characters, and provides a **Clear** action.
Console output also goes to stderr. JavaScript evaluation and object inspection
are not available in this panel.

Applications can receive page console output and uncaught script errors through
`WebViewEvent::ConsoleMessage`, including the level and backend-provided text.

See [the architecture and implementation plan](PLAN.md) for backend boundaries, supported behavior, and remaining work.
