# Select audio tracks

`VideoPlayer` uses the same stream API for audio-only and video sources.
Read `media_info()` after preparation, or subscribe to
`VideoPlayerEvent::MediaInfoChanged`, to obtain `MediaInfo::audio_streams`.
Each entry contains a `MediaStreamId`, a `selected` flag, and optional codec,
language, title, channel count, sample rate and bitrate metadata.

Pass an ID from the current list to select a track:

```rust
player.update(cx, |player, cx| player.select_audio_stream(&track_id, cx))?;
```

Selection preserves the shared playback timeline. Refresh controls from
`MediaInfoChanged` and its `selected` flags; a successful request may be applied
asynchronously. Missing metadata remains `None`, so controls should provide a
fallback label, such as the track number.

Treat stream IDs as opaque and local to the current session. Refresh the list
after a reload or source change instead of persisting an ID. Unsupported
selection returns a `MediaError`; available streams depend on the source,
backend and installed decoders. GStreamer, Windows Media Foundation and Android
Media3 support audio selection. The browser backend does not expose it.
