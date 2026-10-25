# Audio player

`AudioPlayer` is a presentation-free audio component. It is independent from
`VideoPlayer`. By default, Symphonia performs demuxing and decoding on a worker
thread and CPAL writes PCM to the default native output device. On Web, it uses
browser audio playback. A caller-selected `MediaBackend` uses the shared
`MediaPlayer` session controls and requires no video feature. Audio
and video are enabled by default. An audio-only application can disable the
default features:

```toml
gpui_media = {
    path = ".../gpui_media",
    default-features = false,
    features = ["audio"],
}
```

The component owns playback state only. It does not provide buttons, labels,
sliders, artwork or another built-in interface.

## Files and network streams

Create an `AudioSource` from a local path or an HTTP(S) URL, then store the
player in a GPUI entity:

```rust
use gpui::{Context, Entity};
use gpui_media::{AudioPlayer, AudioPlayerOptions, AudioSource, MediaResult};

fn open_audio(
    input: &str,
    cx: &mut Context<MyView>,
) -> MediaResult<Entity<AudioPlayer>> {
    let source = AudioSource::parse(input)?;
    Ok(cx.new(|cx| AudioPlayer::new(
        source,
        AudioPlayerOptions::default(),
        cx,
    ).expect("failed to create audio player")))
}

# struct MyView;
```

Local files are seekable. HTTP(S) sources are read sequentially and therefore
also work with chunked responses and live endpoints; they do not require HTTP
byte-range support. A sequential network source reports `is_seekable() ==
false`.

Use `MediaSource` before converting to `AudioSource` when the request needs
headers, basic authentication, a user agent, a timeout or a proxy:

```rust
use gpui_media::{AudioSource, MediaSource, NetworkSourceOptions};

let network = NetworkSourceOptions::default()
    .with_header("X-Playback-Token", "token")?;
let media = MediaSource::from_uri("https://example.test/live.mp3")?
    .with_network_options(network);
let source = AudioSource::from(media);
# Ok::<(), gpui_media::MediaError>(())
```

## Optional media backends

The built-in decoder does not require `gpui_media_backend`. To select another
implementation, pass a backend to the audio builder:

```rust
use gpui::Context;
use gpui_media::{AudioPlayer, AudioSource, MediaBackend, MediaResult};

fn backend_audio(
    source: AudioSource,
    backend: impl MediaBackend,
    cx: &mut Context<AudioPlayer>,
) -> MediaResult<AudioPlayer> {
    AudioPlayer::builder(source).backend(backend).build(cx)
}
```

`SystemBackend` selects Media3 on Android, GStreamer on Linux/macOS, Media
Foundation on Windows, and browser media playback on Web. It is optional on all
platforms. Backend-specific configuration and host requirements still apply.
Audio requests use `MediaPlaybackMode::AudioOnly`: system backends disable video
tracks and frame output, including for containers with both audio and video.
On Android, the built-in decoder supports audio focus, automatic pause when an
output device is disconnected, and optional CPU/Wi-Fi wake locks independently
of Media3. Enable the GPUiForge `audio-playback` host feature. Wake locks require
`android.permission.WAKE_LOCK`; select `PlaybackWakeMode::Local` for local files
or `Network` for network streams. Focus is requested before playback and released
on pause, completion, failure, or drop. Transient focus loss pauses playback and
resumes on focus gain unless the application paused in the meantime. Ducking
attenuates output without changing the application's volume setting. When a
route change invalidates the native output stream, playback reopens the default
output without clearing play intent. A paused player remains paused.

Use GPUI system media controls and the GPUiForge `media-notifications` and
`background-media` features for lock-screen controls and foreground execution.
These are separate from the decoder. Android focus requests require a foreground
application or an eligible foreground service on recent Android releases.
Other built-in outputs report unsupported focus and wake-lock operations.
Selected media backends provide these controls according to their capabilities.

