# Configure media support

Enable the `video` Cargo feature to use `VideoPlayer` and inject `SystemBackend`:

```toml
[dependencies]
gpui_media = { path = "path/to/gpui/crates/gpui_media", default-features = false, features = ["video"] }
gpui_media_backend = { path = "path/to/gpui/crates/gpui_media_backend" }
```

`SystemBackend` selects the platform implementation. Player creation, sources,
controls, audio tracks and frame extraction use the shared
[media API](../README.md).

## Host features

For GPUiForge Android projects, select the host modules in `gpuiforge.json`:

```json
{
  "platforms": {
    "android": {
      "features": ["media"]
    }
  }
}
```

| Feature | Enable when using |
| --- | --- |
| `media` | `SystemBackend` playback or frame extraction |
| `files` | System file selection and document access |
| `media-notifications` | `SystemMediaSession` notifications and transport controls |
| `background-media` | Explicit background playback leases; includes `media-notifications` |

Add only the permissions needed by the application to
`platforms.android.permissions`:

| Permission | Required for |
| --- | --- |
| `android.permission.INTERNET` | Network media |
| `android.permission.WAKE_LOCK` | Explicit `PlaybackWakeMode::Local` or `Network` |

Run `gpuiforge sync` after changing the configuration. Manually maintained
hosts must include the matching modules and dependencies themselves.

## Desktop dependencies

Linux and macOS require GStreamer development libraries at build time and
runtime libraries with the plugins needed by the source media. Select at least
one version feature on `gpui_media_backend`: `v1_24` (default), `v1_26`, or
`v1_28`. These require GStreamer 1.24, 1.26 or 1.28 respectively. Features are
cumulative; the highest enabled version determines the minimum requirement.

Windows uses Media Foundation; Android uses the configured Media3 host; Web
uses browser media APIs. They require no GStreamer installation or version
feature.

Applications distributing GStreamer must account for the libraries and plugins
they package; see the [GStreamer distribution guidance](https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/licensing.html).

See [backend capabilities](backend-capabilities.md) for operation availability.
