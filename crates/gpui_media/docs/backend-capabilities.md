# Backend capabilities

Use the same `VideoPlayer` and `VideoFrameExtractor` APIs with `SystemBackend`
on every platform. Consult `backend_capabilities()` before enabling controls
that the current session may not support.

Linux supports CPU frames and DMA-BUF transport, including consumer-advertised
native NV12 modifiers. macOS can deliver CoreVideo frames and CPU frames.
Windows delivers CPU frames through Media Foundation and WIC. Decoder and
container support depend on the system's installed media components.

Android supports device-decodable progressive local and HTTP(S) media, HLS/DASH
on-demand streams, audio track selection, text subtitles, and GPU or CPU frame delivery.
Frame stepping, DRM and HDR output are unavailable. Extraction uses
`VideoDecoderPolicy::Auto` and `SeekMode::Accurate`; explicit decoder policies,
other seek modes, audio-only sources and positions beyond a known video duration
return errors. HTTP options support headers, user agent and connect/read timeout;
other explicit options return unsupported-operation errors.

Use `VideoPlayer::builder(...).build_in_window(window, cx)` to negotiate native
frame transport with the renderer. On compatible Android Vulkan devices,
`FrameTransport::HardwareBuffer` keeps playback pixels on the GPU, including
color conversion and the copy into GPUI's texture. Unsupported devices or
failed imports fall back to CPU frames. Independent frame extraction returns
CPU pixels. `VideoPlayer::frame_transport()` reports the transport actually used.

HLS/DASH use the same source, playback, seeking and stream-selection APIs as
other media. See [network sources](playback-lifecycle.md#network-sources) for
extensionless URLs. Live-edge controls and moving seek-window geometry are not
exposed by the shared timeline.

Android exposes embedded SubRip, WebVTT, SSA/ASS, TTML and tx3g text tracks
supported by Media3. Bitmap subtitles are not exposed. Text is delivered as
live plain-text cues; formatting and positioning are not retained.

`MediaPlaybackRequest::output_capabilities` describes native layouts accepted
by the consumer. It does not choose the decoder or its device. GStreamer
selects decoders from its plugin registry; CPU frame output does not imply
software decoding. Playback sessions do not expose an explicit hardware-decoder
selection policy; see [extraction policies](#extraction-policies).

`VideoFrame::decoder_info()` exposes a snapshot of the observed video decoder.
GStreamer reports the factory name after its output pad produces raw video,
matched to the output stream. Acceleration follows the factory's `Hardware`
classification; it is not a measurement of GPU usage. Available device
properties use backend-specific names, such as `device-path` or `cuda-device-id`.
Metadata is absent when no unique decoder can be identified. Media Foundation
does not currently expose decoder metadata through this backend.

Decoder metadata is available on both playback and extracted frames. It is
independent of `VideoFrame::transport()`: a hardware decoder can deliver CPU
frames. Retained frames keep their metadata when the session changes or closes.

On Linux/macOS, network source options configure supported GStreamer source
properties. Windows rejects custom network options that Media Foundation does
not expose through this backend. Session capabilities report the operations
available for the opened source.

## Extraction policies

For GStreamer, `VideoFrameExtractorOptions::timeout` is one shared waiting
budget per active extraction request, including initial preroll and seek
retries. Time spent queued behind another request is not included. Synchronous
plugin calls cannot be forcibly interrupted by this budget.

On Linux/macOS, `VideoFrameExtractorOptions::video_decoder` selects the video
decoder policy for each extraction session:

| Policy | Allowed video decoders |
| --- | --- |
| `Auto` (default) | System registry selection |
| `SoftwareOnly` | Decoders without the GStreamer `Hardware` classification |
| `HardwareOnly` | Decoders with the GStreamer `Hardware` classification |

```rust
use gpui_media_backend::{VideoDecoderPolicy, VideoFrameExtractorOptions};

let options = VideoFrameExtractorOptions {
    video_decoder: VideoDecoderPolicy::SoftwareOnly,
    ..Default::default()
};
let extractor = VideoFrameExtractor::with_options(source, options, backend)?;
```

Explicit policies filter candidates within the session and verify the observed
decoder before returning frames. They do not change global plugin ranks or
other sessions. Missing compatible decoders, unidentifiable decoders, or failed
output negotiation return errors; an excluded decoder is not a fallback.
The policy controls decoder classification, not a plugin's internal execution
or fallback behavior, and does not select a GPU device or frame transport.

Automatic extraction uses `playbin3`; explicit policies use `playbin` with
`uridecodebin` candidate filtering. Windows supports `Auto` and rejects explicit
policies with `MediaErrorKind::Unsupported`. Playback sessions use automatic
decoder selection independently of extraction options.

## Browser playback

See the [browser playback contract](../README.md#browser-playback) for source,
codec, threading and operation restrictions.
