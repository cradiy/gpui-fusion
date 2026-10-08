package dev.gpui.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import org.json.JSONObject

/** Session-owned allowance for application-managed transfers; never restarts work. */
internal class DataSyncHost(
    private val context: Context,
    private val event: (String, String, String) -> Unit,
) : AutoCloseable {
    fun operation(operation: String, payload: String, active: Boolean) {
        GpuiSession.checkThread()
        when (operation) {
            "start" -> {
                check(active) { "Start data-sync execution from an active Activity after a user action" }
                check(DataSyncService.request == null) { "A data-sync lease is already active or stopping" }
                val data = JSONObject(payload)
                val notification = data.getJSONObject("notification")
                val channel = notification.getString("channel")
                val channelName = notification.getString("channel_name")
                require(channel.isNotBlank() && channelName.isNotBlank())
                context.getSystemService(NotificationManager::class.java).createNotificationChannel(
                    NotificationChannel(channel, channelName, NotificationManager.IMPORTANCE_LOW)
                )
                val request = DataSyncRequest(this, data.getString("token"), channel, channelName, build(notification), event)
                DataSyncService.request = request
                try {
                    context.startForegroundService(Intent(context, DataSyncService::class.java).putExtra("token", request.token))
                } catch (error: RuntimeException) {
                    DataSyncService.request = null
                    throw error
                }
            }
            "update" -> {
                val data = JSONObject(payload)
                val request = checkNotNull(DataSyncService.request) { "Data-sync lease has stopped" }
                check(request.owner === this && request.token == data.getString("token") && !request.cancelled)
                val notification = data.getJSONObject("notification")
                require(notification.getString("channel") == request.channel && notification.getString("channel_name") == request.channelName) {
                    "An active data-sync lease cannot change notification channels"
                }
                request.notification = build(notification)
                request.service?.refresh(request.notification)
            }
            "stop" -> DataSyncService.request?.takeIf { it.owner === this && it.token == payload }?.cancel()
            else -> error("Unknown data-sync operation")
        }
    }

    private fun build(item: JSONObject): Notification {
        require(item.getString("title").isNotBlank())
        val custom = item.optString("icon", "").takeIf { it.isNotBlank() }?.let {
            context.resources.getIdentifier(it, "drawable", context.packageName)
        } ?: 0
        val configured = context.resources.getIdentifier("gpui_notification_icon", "drawable", context.packageName)
        val icon = custom.takeIf { it != 0 } ?: configured.takeIf { it != 0 }
            ?: context.applicationInfo.icon.takeIf { it != 0 } ?: android.R.drawable.sym_def_app_icon
        val builder = NotificationCompat.Builder(context, item.getString("channel"))
            .setSmallIcon(icon).setContentTitle(item.getString("title")).setContentText(item.getString("body"))
            .setOngoing(true).setOnlyAlertOnce(true).setSilent(true).setCategory(NotificationCompat.CATEGORY_PROGRESS)
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
            builder.setContentIntent(PendingIntent.getActivity(context, 0,
                it.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
        }
        return builder.build()
    }

    override fun close() { DataSyncService.request?.takeIf { it.owner === this }?.cancel() }
}

internal class DataSyncRequest(
    val owner: DataSyncHost,
    val token: String,
    val channel: String,
    val channelName: String,
    var notification: Notification,
    val event: (String, String, String) -> Unit,
) {
    var service: DataSyncService? = null
    var cancelled = false
    fun cancel() {
        cancelled = true
        // Pending startForegroundService calls must still reach startForeground before stopping.
        service?.finish("stopped", "Data-sync lease released")
    }
}

class DataSyncService : Service() {
    private var current: DataSyncRequest? = null
    private var ending = false
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val next = request?.takeIf { it.token == intent?.getStringExtra("token") }
        if (next == null) { stopSelf(startId); return START_NOT_STICKY }
        current = next
        next.service = this
        try {
            refresh(next.notification)
            if (next.cancelled) finish("stopped", "Data-sync start cancelled")
            else next.event(next.token, "ready", "")
        } catch (error: RuntimeException) {
            finish("error", error.message ?: error.javaClass.simpleName)
        }
        return START_NOT_STICKY
    }

    internal fun refresh(notification: Notification) {
        if (Build.VERSION.SDK_INT >= 29) startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
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
    }

    override fun onTimeout(startId: Int, fgsType: Int) { finish("timeout", "Android data-sync time allowance exhausted") }
    override fun onDestroy() {
        finish("stopped", "Android data-sync service destroyed")
        if (request === current) request = null
        current = null
        super.onDestroy()
    }

    companion object {
        // Reserved for this service; general GPUI notifications use tagged identifiers.
        private const val NOTIFICATION_ID = 0x47505549
        internal var request: DataSyncRequest? = null
    }
}
