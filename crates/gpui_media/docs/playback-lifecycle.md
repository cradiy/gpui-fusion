# Playback lifecycle

Retain the `VideoPlayer` entity for as long as playback is needed. Rebuilding a
window or its rendering surface need not recreate the player. Drop its entity
and subscriptions when playback ends. Application lifecycle policy determines
whether leaving a screen should pause playback or retain the session.

## Selected documents

For a file returned by `App::prompt_for_files`, use its provider URL:

```rust
let source = MediaSource::from_uri(file.url().ok_or_else(|| {
    anyhow::anyhow!("The selected file does not expose a media URL")
})?)?;
```

Keep the `FileHandle` alive alongside playback or until extraction requests
finish. To reopen a document after restarting the application, persist and
restore its access through the file API. A provider URL alone does not grant
access. Use `MediaSource::from_path` for application-owned filesystem paths.

## Audio focus

Observe `VideoPlayerEvent::StateChanged` to keep controls synchronized with
system pauses and resumes. If the application manages audio focus itself,
create a paused player and disable backend focus management before playing:

```rust
player.update(cx, |player, _| player.set_audio_focus_enabled(false))?;
player.update(cx, |player, cx| player.play(cx))?;
```

The Android backend manages focus by default: temporary interruptions can
pause or lower volume, permanent loss pauses playback, and headphone
disconnection pauses playback independently of focus management. Backends
without this control return an unsupported-operation error.

## Screen-off playback

Choose a power policy explicitly when the backend supports it:

```rust
use gpui_media::PlaybackWakeMode;

player.update(cx, |player, _| player.set_wake_mode(PlaybackWakeMode::Local))?;
```

`None` is the default. `Local` requests CPU wakefulness while playback needs it.
`Network` additionally requests a Wi-Fi lock; it is not required merely because
the source is a URL. Locks follow playback state and release on pause, end,
failure or player release. `None` disables them. The display may still turn off.
Independent extraction does not acquire playback wake locks.

Android requires the `WAKE_LOCK` permission in the
[host configuration](configuration.md).
Unsupported backends return an unsupported-operation error. Power policy does
not provide foreground execution or recovery after process termination.

## System controls and background execution

Attach a `SystemMediaSession` using `VideoPlayer::set_system_media_session` for
system transport controls, metadata and artwork. Use the shared
[system media guide](../../gpui_notifications/docs/media.md) on every platform.
The binding follows the player's lifetime; avoid separate sessions for
incidental previews.

Where foreground execution is required, retain a
[background playback lease](../../gpui_android/docs/background.md#media-playback).
Creating a player or a media notification does not itself start a foreground
service. A lease does not supply playlist persistence or resume playback after
process death.

## Picture-in-picture

Use the window API when `window.supports_picture_in_picture()` is true:

```rust,ignore
let request = window.enter_picture_in_picture(gpui::size(16, 9), cx);
cx.spawn(async move |_, _| {
    if let Err(error) = request.await {
        log::warn!("Picture-in-picture request failed: {error:#}");
    }
}).detach();
```

The ratio describes the content width and height; the system may limit its
range or reject entry. Register `Window::on_picture_in_picture_changed` to
refresh the view, and render a compact video surface and subtitles when
`Window::is_picture_in_picture()` is true. Keep the same player entity alive
across both layouts. The system provides controls for returning to the full
window or closing it. Entry is explicit; going Home does not request it.

Android requires the GPUiForge `media` host feature. A visible picture-in-picture
window continues rendering while the Activity is paused. Do not pause playback
solely because it loses input focus. Bind a `SystemMediaSession` for system
play/pause controls; picture-in-picture does not create a separate media session
or background service. Other window backends currently report unsupported.
