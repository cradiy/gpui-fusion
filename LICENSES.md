# Package licenses

This repository contains Apache-2.0 and MIT packages.

## Apache-2.0 package scope

The following Zed-derived packages use Apache-2.0 and retain their upstream
attribution. See [LICENSE-APACHE](LICENSE-APACHE).

| Package | Manifest directory |
| --- | --- |
| `collections` | `crates/collections` |
| `gpui` | `crates/gpui` |
| `gpui_linux` | `crates/gpui_linux` |
| `gpui_macos` | `crates/gpui_macos` |
| `gpui_macros` | `crates/gpui_macros` |
| `gpui_platform` | `crates/gpui_platform` |
| `gpui_shared_string` | `crates/gpui_shared_string` |
| `gpui_tokio` | `crates/gpui_tokio` |
| `gpui_util` | `crates/gpui_util` |
| `gpui_web` | `crates/gpui_web` |
| `hello_web` | `crates/gpui_web/examples/hello_web` |
| `gpui_wgpu` | `crates/gpui_wgpu` |
| `gpui_windows` | `crates/gpui_windows` |
| `http_client` | `crates/http_client` |
| `http_client_tls` | `crates/http_client_tls` |
| `refineable` | `crates/refineable` |
| `derive_refineable` | `crates/refineable/derive_refineable` |
| `reqwest_client` | `crates/reqwest_client` |
| `scheduler` | `crates/scheduler` |
| `sum_tree` | `crates/sum_tree` |
| `util_macros` | `crates/util_macros` |

## First-party MIT packages

First-party packages outside the Apache-2.0 scope are authored by
`cradiy <cradiy.rs@gmail.com>` and use the [MIT License](LICENSE-MIT), with
`Copyright (c) 2026 cradiy` and a package-local `LICENSE` file.

These include `gpui_3d`, `gpui_3d_effects`, `gpui_3d_gltf`, `gpui_android`,
`gpui_automation`, `gpui_effects`, `gpui_inspector`, `gpui_io`, `gpui_media`,
`gpui_media_backend`, `gpui_media_core`, `gpui_notifications`, `gpui_render`,
`gpui_wayland_source`, `uic`, `uic-macros`, and the standalone example packages
outside the Apache-2.0 scope.

## Third-party and inherited material

Third-party dependencies, bundled assets, and source copied from inherited
packages retain their licenses and attribution. In particular,
`uic/assets/icons/LICENSE` contains the Lucide ISC and Feather MIT notices.
