package dev.gpui.android

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

internal class MediaPlaybackRequest(
    val owner: MediaNotification,
    val token: String,
    var notification: Notification,
    val event: (String, String, String) -> Unit,
) {
    var service: MediaPlaybackService? = null
    var cancelled = false
    fun cancel() {
        cancelled = true
        // A pending start still needs foreground promotion before it can be stopped safely.
        service?.finish("stopped", "Background playback released")
    }
}

/** Foreground execution only. The GPUI application owns playback and its lifecycle. */
class MediaPlaybackService : Service() {
    private var current: MediaPlaybackRequest? = null
    private var ending = false
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val next = request?.takeIf { it.token == intent?.getStringExtra("token") }
        if (next == null) { stopSelf(startId); return START_NOT_STICKY }
        current = next
        next.service = this
        try {
            refresh(next.notification)
            next.owner.hideStandaloneNotification()
            if (next.cancelled) finish("stopped", "Background playback start cancelled")
            else next.event(next.token, "ready", "")
        } catch (error: RuntimeException) {
            finish("error", error.message ?: error.javaClass.simpleName)
        }
        return START_NOT_STICKY
    }

    private fun refresh(notification: Notification) {
        if (Build.VERSION.SDK_INT >= 29) startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        else startForeground(NOTIFICATION_ID, notification)
    }

    internal fun finish(reason: String, message: String) {
        if (ending) return
        ending = true
        val previous = current
        previous?.service = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        previous?.event?.invoke(previous.token, reason, message)
        previous?.owner?.restoreStandaloneNotification()
    }

    override fun onDestroy() {
        finish("stopped", "Android media playback service destroyed")
        if (request === current) request = null
        current = null
        super.onDestroy()
    }

    companion object {
        private const val NOTIFICATION_ID = 0x4750554d
        private var request: MediaPlaybackRequest? = null

        internal fun start(context: Context, owner: MediaNotification, token: String, event: (String, String, String) -> Unit) {
            check(request == null) { "A background playback lease is already active or stopping" }
            val next = MediaPlaybackRequest(owner, token, owner.backgroundNotification(), event)
            request = next
            try {
                context.startForegroundService(Intent(context, MediaPlaybackService::class.java).putExtra("token", token))
            } catch (error: RuntimeException) {
                request = null
                throw error
            }
        }

        internal fun stop(token: String) { request?.takeIf { it.token == token }?.cancel() }
        internal fun close(owner: MediaNotification) { request?.takeIf { it.owner === owner }?.cancel() }

        internal fun publish(owner: MediaNotification, notification: Notification): Boolean {
            val pending = request?.takeIf { it.owner === owner && !it.cancelled } ?: return false
            pending.notification = notification
            val service = pending.service ?: return false
            service.refresh(notification)
            return true
        }
    }
}
