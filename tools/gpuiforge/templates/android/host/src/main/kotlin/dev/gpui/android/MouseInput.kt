package dev.gpui.android

import android.content.Context
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.ViewConfiguration
import kotlin.math.hypot

internal class MouseInput(context: Context, private val session: GpuiSession) {
    private val config = ViewConfiguration.get(context)
    private var buttons = 0
    private var device = -1
    private var x = 0f
    private var y = 0f
    private var modifiers = 0
    private var hovered = false
    private var lastButton = 0
    private var lastUp = Long.MIN_VALUE
    private var clickX = 0f
    private var clickY = 0f
    private var clicks = 0
    private val counts = mutableMapOf<Int, Int>()
    private val supported = intArrayOf(1, 2, 4, 8, 16)

    fun accepts(event: MotionEvent) = event.isFromSource(InputDevice.SOURCE_MOUSE)

    fun event(event: MotionEvent): Boolean {
        if (!accepts(event)) return false
        val action = event.actionMasked
        if (action !in intArrayOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP,
                MotionEvent.ACTION_MOVE, MotionEvent.ACTION_CANCEL,
                MotionEvent.ACTION_BUTTON_PRESS, MotionEvent.ACTION_BUTTON_RELEASE,
                MotionEvent.ACTION_HOVER_ENTER, MotionEvent.ACTION_HOVER_MOVE,
                MotionEvent.ACTION_HOVER_EXIT, MotionEvent.ACTION_SCROLL)) return false
        if (device != event.deviceId) { cancel(); device = event.deviceId }
        x = event.x
        y = event.y
        val meta = KeyEvent.normalizeMetaState(event.metaState)
        modifiers = (if (meta and KeyEvent.META_SHIFT_ON != 0) 1 else 0) or
            (if (meta and KeyEvent.META_CTRL_ON != 0) 2 else 0) or
            (if (meta and KeyEvent.META_ALT_ON != 0) 4 else 0) or
            (if (meta and KeyEvent.META_META_ON != 0) 8 else 0)
        if (action == MotionEvent.ACTION_CANCEL) { cancel(); return true }
        if (action == MotionEvent.ACTION_HOVER_EXIT) {
            send(3)
            hovered = false
            return true
        }
        hovered = true
        if (action == MotionEvent.ACTION_SCROLL) {
            session.mouse(4, x, y, 0, pressed(), 1, modifiers,
                event.getAxisValue(MotionEvent.AXIS_HSCROLL) * config.scaledHorizontalScrollFactor,
                event.getAxisValue(MotionEvent.AXIS_VSCROLL) * config.scaledVerticalScrollFactor)
            return true
        }
        if (action == MotionEvent.ACTION_HOVER_ENTER || action == MotionEvent.ACTION_HOVER_MOVE) {
            send(0)
            return true
        }
        if (buttons != 0 && hypot(x - clickX, y - clickY) > config.scaledTouchSlop) {
            lastButton = 0
        }
        // DOWN/UP and BUTTON_PRESS/RELEASE can describe the same transition.
        val next = event.buttonState and 31
        for (button in supported) {
            if (buttons and button != 0 && next and button == 0) {
                buttons = buttons and button.inv()
                send(2, button, counts.remove(button) ?: 1)
                if (lastButton == button) lastUp = event.eventTime
            }
        }
        for (button in supported) {
            if (buttons and button == 0 && next and button != 0) {
                val repeated = lastButton == button && lastUp != Long.MIN_VALUE &&
                    event.eventTime - lastUp <= ViewConfiguration.getDoubleTapTimeout() &&
                    hypot(x - clickX, y - clickY) <= config.scaledDoubleTapSlop
                clicks = if (repeated) clicks % 3 + 1 else 1
                lastButton = button
                lastUp = Long.MIN_VALUE
                clickX = x
                clickY = y
                buttons = buttons or button
                counts[button] = clicks
                send(1, button, clicks)
            }
        }
        if (action == MotionEvent.ACTION_MOVE) send(0)
        return true
    }

    fun cancel() {
        for (button in supported) {
            if (buttons and button != 0) {
                buttons = buttons and button.inv()
                send(5, button, counts.remove(button) ?: 1)
            }
        }
        if (hovered) send(3)
        hovered = false
        lastButton = 0
        lastUp = Long.MIN_VALUE
        device = -1
        modifiers = 0
    }

    private fun pressed() = supported.firstOrNull { buttons and it != 0 } ?: 0

    private fun send(kind: Int, button: Int = 0, count: Int = 1) {
        session.mouse(kind, x, y, button, pressed(), count, modifiers)
    }
}
