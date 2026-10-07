# System media controls

For Android foreground playback leases that reuse the media notification, see
[background execution](../../gpui_android/docs/background.md#media-playback).

`SystemMediaSession` publishes an application's active playback to Android
MediaSession and MediaStyle notifications, Linux MPRIS, Windows system media
transport controls, macOS Now Playing, or the browser Media Session API.
These surfaces follow the operating system's presentation policy.

Android applications enable `"media-notifications"` in
`platforms.android.features`. Add `"media"` when using GPUI's Android media
backend. Configure `"notification-icon": "assets/notification.xml"` for a default
small icon, or pass a packaged resource name per session update. Run
`gpuiforge sync` after changing the host configuration.

## Publish playback

Create one session for the application's active player:

```rust
use gpui::gpui_notifications::*;
use std::time::Duration;

let request = cx.system_media_session(MediaSessionOptions {
    app_id: "com.example.player".into(),
    app_name: "Example Player".into(),
});
let mut session = request.await?;
let commands = session.take_commands().expect("one command consumer");
session.update(MediaSessionState {
    metadata: MediaMetadata {
        track_id: "track-42".into(),
        title: "Example track".into(),
        artist: Some("Example artist".into()),
        ..Default::default()
    },
    playback: MediaPlayback::Playing,
    position: Duration::from_secs(12),
    duration: Some(Duration::from_secs(180)),
    seekable: true,
    icon: Some(NotificationIcon::Resource("ic_media".into())),
    ..Default::default()
})?;
```

Retain the session, publish complete snapshots after state changes, and refresh
the timeline periodically during playback. Use a stable `track_id` and change
it when selecting another item so stale system seek requests can be rejected.
After a discontinuous position change, call `session.seeked(position)`.
Rates must be finite and positive; volume ranges from 0 to 1.

Receive commands on a foreground task and route them to the player. Play,
pause, toggle, stop, next, previous, absolute/relative seeking, and volume
changes are represented as `MediaCommand`. Set `can_next`, `can_previous`, and
`seekable` according to actual application capabilities. Individual platforms
expose different subsets. Playlist traversal and fetching media remain the
application's responsibility.

## Bind a GPUI video player

```rust
use gpui_media::VideoSystemMediaOptions;

player.update(cx, |player, cx| {
    player.set_system_media_session(Some(session), VideoSystemMediaOptions {
        metadata: MediaMetadata {
            track_id: "track-42".into(),
            title: "Example track".into(),
            ..Default::default()
        },
        icon: Some(NotificationIcon::Resource("ic_media".into())),
        ..Default::default()
    }, cx);
});
```

The binding forwards playback and seek commands, publishes state changes and
periodic position updates, and reports next/previous requests through
`VideoPlayerEvent::SystemMediaAction`. Session errors are reported through
`SystemMediaError` without stopping local playback. To withdraw the system
surface, bind `None` or drop the player. Avoid registering sessions for incidental
previews.

## Publish cover artwork

Cover artwork is separate from the notification's small application icon.
Prepare it on a worker, then update the session independently of its timeline:

```rust
let artwork = cx.background_executor().spawn(async move {
    MediaArtwork::from_encoded(&image_bytes)
}).await?;
session.set_artwork(Some(artwork))?;
session.set_artwork(None)?; // Clear when the next item has no cover.
```

`MediaArtwork::from_encoded` accepts PNG/JPEG. `from_rgba(width, height, pixels)`
accepts tightly packed RGBA8 pixels, including application-extracted video frames.
Preparation is synchronous and fits the image within 512 × 512 without upscaling.
Encoded inputs are limited to 16 MiB, dimensions to 4096 pixels per edge, and
decoder allocations to 64 MiB. Read files with a corresponding byte limit.
The application owns file selection, downloads and frame extraction.

For `VideoPlayer`, pass the prepared cover in `VideoSystemMediaOptions::artwork`
when binding a session, or call `player.set_system_media_artwork(Some(artwork))`
to replace it later. Passing `None` clears it without resetting playback controls.
Ordinary metadata and position updates retain the current cover. Clear or replace
it explicitly when changing tracks.

| Platform | System surface |
| --- | --- |
| Android | MediaSession album artwork and MediaStyle large icon, including foreground playback |
| Linux / FreeBSD | MPRIS `mpris:artUrl` pointing to a session-retained temporary PNG |
| Windows | System media transport controls thumbnail |
| macOS | Now Playing `MPMediaItemArtwork` |
| Web | Media Session artwork backed by a browser object URL |

Cloning a prepared cover shares its data. Position updates do not re-encode or
re-send the image. Clearing or replacing it releases the session's reference;
desktop temporary files remain until the last cover reference is dropped.
Browser object URLs are revoked on replacement or closure. Presentation, cropping
and whether artwork is visible depend on the OS, desktop shell or browser.
Web content policies must allow `blob:` images when restricting `img-src`.

## Platform ownership

On Android, `app_id` must match the package. Supply a monochrome drawable icon
or an application icon. Unsupported custom icon kinds fall back to the
application icon. The media surface uses the `gpui.media` notification channel
and is separate from general notification channels. Android manages eligibility
and notification permissions for media sessions. Audio focus is configured
separately on the media backend.

Linux requires a session bus; the MPRIS name includes the application ID and
process ID. Windows requires the GPUI message loop/COM initialization. macOS
and Web have one active system media owner per application/page. Their system
surfaces use the application identity instead of an Android drawable.

A session controls presentation and command delivery. It does not decode media,
persist a playlist, or resume after process death. Android foreground execution
is requested explicitly through `start_background_playback()`; merely creating
or updating a media session does not start a service. Applications retain
ownership of playback and its host lifecycle.
