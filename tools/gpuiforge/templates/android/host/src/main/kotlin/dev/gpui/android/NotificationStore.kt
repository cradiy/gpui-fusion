package dev.gpui.android

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import androidx.core.app.RemoteInput
import androidx.core.app.NotificationCompat
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

/** Notification delivery only; applications consume actions and perform their own work. */
internal class NotificationStore(private val context: Context, private val wake: () -> Unit) : AutoCloseable {
    private val manager = context.getSystemService(NotificationManager::class.java)
    init { listeners.add(wake) }
    override fun close() { listeners.remove(wake) }

    fun operation(operation: String, payload: String): String {
        when (operation) {
            "create" -> {
                val options = JSONObject(payload)
                require(options.getString("app_id") == context.packageName) { "Notification app_id must match the Android package" }
                val channel = options.getString("channel")
                require(channel.isNotBlank() && options.getString("name").isNotBlank())
                manager.createNotificationChannel(NotificationChannel(channel, options.getString("name"), NotificationManager.IMPORTANCE_DEFAULT))
            }
            "permission" -> {
                if (Build.VERSION.SDK_INT >= 33 && context.checkSelfPermission("android.permission.POST_NOTIFICATIONS") != PackageManager.PERMISSION_GRANTED) return "runtime"
                return if (manager.areNotificationsEnabled() && manager.getNotificationChannel(payload)?.importance != NotificationManager.IMPORTANCE_NONE) "granted" else "denied"
            }
            "show" -> show(JSONObject(payload))
            "remove" -> {
                val item = JSONObject(payload)
                val tag = tag(item.getString("channel"), item.getString("id"))
                manager.cancel(tag, 1)
                preferences(context).edit().remove("token:$tag").apply()
            }
            "drain" -> {
                val (selected, rest) = splitPending(context) { it.getString("channel") == payload }
                preferences(context).edit().putString("pending", rest.toString()).apply()
                return JSONArray(selected.map { it.getJSONObject("event") }).toString()
            }
            else -> error("Unknown notification operation")
        }
        return ""
    }

    fun deliver(accept: (String) -> Boolean) {
        val (_, rest) = splitPending(context) { accept(it.toString()) }
        preferences(context).edit().putString("pending", rest.toString()).apply()
    }

