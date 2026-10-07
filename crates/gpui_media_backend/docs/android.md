# Play media on Android

Use `gpui_media::VideoPlayer` with `gpui_media_backend::SystemBackend` in an
application hosted by GPUiForge. The player renders inside the GPUI layout;
your application supplies the controls.

## Configure the application

Add the playback crates to `Cargo.toml`:

```toml
[dependencies]
gpui_media = { path = "path/to/gpui/crates/gpui_media", default-features = false, features = ["video"] }
gpui_media_backend = { path = "path/to/gpui/crates/gpui_media_backend" }
```

For network media, include the Internet permission in `gpuiforge.toml`:

```toml
[platforms.android]
application-id = "com.example.player"
permissions = ["android.permission.INTERNET"]
```

Run `gpuiforge run android` from the application directory. GPUiForge supplies
the Kotlin media host. For a manually maintained Android project, include the
matching host sources and their Media3 dependency.

## Choose a source

For a network URL:

```rust
use gpui_media::MediaSource;

let source = MediaSource::parse("https://example.com/video.mp4")?;
```

For a file returned by `App::prompt_for_files`, use its provider URL:

```rust
let source = MediaSource::from_uri(file.url().ok_or_else(|| {
    anyhow::anyhow!("The selected file does not expose a media URL")
})?)?;
```

On Android this is a `content://` URI. Keep the `FileHandle` alongside the player
for as long as it is used. To reopen a selected document after restarting the
application, persist and restore its access through the file API. A content URI
by itself does not grant access. For an application-owned filesystem path, use
`MediaSource::from_path` instead.

To configure authenticated HTTP requests:

```rust
use gpui_media::NetworkSourceOptions;
use std::time::Duration;

let options = NetworkSourceOptions::default()
    .with_bearer_token(token)?
    .with_timeout(Duration::from_secs(20));
let source = source.with_network_options(options);
```

Android accepts custom headers, user agent, and connect/read timeout. Other
explicit network options return an unsupported-operation error.

## Create and display the player

Create the player inside the running GPUI application:

```rust
use gpui::{AppContext, ParentElement, Styled, div, px};
use gpui_media::{VideoPlayer, VideoPlayerOptions};
use gpui_media_backend::SystemBackend;

SystemBackend::initialize()?;
let player = cx.new(|cx| {
    VideoPlayer::builder(source, SystemBackend)
        .options(VideoPlayerOptions {
            autoplay: false,
            ..Default::default()
        })
        .build(cx)
        .expect("create media session")
});

let content = div().w_full().h(px(240.)).child(player.clone());
```

Store `player` in your view and include it in `render`. File opening and decoder
errors arrive asynchronously through `VideoPlayerEvent::StateChanged` as
`PlaybackState::Error`. Subscribe to player events to update custom controls
and display errors.

## Control playback

Call controls through the entity's update context:

```rust
player.update(cx, |player, cx| player.play(cx))?;
player.update(cx, |player, cx| player.pause(cx))?;

player.update(cx, |player, cx| {
    player.seek_to(Duration::from_secs(10), gpui_media::SeekMode::Accurate, cx)
})?;
```

Use `SeekMode::KeyFrame` for a nearby keyframe, or `Accurate` for the requested
position. A paused player stays paused after seeking. Read `timeline()` for
position, duration, and seekability. Volume, mute, reload, and playback rates
greater than zero and up to 8 are also supported.

Retain the player when replacing the Activity's display Surface. Drop the player
and its subscriptions when the screen no longer needs it. Decide explicitly
whether application lifecycle changes should pause or resume playback.

## Supported media

Use local or HTTP(S) progressive media supported by Media3 and the device's
decoders. Track selection, subtitles, frame stepping, independent frame
extraction, DRM, adaptive-streaming extensions, and HDR output are unsupported.
Background playback, media notifications, audio focus, and picture-in-picture
require separate system integration.

Video uses CPU frame delivery, including GPU readback and upload. Check device
performance before using high-resolution videos or several players at once.

For a complete application, open
`crates/gpui_android/examples/media_android` and run `gpuiforge run android`.
Choose a file, then use Play, Pause, Seek, Restart, Reload, or Close.
