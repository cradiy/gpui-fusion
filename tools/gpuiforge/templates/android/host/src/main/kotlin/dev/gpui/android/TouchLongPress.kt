package dev.gpui.android

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import kotlin.math.hypot

/** One long press per eligible single-contact sequence. */
internal class TouchLongPress(private val view: View, private val recognize: (Float, Float) -> Unit) {
    private val slop = ViewConfiguration.get(view.context).scaledTouchSlop
    private var pending: Runnable? = null
    private var x = 0f
    private var y = 0f

    fun touch(event: MotionEvent, eligible: Boolean) {
        if (!eligible) { cancel(); return }
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                cancel()
                x = event.x
                y = event.y
                pending = Runnable {
                    pending = null
                    if (view.hasWindowFocus() && view.isShown) recognize(x, y)
                }.also {
                    view.postDelayed(it, (event.downTime + ViewConfiguration.getLongPressTimeout() - SystemClock.uptimeMillis()).coerceAtLeast(0))
                }
            }
            MotionEvent.ACTION_MOVE -> if (hypot(event.x - x, event.y - y) > slop) cancel()
            MotionEvent.ACTION_POINTER_DOWN, MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP,
            MotionEvent.ACTION_CANCEL -> cancel()
        }
    }

    fun cancel() { pending?.let(view::removeCallbacks); pending = null }
}
