package dev.gpui.android

import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
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
                    if (!closed.get() && !failed && revision == generation) {
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
                val current = ExoPlayer.Builder(application)
                    .setLooper(thread.looper)
                    .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(application, http)))
                    .build()
                player = current
                current.addListener(object : Player.Listener {
                    override fun onEvents(player: Player, events: Player.Events) {
                        if (closed.get() || failed) return
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
                current.setVideoFrameMetadataListener { pts, release, _, _ ->
                    output.recordTimestamp(release, pts, generation)
                }
                current.setVideoSurface(output.surface)
                current.setMediaItem(MediaItem.fromUri(uri))
                current.prepare()
            }
        }
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
            current.currentTracks.isTypeSelected(C.TRACK_TYPE_AUDIO))
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
        seekable: Boolean, state: Int, width: Int, height: Int, audio: Boolean)
    private external fun nativeFrame(id: Long, generation: Long, pixels: ByteBuffer, width: Int, height: Int, timestamp: Long)
    private external fun nativeError(id: Long, generation: Long, code: Int, message: String)
}
