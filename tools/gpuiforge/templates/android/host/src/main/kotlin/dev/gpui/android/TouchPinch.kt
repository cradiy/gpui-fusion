package dev.gpui.android

import android.content.Context
import android.os.SystemClock
import android.view.MotionEvent
import android.view.ScaleGestureDetector

/** Native multi-contact scaling, subordinate to raw touch ownership. */
internal class TouchPinch(context: Context, private val send: (Int, Float, Float, Float) -> Unit) {
    private var blocked = false
    private var active = false
    private var x = 0f
    private var y = 0f
    private val detector = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            if (blocked) return false
            x = detector.focusX
            y = detector.focusY
            active = true
            send(0, x, y, 0f)
            return true
        }

        override fun onScale(detector: ScaleGestureDetector): Boolean {
            if (!active || blocked) return false
            val factor = detector.scaleFactor
            if (!factor.isFinite() || factor <= 0f) return false
            x = detector.focusX
            y = detector.focusY
            send(1, x, y, factor - 1f)
            return true
        }

        override fun onScaleEnd(detector: ScaleGestureDetector) { finish(2) }
    }).apply {
        isQuickScaleEnabled = false
        isStylusScaleEnabled = false
    }

    fun begin() {
        cancel()
        blocked = false
    }

    fun event(event: MotionEvent) {
        if (event.actionMasked == MotionEvent.ACTION_CANCEL) cancel()
        else if (!blocked) detector.onTouchEvent(event)
    }

    fun cancel() {
        blocked = true
        finish(3)
        val now = SystemClock.uptimeMillis()
        val event = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0f, 0f, 0)
        try { detector.onTouchEvent(event) } finally { event.recycle() }
    }

    private fun finish(phase: Int) {
        if (!active) return
        active = false
        send(phase, x, y, 0f)
    }
}
