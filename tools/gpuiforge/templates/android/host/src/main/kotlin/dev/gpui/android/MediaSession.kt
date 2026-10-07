package dev.gpui.android

import android.content.Context
import android.content.pm.PackageManager
import android.os.Handler
import android.os.HandlerThread
import androidx.media3.common.C
import androidx.media3.common.AudioAttributes
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.text.CueGroup
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import java.nio.ByteBuffer
import java.util.concurrent.atomic.AtomicBoolean

/** A media session owns its decoder and output independently of the hosting View. */
@UnstableApi
internal class MediaSession(
    context: Context,
    private val id: Long,
    uri: String,
    mimeType: String?,
    keys: Array<String>,
    values: Array<String>,
    timeout: Int,
    private val extractionPosition: Long,
) : AutoCloseable {
    private val application = context.applicationContext
    private val mediaItem = MediaItem.Builder().setUri(uri).setMimeType(when (mimeType) {
        "application/vnd.apple.mpegurl", "application/x-mpegurl", "audio/mpegurl", "audio/x-mpegurl" -> MimeTypes.APPLICATION_M3U8
        else -> mimeType
    }).build()
    private val thread = HandlerThread("gpui-media").apply { start() }
    private val handler = Handler(thread.looper)
    private val closed = AtomicBoolean(false)
    private var player: ExoPlayer? = null
    private var frames: MediaFrames? = null
    private var systemControls: SystemMediaControls? = null
    private val tracks = MediaTracks(id) { revision, tracks -> nativeTracks(id, revision, tracks) }
    private var lastSubtitle = ""
    private var expectedSubtitle: String? = null
    private var subtitleSelectionPending = false
    @Volatile private var generation = 0L
    private var failed = false
    private val tick = object : Runnable {
        override fun run() {
            if (closed.get() || failed) return
            reportState()
            val current = player ?: return
            if (current.isPlaying || current.playbackState == Player.STATE_BUFFERING) {
                handler.postDelayed(this, 100)
            }
        }
    }

    fun setWakeMode(mode: Int): String? {
        if (closed.get()) return "Media session closed"
        if (extractionPosition >= 0) return "Frame extraction does not support playback wake locks"
        if (mode != C.WAKE_MODE_NONE && mode != C.WAKE_MODE_LOCAL && mode != C.WAKE_MODE_NETWORK) {
            return "Invalid playback wake mode"
        }
        if (mode != C.WAKE_MODE_NONE && application.checkSelfPermission(android.Manifest.permission.WAKE_LOCK) != PackageManager.PERMISSION_GRANTED) {
            return "Playback wake locks require android.permission.WAKE_LOCK in gpuiforge.json permissions"
        }
        if (!handler.post { guarded { player?.setWakeMode(mode) } }) return "Media session closed"
        return null
    }

    fun selectStream(key: String?, text: Boolean): String? {
        if (closed.get()) return "Media session closed"
        if (extractionPosition >= 0) return "Frame extraction does not select streams"
        val type = if (text) C.TRACK_TYPE_TEXT else C.TRACK_TYPE_AUDIO
        val choice = key?.let { tracks.find(it) }
        if (key != null && (choice == null || choice.type != type)) return "Unknown or unavailable stream"
        if (!text && choice == null) return "An audio stream is required"
        if (!handler.post {
            guarded {
                val current = player ?: return@guarded
                // A reload or track-list change can invalidate a queued request.
                if (key != null && tracks.find(key) != choice) return@guarded
                if (text) {
                    expectedSubtitle = key
                    subtitleSelectionPending = true
                    clearSubtitles()
                }
                val parameters = current.trackSelectionParameters.buildUpon()
                    .setTrackTypeDisabled(type, choice == null).clearOverridesOfType(type)
                if (choice != null) parameters.setOverrideForType(TrackSelectionOverride(choice.group, choice.index))
                current.trackSelectionParameters = parameters.build()
                if (text && tracks.selectedSubtitle == key) {
                    subtitleSelectionPending = false
                    reportSubtitles(current.currentCues)
                }
            }
        }) return "Media session closed"
        return null
    }

    init {
        handler.post {
            guarded {
                val output = MediaFrames(handler) { pixels, width, height, pts, revision ->
                    if (!closed.get() && !failed && revision == generation && validateExtraction()) {
                        nativeFrame(id, revision, pixels, width, height, pts)
                    }
                }
                frames = output
                output.onError = { fail(0, "Android video output failed (${it.javaClass.simpleName})") }
                output.initialize()
                val headers = keys.zip(values).toMap()
                val http = DefaultHttpDataSource.Factory()
                    .setDefaultRequestProperties(headers)
                    .setConnectTimeoutMs(timeout)
                    .setReadTimeoutMs(timeout)
                headers.entries.firstOrNull { it.key.equals("User-Agent", true) }?.let {
                    http.setUserAgent(it.value)
                }
                val renderers = DefaultRenderersFactory(application)
                if (extractionPosition >= 0) {
                    renderers.forceDisableMediaCodecAsynchronousQueueing()
                }
                val current = ExoPlayer.Builder(application, renderers)
                    .setWakeMode(C.WAKE_MODE_NONE)
                    .setLooper(thread.looper)
                    .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(application, http)))
                    .build()
                player = current
                if (extractionPosition < 0) {
                    current.setAudioAttributes(AudioAttributes.DEFAULT, true)
                    current.setHandleAudioBecomingNoisy(true)
                }
                if (extractionPosition >= 0) {
                    current.trackSelectionParameters = current.trackSelectionParameters.buildUpon()
                        .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, true)
                        .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
                        .build()
                    current.setSeekParameters(SeekParameters.EXACT)
                }
                current.addListener(object : Player.Listener {
                    override fun onEvents(player: Player, events: Player.Events) {
                        if (closed.get() || failed) return
                        if (!validateExtraction()) return
                        tracks.report(player.currentTracks, generation)
                        if (subtitleSelectionPending && tracks.selectedSubtitle == expectedSubtitle) {
                            subtitleSelectionPending = false
                        }
                        if (events.contains(Player.EVENT_CUES) || events.contains(Player.EVENT_TRACKS_CHANGED)) {
                            reportSubtitles(player.currentCues)
                        }
                        if (player.playbackState == Player.STATE_ENDED) clearSubtitles()
                        val size = player.videoSize
                        if (size.width > 0 && size.height > 0) {
                            output.width = (size.width * size.pixelWidthHeightRatio).toInt().coerceAtLeast(1)
                            output.height = size.height
                        }
                        handler.removeCallbacks(tick)
                        tick.run()
                    }
                    override fun onPlayerError(error: PlaybackException) {
                        fail(error.errorCode, error.errorCodeName)
                    }
                })
                current.setVideoFrameMetadataListener { pts, release, format, _ ->
                    val rotated = format.rotationDegrees == 90 || format.rotationDegrees == 270
                    val width = (format.width * format.pixelWidthHeightRatio).toInt()
                    output.recordTimestamp(release, pts, generation,
                        if (rotated) format.height else width, if (rotated) width else format.height)
                }
                current.setVideoSurface(output.surface)
                current.setMediaItem(mediaItem, extractionPosition.coerceAtLeast(0))
                current.prepare()
            }
        }
    }

    private fun validateExtraction(): Boolean {
        if (extractionPosition < 0) return true
        val current = player ?: return false
        if (current.duration != C.TIME_UNSET && extractionPosition >= current.duration) {
            fail(10000, "Frame position is outside the video duration")
        } else if (current.playbackState == Player.STATE_READY || current.playbackState == Player.STATE_ENDED) {
            if (!current.currentTracks.isTypeSelected(C.TRACK_TYPE_VIDEO)) {
                fail(10001, "The source does not contain a selected video track")
            } else if (extractionPosition > 0 && !current.isCurrentMediaItemSeekable) {
                fail(10001, "The source does not support seeking")
            }
        }
        return !failed
    }

    fun command(operation: Int, value: Double, revision: Long) {
        if (closed.get()) return
        handler.post {
            guarded {
                val current = player ?: return@guarded
                if (operation == 2 || operation == 3 || operation == 6) {
                    generation = revision
                    frames?.discardPending()
                    clearSubtitles()
                }
                when (operation) {
                    0 -> {
                        if (current.playbackState == Player.STATE_ENDED) current.seekTo(0)
                        current.play()
                    }
                    1 -> current.pause()
                    2, 3 -> {
                        current.setSeekParameters(if (operation == 2) SeekParameters.EXACT else SeekParameters.CLOSEST_SYNC)
                        current.seekTo(value.toLong())
                    }
                    4 -> current.volume = value.toFloat()
                    5 -> current.setPlaybackSpeed(value.toFloat())
                    6 -> {
                        failed = false
                        tracks.reset(generation)
                        subtitleSelectionPending = false
                        current.trackSelectionParameters = current.trackSelectionParameters.buildUpon()
                            .clearOverridesOfType(C.TRACK_TYPE_AUDIO)
                            .clearOverridesOfType(C.TRACK_TYPE_TEXT).build()
                        current.setMediaItem(mediaItem)
                        current.playWhenReady = value != 0.0
                        current.prepare()
                    }
                    7 -> current.setAudioAttributes(AudioAttributes.DEFAULT, value != 0.0)
                }
                tracks.report(current.currentTracks, generation)
                handler.removeCallbacks(tick)
                tick.run()
            }
        }
    }

    private fun clearSubtitles() {
        lastSubtitle = ""
        nativeSubtitles(id, generation, null, 0, "[]")
    }

    private fun reportSubtitles(group: CueGroup) {
        if (extractionPosition >= 0 || subtitleSelectionPending) return
        val stream = tracks.selectedSubtitle
        val texts = org.json.JSONArray()
        if (stream != null) group.cues.forEach { cue -> cue.text?.let { texts.put(it.toString()) } }
        val encoded = texts.toString()
        val signature = "$generation:$stream:${group.presentationTimeUs}:$encoded"
        if (signature == lastSubtitle) return
        lastSubtitle = signature
        nativeSubtitles(id, generation, stream, group.presentationTimeUs.coerceAtLeast(0), encoded)
    }

    private fun reportState() {
        val current = player ?: return
        systemControls?.update(current)
        nativeState(id, generation, current.currentPosition,
            current.duration.takeUnless { it == C.TIME_UNSET } ?: -1L,
            current.isCurrentMediaItemSeekable, current.playbackState,
            frames?.width ?: 0, frames?.height ?: 0,
            current.currentTracks.isTypeSelected(C.TRACK_TYPE_AUDIO),
            current.playWhenReady, current.playbackSuppressionReason != Player.PLAYBACK_SUPPRESSION_REASON_NONE)
    }

    fun setSystemControls(title: String?, artist: String?, album: String?) {
        if (closed.get()) return
        handler.post {
            guarded {
                if (title == null) {
                    systemControls?.close()
                    systemControls = null
                } else if (extractionPosition < 0) {
                    val controls = systemControls ?: SystemMediaControls(application, handler, id) { operation, position ->
                        if (!closed.get() && !failed) nativeSystemCommand(id, operation, position)
                    }.also { systemControls = it }
                    controls.setMetadata(title, artist, album)
                    player?.let(controls::update)
                }
            }
        }
    }

    private fun guarded(block: () -> Unit) {
        if (closed.get()) return
        try { block() } catch (error: Exception) {
            fail(0, "Android media operation failed (${error.javaClass.simpleName})")
        }
    }

    private fun fail(code: Int, message: String) {
        if (closed.get() || failed) return
        failed = true
        clearSubtitles()
        handler.removeCallbacks(tick)
        runCatching { player?.pause() }
        runCatching { systemControls?.setError(message) }
        nativeError(id, generation, code, message)
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        handler.post {
            handler.removeCallbacksAndMessages(null)
            runCatching { systemControls?.close() }
                .onFailure { android.util.Log.w("GPUI", "System media controls release failed", it) }
            systemControls = null
            runCatching { player?.release() }
                .onFailure { android.util.Log.w("GPUI", "Media player release failed", it) }
            player = null
            runCatching { frames?.close() }
                .onFailure { android.util.Log.w("GPUI", "Media output release failed", it) }
            frames = null
            // Renderer release posts final analytics events to the application Looper.
            handler.looper.queue.addIdleHandler {
                thread.quitSafely()
                false
            }
        }
    }

    private external fun nativeState(id: Long, generation: Long, position: Long, duration: Long,
        seekable: Boolean, state: Int, width: Int, height: Int, audio: Boolean,
        playWhenReady: Boolean, suppressed: Boolean)
    private external fun nativeFrame(id: Long, generation: Long, pixels: ByteBuffer, width: Int, height: Int, timestamp: Long)
    private external fun nativeError(id: Long, generation: Long, code: Int, message: String)
    private external fun nativeSystemCommand(id: Long, operation: Int, position: Long)
    private external fun nativeSubtitles(id: Long, generation: Long, stream: String?, startUs: Long, texts: String)
    private external fun nativeTracks(id: Long, generation: Long, tracks: String)
}
