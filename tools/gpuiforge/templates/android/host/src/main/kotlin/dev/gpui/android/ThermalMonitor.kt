package dev.gpui.android

import android.content.Context
import android.os.Handler
import android.os.PowerManager
import android.os.SystemClock
import androidx.annotation.RequiresApi
import java.util.concurrent.Executor

/** Retains one system listener across View and Surface replacement. */
@RequiresApi(29)
internal class ThermalMonitor(context: Context, private val handler: Handler,
                              private val changed: (Int) -> Unit) : AutoCloseable {
    private val manager = context.applicationContext.getSystemService(PowerManager::class.java)
    private var closed = false
    var status = manager.currentThermalStatus
        private set
    private val listener = PowerManager.OnThermalStatusChangedListener { next ->
        if (!closed && status != next) {
            status = next
            changed(next)
        }
    }

    init {
        // Always enqueue: native observers may call back into GPUI when notified.
        manager.addThermalStatusListener(Executor { task ->
            handler.postAtTime(task, this, SystemClock.uptimeMillis())
        }, listener)
    }

    override fun close() {
        if (closed) return
        closed = true
        try {
            manager.removeThermalStatusListener(listener)
        } finally {
            handler.removeCallbacksAndMessages(this)
        }
    }
}