`AudioPlayerBuilder::prepare` opens a session without starting playback or
creating a GPUI entity. Run preparation off the UI thread when the backend can
block; browser operations stay on the browser thread. Attach the result with
`PreparedAudioPlayer::build` inside `cx.new`. Keep ownership with the active
playback request and discard superseded results. Dropping an unbuilt result
releases its session. Command failures after attachment are player error states.
Caller-fed encoded streams use the built-in decoder and cannot be passed to a
URI-based system backend.

## Caller-fed encoded streams

Use a bounded stream when another task owns transport, authentication or
protocol framing. Chunks contain encoded bytes, not PCM. The extension or MIME
hint helps Symphonia select the demuxer before enough bytes are available for
content probing.

```rust
use gpui_media::{AudioSource, AudioStreamHint};

async fn feed_audio(first_chunk: Vec<u8>, second_chunk: Vec<u8>)
    -> gpui_media::MediaResult<AudioSource>
{
    let (writer, source) = AudioSource::stream(
        8,
        AudioStreamHint::new().with_extension("mp3"),
    )?;

    // Construct AudioPlayer from `source.clone()` on the GPUI thread.
    // `write` waits when eight chunks are pending, so a fast producer cannot
    // grow memory without bound.
    writer.write(first_chunk).await?;
    writer.write(second_chunk).await?;
    writer.close(); // End-of-stream after queued chunks are decoded.
    Ok(source)
}
```

A caller-fed source is one-shot. Opening it with a second player returns an
error. Dropping the player closes its input channel and releases a producer
waiting for queue capacity.

## Composing controls

Call playback methods from the host view. `AudioPlayer` emits events but does
not choose how they are displayed:

```rust
use gpui::Context;
use gpui_media::{AudioPlayer, PlaybackState, SeekMode};
use std::time::Duration;

fn toggle(player: &mut AudioPlayer, cx: &mut Context<AudioPlayer>) {
    match player.state() {
        PlaybackState::Playing | PlaybackState::Loading => player.pause(cx),
        _ => player.play(cx).expect("failed to resume audio"),
    }
}

fn seek(player: &mut AudioPlayer, cx: &mut Context<AudioPlayer>) {
    if player.is_seekable() {
        player
            .seek_to(Duration::from_secs(30), SeekMode::Accurate, cx)
            .expect("failed to seek audio");
    }
}
```

Subscribe to `AudioPlayerEvent` when a parent component needs to redraw its own
controls or synchronize application state:

```rust
cx.subscribe(&player, |view, _, event, cx| {
    match event {
        AudioPlayerEvent::StateChanged(state) => view.state = state.clone(),
        AudioPlayerEvent::TimelineChanged(timeline) => view.position = timeline.position(),
        AudioPlayerEvent::InfoChanged(info) => view.sample_rate = info.sample_rate(),
        AudioPlayerEvent::VolumeChanged { .. } => {}
        _ => {}
    }
    cx.notify();
}).detach();
```

The complete cross-platform example provides play, pause, seek and mute
controls:

```sh
cargo run -p gpui_media --example audio -- /path/to/audio.mp3
cargo run -p gpui_media --example audio -- https://example.test/live.mp3
cargo run -p gpui_media --example audio -- --stream mp3 /path/to/audio.mp3
```

## Playback behavior

- Decoding and blocking input reads run outside the GPUI thread.
- Decoded PCM is held near a 500 ms target instead of decoding the entire
  source into memory.
- The caller-fed input queue is bounded and applies asynchronous backpressure.
- Pause, mute and volume changes reach the CPAL callback without waiting for a
  blocked network read.
- The playback timeline advances only for PCM consumed by the audio device.
- Stream completion is reported after queued PCM has drained.
- Mid-stream sample-rate or channel-layout changes currently produce an error;
  create a new source when the encoded stream changes format.

## Web

Use HTTP(S), `blob:` or `data:` URLs with the same `AudioPlayer` API. Playback
format support, loading, decoding and output are managed by the browser. Call
`play` from a user interaction when autoplay is blocked; failures are reported
through the player state. Duration and seekability follow the browser timeline.

The browser implementation does not accept native filesystem paths, custom
network options or caller-fed encoded streams. Sample rate and channel count
are reported as zero when unavailable, and the codec label is `browser`.
