# GPUI

GPUI is a GPU-accelerated UI framework for Rust, combining retained application
state with declarative views and custom rendering. It provides layout, text,
images, input handling, window management, and asynchronous tasks.

This crate is the core of [GPUI Fusion](../../README.md), an independently
maintained repository derived from [Zed](https://github.com/zed-industries/zed).
Platform backends and optional libraries provide desktop, Android, and web
integration, reusable components, rich text, media, visual effects, and 3D scenes.

## Getting started

Use the repository's Rust toolchain. GPUI is under active development and its
APIs may change between revisions.

For a desktop application using this repository, add both crates from the same
source:

```toml
[dependencies]
gpui = { git = "https://github.com/cradiy/gpui-fusion" }
gpui_platform = { git = "https://github.com/cradiy/gpui-fusion" }
```

`gpui_platform::application()` selects the host platform. Call `Application::run`
to initialize application state, then `App::open_window` to create a window with
a root view implementing `Render`.

The default features include Wayland for Linux and FreeBSD, and `font-kit` for
macOS. Windows uses its native backend. No explicit feature selection is needed
for the default desktop setup.

Run the introductory example from the repository root:

```sh
cargo run -p gpui --example hello_world
```

See the [example source](examples/hello_world.rs) for a complete application and
the [examples index](examples/README.md) for layout, input, drawing, and window
examples. Android and web setup are covered by the
[Android hosting guide](../gpui_android/docs/hosting.md) and
[web backend](../gpui_web/README.md).

## Project entry points

- [Repository overview](../../README.md)
- [Context guide](docs/contexts.md)
- [UI components](../../uic)
- [Rich text and Markdown](../gpui_richtext/README.md)
- [Icons](../gpui_icons/README.md)
- [Visual effects](../gpui_effects/README.md)
- [Media](../gpui_media/README.md)
- [3D scenes](../gpui_3d/README.md)

## License

GPUI retains its upstream Zed attribution and is licensed under
[Apache-2.0](../../LICENSE-APACHE). See the repository's
[package license assignments](../../LICENSES.md) for companion crates and assets.
