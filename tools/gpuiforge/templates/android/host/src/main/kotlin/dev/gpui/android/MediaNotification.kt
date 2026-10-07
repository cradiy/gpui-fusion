package dev.gpui.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.net.Uri
import android.os.Handler
import android.os.Looper
import org.json.JSONObject

/** Transport surface; the Rust application owns and executes playback commands. */
internal class MediaNotification(private val context: Context, private val id: String, private val deliver: (String) -> Unit) : AutoCloseable {
    private val session = MediaSession(context, "gpui-media-$id")
    private val manager = context.getSystemService(NotificationManager::class.java)
    private var closed = false
    private var seekable = false
    private var canNext = false
    private var canPrevious = false
    private var lastNotification = ""
    init {
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "Media playback", NotificationManager.IMPORTANCE_LOW))
        session.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = command("play")
            override fun onPause() = command("pause")
            override fun onStop() = command("stop")
            override fun onSkipToNext() { if (canNext) command("next") }
            override fun onSkipToPrevious() { if (canPrevious) command("previous") }
            override fun onSeekTo(position: Long) { if (seekable && position >= 0) command("seek_to", JSONObject().put("secs", position / 1000).put("nanos", (position % 1000) * 1000000)) }
        }, Handler(Looper.getMainLooper()))
        sessions[id] = this
        session.isActive = true
    }
    private fun command(command: String, value: Any? = null) {
        if (!closed) deliver(JSONObject().put("id", id).put("event", JSONObject().put("command", command).apply { if (value != null) put("value", value) }).toString())
    }
    private fun millis(duration: JSONObject): Long = duration.getLong("secs") * 1000 + duration.getLong("nanos") / 1000000
    fun update(state: JSONObject) {
        check(!closed)
        val metadata = state.getJSONObject("metadata")
        val playing = state.getString("playback") == "playing"
        val nativeState = when (state.getString("playback")) {
            "playing" -> PlaybackState.STATE_PLAYING
            "paused" -> PlaybackState.STATE_PAUSED
            "buffering" -> PlaybackState.STATE_BUFFERING
            else -> PlaybackState.STATE_STOPPED
        }
        seekable = state.getBoolean("seekable"); canNext = state.getBoolean("can_next"); canPrevious = state.getBoolean("can_previous")
        var actions = PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_STOP
        if (seekable) actions = actions or PlaybackState.ACTION_SEEK_TO
        if (canNext) actions = actions or PlaybackState.ACTION_SKIP_TO_NEXT
        if (canPrevious) actions = actions or PlaybackState.ACTION_SKIP_TO_PREVIOUS
        session.setPlaybackState(PlaybackState.Builder().setActions(actions).setState(nativeState, millis(state.getJSONObject("position")), if (playing) state.getDouble("rate").toFloat() else 0f).build())
        val meta = MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE, metadata.getString("title"))
        if (!metadata.isNull("artist")) meta.putString(MediaMetadata.METADATA_KEY_ARTIST, metadata.getString("artist"))
        if (!metadata.isNull("album")) meta.putString(MediaMetadata.METADATA_KEY_ALBUM, metadata.getString("album"))
        if (!state.isNull("duration")) meta.putLong(MediaMetadata.METADATA_KEY_DURATION, millis(state.getJSONObject("duration")))
        val signature = "${metadata}:${state.optJSONObject("duration")}:$nativeState:$actions:${state.optJSONObject("icon")}"
        if (signature == lastNotification) return
        session.setMetadata(meta.build())
        val icon = state.optJSONObject("icon")
        val customIcon = if (icon?.optString("kind") == "resource") context.resources.getIdentifier(icon.getString("value"), "drawable", context.packageName) else 0
        val configuredIcon = context.resources.getIdentifier("gpui_notification_icon", "drawable", context.packageName)
        val iconId = if (customIcon != 0) customIcon else if (configuredIcon != 0) configuredIcon else context.applicationInfo.icon.takeIf { it != 0 } ?: android.R.drawable.sym_def_app_icon
        val builder = Notification.Builder(context, CHANNEL).setSmallIcon(iconId)
            .setContentTitle(metadata.getString("title"))
            .setContentText(if (metadata.isNull("artist")) null else metadata.getString("artist"))
            .setOnlyAlertOnce(true).setShowWhen(false).setOngoing(playing)
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
            val launch = PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            session.setSessionActivity(launch)
            builder.setContentIntent(launch)
        }
        fun button(command: String, label: String, icon: Int) {
            val intent = Intent(context, MediaNotificationReceiver::class.java).setData(Uri.parse("gpui-media://control/$id/$command")).putExtra("id", id).putExtra("command", command)
            val pending = PendingIntent.getBroadcast(context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            builder.addAction(Notification.Action.Builder(android.graphics.drawable.Icon.createWithResource(context, icon), label, pending).build())
        }
        var count = 0
        if (canPrevious) { button("previous", "Previous", android.R.drawable.ic_media_previous); count++ }
        button(if (playing) "pause" else "play", if (playing) "Pause" else "Play", if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play); count++
        if (canNext) { button("next", "Next", android.R.drawable.ic_media_next); count++ }
        button("stop", "Stop", android.R.drawable.ic_menu_close_clear_cancel)
        builder.setStyle(Notification.MediaStyle().setMediaSession(session.sessionToken).setShowActionsInCompactView(*IntArray(count) { it }))
        manager.notify("gpui.media:$id", 1, builder.build())
        lastNotification = signature
    }
    override fun close() {
        if (closed) return
        closed = true
        sessions.remove(id)
        manager.cancel("gpui.media:$id", 1)
        session.isActive = false
        session.setCallback(null)
        session.release()
    }
    companion object {
        private const val CHANNEL = "gpui.media"
        private val sessions = mutableMapOf<String, MediaNotification>()
        fun receive(context: Context, intent: Intent) {
            val id = intent.getStringExtra("id") ?: return
            val session = sessions[id]
            if (session == null) { context.getSystemService(NotificationManager::class.java).cancel("gpui.media:$id", 1); return }
            when (val command = intent.getStringExtra("command")) {
                "play", "pause", "stop" -> session.command(command)
                "next" -> if (session.canNext) session.command(command)
                "previous" -> if (session.canPrevious) session.command(command)
            }
        }
    }
}
class MediaNotificationReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) = MediaNotification.receive(context, intent)
}
