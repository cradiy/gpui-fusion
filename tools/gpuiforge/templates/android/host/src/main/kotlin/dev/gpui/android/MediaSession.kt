package dev.gpui.android

import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import androidx.media3.common.C
import androidx.media3.common.AudioAttributes
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
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
    private val uri: String,
    keys: Array<String>,
    values: Array<String>,
    timeout: Int,
    private val extractionPosition: Long,
) : AutoCloseable {
    private val thread = HandlerThread("gpui-media").apply { start() }
    private val handler = Handler(thread.looper)
    private val closed = AtomicBoolean(false)
    private var player: ExoPlayer? = null
    private var frames: MediaFrames? = null
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

    init {
        val application = context.applicationContext
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
                current.setMediaItem(MediaItem.fromUri(uri), extractionPosition.coerceAtLeast(0))
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
                        current.setMediaItem(MediaItem.fromUri(uri))
                        current.playWhenReady = value != 0.0
                        current.prepare()
                    }
                    7 -> current.setAudioAttributes(AudioAttributes.DEFAULT, value != 0.0)
                }
                handler.removeCallbacks(tick)
                tick.run()
            }
        }
    }

    private fun reportState() {
        val current = player ?: return
        nativeState(id, generation, current.currentPosition,
            current.duration.takeUnless { it == C.TIME_UNSET } ?: -1L,
            current.isCurrentMediaItemSeekable, current.playbackState,
            frames?.width ?: 0, frames?.height ?: 0,
            current.currentTracks.isTypeSelected(C.TRACK_TYPE_AUDIO),
            current.playWhenReady, current.playbackSuppressionReason != Player.PLAYBACK_SUPPRESSION_REASON_NONE)
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
        handler.removeCallbacks(tick)
        runCatching { player?.pause() }
        nativeError(id, generation, code, message)
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        handler.post {
            handler.removeCallbacksAndMessages(null)
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
}
