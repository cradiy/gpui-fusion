package dev.gpui.android

import android.content.Context
import android.database.ContentObserver
import android.os.Handler
import android.provider.Settings

/** Observes the system's animator preference without polling during rendering. */
internal class MotionPreferences(context: Context, handler: Handler,
                                 private val changed: (Boolean) -> Unit) : AutoCloseable {
    private val resolver = context.applicationContext.contentResolver
    private var closed = false
    var reduced = read()
        private set
    private val observer = object : ContentObserver(handler) {
        override fun onChange(selfChange: Boolean) {
            if (closed) return
            val next = read()
            if (reduced != next) {
                reduced = next
                changed(next)
            }
        }
    }

    private fun read(): Boolean =
        Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f

    init {
        resolver.registerContentObserver(
            Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, observer
        )
        try {
            reduced = read()
        } catch (error: RuntimeException) {
            close()
            throw error
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        resolver.unregisterContentObserver(observer)
    }
}
