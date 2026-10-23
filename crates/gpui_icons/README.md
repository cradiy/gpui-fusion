# GPUI icons

Typed icon names and feature-selected SVG assets for GPUI. Icon names are always
available; their generated documentation identifies the feature required to
bundle the SVG. `GpuiIcons::required_feature()` exposes the requirement as
`Option<&'static str>` without depending on the embedded resources.

Register `IconAssets::new()` with `Application::with_assets`, then pass a
`GpuiIcons` value or its `path()` to `svg().path(...)`. Applications with other
assets can delegate icon loads to this same source. Unknown paths return `None`;
known but unbundled icons return an error identifying the required feature.
`AssetSource::list` returns only bundled resources.
Loading dispatches by the source prefix to a source-specific loader;
it does not convert paths back to enum values. Enum members use their source as
a prefix. `path()` returns the full source-qualified path; `Display` returns
the file name including its extension.

## Features

| Feature | Contents |
| --- | --- |
| `lucide-core` | Icons required by built-in components |
| `lucide-common` (default) | Common application actions, including `lucide-core` |
| `lucide-media` | Audio, video, playback, and photography |
| `lucide-files` | Documents, folders, storage, and libraries |
| `lucide-editing` | Typography, drawing, layout, and editing |
| `lucide-development` | Code, version control, packages, and servers |
| `lucide-connectivity` | Networking, messaging, and connections |
| `lucide-misc` | Remaining Lucide icons |
| `all` | Every collection |

Disable default features to select only the collections needed by the application.
UIC enables `lucide-core` explicitly; its examples also enable `lucide-common`.
An icon belongs to one packaging category, which does not affect its asset path.
All enabled collections use one `IconAssets` instance. SVG bytes are embedded in
both debug and release builds, including Android and Web builds.

## Asset layout and custom collections

SVG files live under `assets/<source>/<category>/`. Each collection is configured
explicitly in the procedural macro attributes. `file_enum` generates names,
paths, and feature documentation; `file_assets` independently generates embedded
bytes and path dispatch. Both macros are re-exported by `gpui_icons` and can be
used by applications for their own collections without extending `GpuiIcons`.

Configure groups in [src/icons.rs](src/icons.rs) for names and
[src/assets.rs](src/assets.rs) for embedding. Each group specifies its directory,
asset path prefix, and optional Cargo feature. Enum groups also specify a variant
prefix. Declare packaging features in Cargo.toml and include them in `all` when
they belong to the complete collection. Names remain available without the feature.

Public paths use `icons/<source>/<filename>.svg`, independent of category.
See [macro documentation](macros/README.md) for explicit configuration and the
generated interfaces. On stable Rust, adding directory entries requires touching
the annotated source to trigger regeneration.

The crate code is MIT licensed. Lucide and inherited Feather asset notices are
preserved in [assets/lucide/LICENSE](assets/lucide/LICENSE).
