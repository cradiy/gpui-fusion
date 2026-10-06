package dev.gpui.android

import android.content.Context
import android.view.MotionEvent
import android.view.VelocityTracker
import android.view.ViewConfiguration
import android.widget.OverScroller
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.roundToInt

/** Single-contact scrolling synthesized only while raw touch handlers allow it. */
internal class TouchScroll(context: Context, private val session: GpuiSession) {
    private val scroller = OverScroller(context)
    private val config = ViewConfiguration.get(context)
    private val touchSlop = config.scaledTouchSlop
    private val minimumVelocity = config.scaledMinimumFlingVelocity
    private val maximumVelocity = config.scaledMaximumFlingVelocity
    private var velocity: VelocityTracker? = null
    private var blocked = false
    private var dragging = false
    private var scrolling = false
    private var anchorX = 0f
    private var anchorY = 0f
    private var lastX = 0f
    private var lastY = 0f
    private var flingX = 0
    private var flingY = 0

    fun begin(event: MotionEvent): Boolean {
        val interrupted = scrolling
        cancel()
        blocked = false
        anchorX = event.x
        lastX = anchorX
        anchorY = event.y
        lastY = anchorY
        velocity = VelocityTracker.obtain()
        return interrupted
    }

    fun block() {
        blocked = true
        finish(3)
        recycleVelocity()
    }

    // Returns true once a drag has crossed the slop, including its release.
    fun event(event: MotionEvent): Boolean {
        val tracker = velocity ?: return false
        if (blocked) return false
        tracker.addMovement(event)
        when (event.actionMasked) {
            MotionEvent.ACTION_MOVE -> {
                val x = event.x
                val y = event.y
                if (!dragging && hypot(x - anchorX, y - anchorY) > touchSlop) {
                    dragging = true
                    scrolling = true
                    session.scroll(0, anchorX, anchorY, 0f, 0f)
                }
                if (dragging) session.scroll(1, anchorX, anchorY, x - lastX, y - lastY)
                lastX = x
                lastY = y
            }
            MotionEvent.ACTION_UP -> {
                val wasDragging = dragging
                if (dragging) {
                    tracker.computeCurrentVelocity(1000, maximumVelocity.toFloat())
                    val vx = tracker.getXVelocity(event.getPointerId(0)).let { if (abs(it) < minimumVelocity) 0f else it }
                    val vy = tracker.getYVelocity(event.getPointerId(0)).let { if (abs(it) < minimumVelocity) 0f else it }
                    if (vx != 0f || vy != 0f) {
                        flingX = 0
                        flingY = 0
                        scroller.fling(0, 0, vx.roundToInt(), vy.roundToInt(),
                            Int.MIN_VALUE / 2, Int.MAX_VALUE / 2,
                            Int.MIN_VALUE / 2, Int.MAX_VALUE / 2)
                    } else {
                        finish(2)
                    }
                }
                dragging = false
                recycleVelocity()
                return wasDragging
            }
        }
        return dragging
    }

    fun frame() {
        if (!scrolling || dragging) return
        if (scroller.computeScrollOffset()) {
            val x = scroller.currX
            val y = scroller.currY
            if (x != flingX || y != flingY) {
                session.scroll(1, anchorX, anchorY, (x - flingX).toFloat(), (y - flingY).toFloat())
            }
            flingX = x
            flingY = y
        }
        if (scroller.isFinished) finish(2)
    }

    fun cancel() {
        blocked = true
        finish(3)
        recycleVelocity()
    }

    private fun finish(phase: Int) {
        scroller.forceFinished(true)
        dragging = false
        val wasScrolling = scrolling
        scrolling = false
        if (wasScrolling) session.scroll(phase, anchorX, anchorY, 0f, 0f)
    }

    private fun recycleVelocity() {
        velocity?.recycle()
        velocity = null
    }
}