    private fun show(payload: JSONObject) {
        check(manager.areNotificationsEnabled()) { "Notifications are disabled; request permission before posting" }
        val channel = payload.getString("channel")
        check(manager.getNotificationChannel(channel)?.importance != NotificationManager.IMPORTANCE_NONE) { "Notification channel is disabled" }
        val item = payload.getJSONObject("notification")
        val id = item.getString("id")
        val tag = tag(channel, id)
        val token = UUID.randomUUID().toString()
        val actions = item.getJSONArray("actions")
        val allowed = JSONObject().put("token", token).put("actions", actions)
        fun intent(action: String, broadcast: Boolean): Intent {
            val intent = if (broadcast) Intent(context, NotificationReceiver::class.java)
                else context.packageManager.getLaunchIntentForPackage(context.packageName) ?: error("Application has no launcher Activity")
            return intent.setAction(ACTION).setData(Uri.Builder().scheme("gpui-notification").authority(context.packageName).appendPath(channel).appendPath(id).appendPath(action).build())
                .putExtra("gpui_channel", channel).putExtra("gpui_id", id).putExtra("gpui_action", action).putExtra("gpui_token", token)
                .addFlags(if (broadcast) 0 else Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        fun pending(action: String, reply: Boolean, broadcast: Boolean): PendingIntent {
            val mutable = if (reply && Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else if (reply) 0 else PendingIntent.FLAG_IMMUTABLE
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or mutable
            return if (broadcast) PendingIntent.getBroadcast(context, 0, intent(action, true), flags)
                else PendingIntent.getActivity(context, 0, intent(action, false), flags)
        }
        val icon = item.optJSONObject("icon")
        val customIcon = if (icon?.optString("kind") == "resource") context.resources.getIdentifier(icon.getString("value"), "drawable", context.packageName) else 0
        val configuredIcon = context.resources.getIdentifier("gpui_notification_icon", "drawable", context.packageName)
        val iconId = if (customIcon != 0) customIcon else if (configuredIcon != 0) configuredIcon else context.applicationInfo.icon.takeIf { it != 0 } ?: android.R.drawable.sym_def_app_icon
        val builder = NotificationCompat.Builder(context, channel)
            .setSmallIcon(iconId)
            .setContentTitle(item.getString("title")).setContentText(item.getString("body"))
            .setStyle(NotificationCompat.BigTextStyle().bigText(item.getString("body")))
            .setOnlyAlertOnce(true).setAutoCancel(true)
            .setContentIntent(pending("default", false, false))
            .setDeleteIntent(pending("dismiss", false, true))
        if (item.getBoolean("silent")) builder.setSilent(true)
        if (!item.isNull("progress")) builder.setProgress(100, item.getInt("progress"), false)
        for (index in 0 until actions.length()) {
            val action = actions.getJSONObject(index)
            val reply = !action.isNull("reply_placeholder")
            val native = NotificationCompat.Action.Builder(0, action.getString("label"), pending(action.getString("id"), reply, true))
            if (reply) native.addRemoteInput(RemoteInput.Builder("reply").setLabel(action.getString("reply_placeholder")).build())
            builder.addAction(native.build())
        }
        val notification = builder.build()
        manager.notify(tag, 1, notification)
        preferences(context).edit().putString("token:$tag", allowed.toString()).apply()
    }

    companion object {
        const val ACTION = "dev.gpui.android.NOTIFICATION"
        private val listeners = mutableSetOf<() -> Unit>()
        private fun preferences(context: Context) = context.getSharedPreferences("gpui-notifications", Context.MODE_PRIVATE)
        private fun tag(channel: String, id: String) = "gpui:$channel:$id"
        private fun splitPending(context: Context, select: (JSONObject) -> Boolean): Pair<List<JSONObject>, JSONArray> {
            val pending = JSONArray(preferences(context).getString("pending", "[]"))
            val selected = mutableListOf<JSONObject>(); val rest = JSONArray()
            for (index in 0 until pending.length()) {
                val item = pending.getJSONObject(index)
                if (select(item)) selected.add(item) else rest.put(item)
            }
            return selected to rest
        }
        fun receive(context: Context, intent: Intent): Boolean {
            if (intent.action != ACTION) return false
            val channel = intent.getStringExtra("gpui_channel") ?: return false
            val id = intent.getStringExtra("gpui_id") ?: return false
            val action = intent.getStringExtra("gpui_action") ?: return false
            val tag = tag(channel, id)
            val prefs = preferences(context)
            val allowed = prefs.getString("token:$tag", null)?.let(::JSONObject) ?: return false
            if (intent.getStringExtra("gpui_token") != allowed.getString("token")) return false
            val actions = allowed.getJSONArray("actions")
            val specification = (0 until actions.length()).map { actions.getJSONObject(it) }.find { it.getString("id") == action }
            if (action != "default" && action != "dismiss" && specification == null) return false
            val reply = if (specification?.isNull("reply_placeholder") == false) RemoteInput.getResultsFromIntent(intent)?.getCharSequence("reply")?.toString() else null
            val event = JSONObject().put("kind", if (action == "dismiss") "dismissed" else "activated").put("id", id)
                .put("action", if (action == "default") JSONObject.NULL else action).put("reply", reply ?: JSONObject.NULL)
            val pending = JSONArray(prefs.getString("pending", "[]"))
            pending.put(JSONObject().put("channel", channel).put("event", event))
            if (!prefs.edit().putString("pending", pending.toString()).remove("token:$tag").commit()) return false
            context.getSystemService(NotificationManager::class.java).cancel(tag, 1)
            listeners.toList().forEach { it() }
            intent.removeExtra("gpui_token")
            return true
        }
    }
}

class NotificationReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) { NotificationStore.receive(context, intent) }
}
