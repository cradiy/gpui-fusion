package dev.gpui.android

import android.view.Window
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

internal class FullscreenHost(private val window: Window) {
    private var enabled = false
    private var previousVisible = 0
    private var previousBehavior = 0

    fun setEnabled(next: Boolean) {
        if (next == enabled) return
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        val bars = WindowInsetsCompat.Type.systemBars()
        if (next) {
            val insets = ViewCompat.getRootWindowInsets(window.decorView)
            previousVisible = 0
            for (type in intArrayOf(WindowInsetsCompat.Type.statusBars(),
                WindowInsetsCompat.Type.navigationBars(), WindowInsetsCompat.Type.captionBar())) {
                if (insets == null || insets.isVisible(type)) previousVisible = previousVisible or type
            }
            previousBehavior = controller.systemBarsBehavior
            controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            controller.hide(bars)
        } else {
            controller.systemBarsBehavior = previousBehavior
            controller.show(previousVisible)
            controller.hide(bars and previousVisible.inv())
        }
        enabled = next
        ViewCompat.requestApplyInsets(window.decorView)
    }
}
