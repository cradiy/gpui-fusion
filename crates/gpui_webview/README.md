# gpui_webview

Native browser views for GPUI, with an engine-independent component API and optional rendering backends.

The experimental Servo backend targets Linux Wayland. Enable `servo` to build it and select `WebViewBackend::Servo` when constructing a `WebView`.

Servo and Stylo use pinned Git forks; sibling source checkouts are not required.

Run the browser example from this workspace:

```sh
cargo run -p gpui_webview --features servo --example browser
```

See [the architecture and implementation plan](PLAN.md) for backend boundaries, supported behavior, and remaining work.
