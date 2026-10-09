# GPUI Web

`gpui_web` brings GPUI applications to the browser, with support for user
interfaces, basic 3D scenes, video, and audio.

## Requirements

- Rust with the `wasm32-unknown-unknown` target.
- [Trunk](https://trunkrs.dev/) to build and serve the examples.
- A browser with WebGPU enabled, using localhost or HTTPS.

Install the build tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

## Threading

For a single-threaded application on stable Rust, disable the platform dependency's
default features:

```toml
gpui_platform = { path = "../gpui/crates/gpui_platform", default-features = false }
```

`gpui_platform::application()` uses the single-threaded dispatcher when built
without `multithreaded`. `single_threaded_web()` also disables workers at runtime,
but does not change Cargo features. Direct `gpui_web` dependencies must likewise
set `default-features = false` for stable single-threaded builds.

`gpui_platform` enables `multithreaded` by default. To opt in when defaults are
disabled, add `features = ["multithreaded"]`. Multithreaded builds require nightly
Rust, a rebuilt standard library with WebAssembly atomics and shared memory, and
COOP/COEP response headers. The `hello_web` example includes that configuration.
Single-threaded builds do not need those flags or isolation headers. Cargo features
are additive: another dependency enabling `multithreaded` enables it for the build.

## Examples

Run these commands from `crates/gpui_web`. Start Trunk inside the example
directory so it uses that example's configuration, then open the address printed
in the terminal.

### Hello World

A minimal GPUI application. This example uses nightly Rust:

```sh
rustup toolchain install nightly --component rust-src --target wasm32-unknown-unknown
cd examples/hello_web
trunk serve
```

### 3D, video, and audio

An interactive cube and media playback example. It supports stable Rust:

```sh
cd examples/media_web
trunk serve
```

Click **Load video** or **Load audio** to choose a local file, then **Play**.
Use **Pause** and **Seek +2s** to control playback, or **Rotate cube** to change
the 3D view. Selected files stay on your device.

## Browser support

Available media formats and graphics capabilities depend on the browser and
device. Some desktop integrations, including native screen picking, are not
available on Web.

If graphics initialization fails, check that WebGPU is enabled and supported by
your browser. If the page reports a lost graphics connection, reload it.
