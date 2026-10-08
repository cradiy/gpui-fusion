# gpui_media_backend

`SystemBackend` selects the media backend for the target platform. Use
[gpui_media](../gpui_media/README.md) for playback, stream selection and frame
extraction.

## Platform features

| Platform | Cargo features on `gpui_media_backend` | GPUiForge host features |
| --- | --- | --- |
| Linux / macOS | `v1_24` (default), `v1_26`, or `v1_28`; enable at least one | None |
| Windows | No additional features | None |
| Android | No additional Cargo features | `media` |
| Web | No additional features | None |

GStreamer version features require the corresponding system version or newer
on Linux/macOS. They do not affect the other platforms.

For Android, add `files` for file selection, `media-notifications` for system
media controls, and `background-media` for background playback leases.
`background-media` includes `media-notifications`.

See [media configuration](../gpui_media/docs/configuration.md) for dependencies,
host configuration and permissions. All playback usage is documented in
[gpui_media](../gpui_media/README.md).
