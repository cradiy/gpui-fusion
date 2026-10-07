# Select media tracks

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

## Embedded text subtitles

Read `MediaInfo::subtitle_streams` for the available subtitle tracks and their
language, title, forced flag and selection state. Select an ID from that list,
or pass `None` to turn subtitles off:

```rust
player.update(cx, |player, cx| player.select_subtitle_stream(Some(&track_id), cx))?;
player.update(cx, |player, cx| player.select_subtitle_stream(None, cx))?;
```

Subscribe to `VideoPlayerEvent::Subtitle`. Store `SubtitleEvent::Cue` entries
for their stream and clear all stored embedded cues on `SubtitleEvent::Reset`.
Render cues for the selected stream when `cue.start <= position && position <
cue.end`. Some backends publish live snapshots whose end is `Duration::MAX`;
these stay valid until the next reset. Always handle resets, including while
paused and when the replacement contains no text.

The application owns subtitle composition and styling. `PlainText` cues contain
decoded text, not the original subtitle styling or positioning. For external
SRT, WebVTT or ASS/SSA files, use `parse_subtitles` and the same time filtering.
Keep external cue storage separate from embedded stream resets.
