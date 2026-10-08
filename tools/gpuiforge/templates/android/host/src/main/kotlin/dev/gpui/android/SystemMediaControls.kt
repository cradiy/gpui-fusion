package dev.gpui.android

import android.content.Context
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Handler
import android.os.SystemClock
import androidx.media3.common.C
import androidx.media3.common.Player

/** Publishes one player's transport state without owning its playback lifetime. */
internal class SystemMediaControls(
    context: Context,
    handler: Handler,
    id: Long,
    private val command: (Int, Long) -> Unit,
) : AutoCloseable {
    private val session = MediaSession(context.applicationContext, "gpui-media-$id")
    private var closed = false
    private var metadata = MediaMetadata.Builder()
    private var duration = Long.MIN_VALUE
    private var lastState = -1
    private var lastActions = -1L
    private var lastSpeed = -1f
    private var lastPosition = 0L
    private var lastUpdate = 0L

    init {
        session.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = send(0)
            override fun onPause() = send(1)
            override fun onStop() = send(2)
            override fun onSeekTo(position: Long) {
                if (position >= 0 && lastActions and PlaybackState.ACTION_SEEK_TO != 0L) send(3, position)
            }
        }, handler)
        session.isActive = true
    }

    private fun send(operation: Int, position: Long = 0) {
        if (!closed) command(operation, position)
    }

    fun setMetadata(title: String, artist: String?, album: String?) {
        metadata = MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE, title)
        artist?.let { metadata.putString(MediaMetadata.METADATA_KEY_ARTIST, it) }
        album?.let { metadata.putString(MediaMetadata.METADATA_KEY_ALBUM, it) }
        duration = Long.MIN_VALUE
    }

    fun update(player: Player) {
        if (closed) return
        val nextDuration = player.duration.takeUnless { it == C.TIME_UNSET } ?: -1L
        if (duration != nextDuration) {
            duration = nextDuration
            metadata.putLong(MediaMetadata.METADATA_KEY_DURATION, duration)
            session.setMetadata(metadata.build())
        }
        val state = when {
            player.playerError != null -> PlaybackState.STATE_ERROR
            player.playbackState == Player.STATE_ENDED -> PlaybackState.STATE_STOPPED
            !player.playWhenReady || player.playbackSuppressionReason != Player.PLAYBACK_SUPPRESSION_REASON_NONE -> PlaybackState.STATE_PAUSED
            player.playbackState == Player.STATE_BUFFERING -> PlaybackState.STATE_BUFFERING
            player.isPlaying -> PlaybackState.STATE_PLAYING
            else -> PlaybackState.STATE_NONE
        }
        var actions = PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
            PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_STOP
        if (player.isCurrentMediaItemSeekable) actions = actions or PlaybackState.ACTION_SEEK_TO
        val speed = if (player.isPlaying) player.playbackParameters.speed else 0f
        val position = player.currentPosition
        val now = SystemClock.elapsedRealtime()
        val expected = lastPosition + ((now - lastUpdate) * lastSpeed).toLong()
        if (state == lastState && actions == lastActions && speed == lastSpeed &&
            kotlin.math.abs(position - expected) < 100 && now - lastUpdate < 1000) return
        session.setPlaybackState(PlaybackState.Builder().setActions(actions)
            .setState(state, position, speed, now).build())
        lastState = state
        lastActions = actions
        lastSpeed = speed
        lastPosition = position
        lastUpdate = now
    }

    fun setError(message: String) {
        session.setPlaybackState(PlaybackState.Builder().setActions(0)
            .setState(PlaybackState.STATE_ERROR, lastPosition, 0f)
            .setErrorMessage(message).build())
        lastState = PlaybackState.STATE_ERROR
    }

    override fun close() {
        if (closed) return
        closed = true
        session.isActive = false
        session.setCallback(null)
        session.release()
    }
}
