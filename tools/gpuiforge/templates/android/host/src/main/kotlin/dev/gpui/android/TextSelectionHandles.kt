package dev.gpui.android

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Rect
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.Drawable
import android.os.Build
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.widget.Magnifier
import android.widget.PopupWindow

internal class TextSelectionHandles(
    private val view: GpuiView,
    private val current: () -> TextInputState?,
    private val dragging: (Boolean) -> Unit,
    private val changed: () -> Unit,
) {
    private val anchor = Handle(true)
    private val head = Handle(false)
    private var magnifier: Magnifier? = null
    private var moving: Handle? = null

    fun beforeFrame(time: Long) { moving?.scrollFrame(time) }
    fun needsFrame() = moving != null

    fun update(input: TextInputState) {
        if (!view.isAttachedToWindow || !view.hasWindowFocus()) { close(); return }
        if (input.anchor != input.head) anchor.show(input, input.anchorBounds)
        else anchor.hide()
        head.show(input, input.headBounds)
        moving?.let {
            it.afterFrame(input)
            dragging(true)
            it.refreshMagnifier(current() ?: input)
        }
    }

    fun close() {
        anchor.hide()
        head.hide()
        moving = null
        dismissMagnifier()
    }

    private fun dismissMagnifier() {
        if (Build.VERSION.SDK_INT >= 28) magnifier?.dismiss()
        magnifier = null
    }

    private inner class Handle(private val isAnchor: Boolean) : View(view.context) {
        private val density = resources.displayMetrics.density
        private val extent = (48 * density).toInt()
        private val popup = PopupWindow(this, extent, extent, false).apply {
            setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            isClippingEnabled = false
            inputMethodMode = PopupWindow.INPUT_METHOD_NOT_NEEDED
        }
        private var drawable: Drawable = ColorDrawable(Color.TRANSPARENT)
        private var attribute = 0
        private var hotspot = .5f
        private var epoch = 0L
        private var fixed = 0
        private var startX = 0f
        private var startY = 0f
        private var sourceX = 0f
        private var sourceY = 0f
        private var bounds: FloatArray? = null
        private var active = false
        private var insertion = false
        private var pointerX = 0f
        private var pointerY = 0f
        private var lastFrame = 0L
        private var selectionPending = false

        init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }

        fun show(input: TextInputState, caret: FloatArray?) {
            val editor = input.editorBounds
            val visible = Rect()
            if (caret == null || editor == null || !view.getLocalVisibleRect(visible)) { hide(); return }
            val x = caret[0] * density
            val y = caret[3] * density
            if (x < maxOf(visible.left.toFloat(), editor[0] * density) ||
                x > minOf(visible.right.toFloat(), editor[2] * density) ||
                y < maxOf(visible.top.toFloat(), editor[1] * density) ||
                y > minOf(visible.bottom.toFloat(), view.viewportHeight().toFloat(), editor[3] * density)) {
                if (!active) hide()
                return
            }
            bounds = caret
            val collapsed = input.anchor == input.head
            val left = if (isAnchor) input.anchor < input.head else input.head < input.anchor
            val next = if (collapsed) android.R.attr.textSelectHandle
                else if (left) android.R.attr.textSelectHandleLeft else android.R.attr.textSelectHandleRight
            if (attribute != next) {
                attribute = next
                val values = context.obtainStyledAttributes(intArrayOf(next))
                try { values.getDrawable(0)?.let { drawable = it.mutate() } }
                finally { values.recycle() }
                hotspot = if (collapsed) .5f else if (left) .75f else .25f
                invalidate()
            }
            val location = IntArray(2)
            view.getLocationInWindow(location)
            val drawableWidth = drawable.intrinsicWidth.coerceIn(1, extent)
            val leftPadding = (extent - drawableWidth) / 2f
            val px = (location[0] + x - leftPadding - drawableWidth * hotspot).toInt()
            val py = (location[1] + y).toInt()
            if (popup.isShowing) popup.update(px, py, -1, -1)
            else popup.showAtLocation(view, Gravity.TOP or Gravity.LEFT, px, py)
        }

        fun hide() { active = false; selectionPending = false; lastFrame = 0; popup.dismiss() }

        override fun onDraw(canvas: Canvas) {
            val width = drawable.intrinsicWidth.coerceIn(1, extent)
            val height = drawable.intrinsicHeight.coerceIn(1, extent)
            val left = (extent - width) / 2
            drawable.setBounds(left, 0, left + width, height)
            drawable.draw(canvas)
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            val input = current() ?: return false
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    val caret = bounds ?: return false
                    active = true
                    moving = this
                    epoch = input.epoch
                    fixed = if (isAnchor) input.head else input.anchor
                    insertion = input.anchor == input.head
                    startX = event.rawX
                    startY = event.rawY
                    sourceX = caret[0] * density
                    sourceY = (caret[1] + caret[3]) * density / 2
                    pointerX = sourceX
                    pointerY = sourceY
                    lastFrame = 0
                    view.requestFrame()
                    dragging(true)
                    showMagnifier(input, sourceX, sourceY)
                }
                MotionEvent.ACTION_MOVE -> {
                    if (!active || input.epoch != epoch || event.pointerCount != 1) return true
                    pointerX = sourceX + event.rawX - startX
                    pointerY = sourceY + event.rawY - startY
                    selectAtPointer(input)
                    dragging(true)
                    refreshMagnifier(input)
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_POINTER_DOWN -> {
                    active = false
                    selectionPending = false
                    lastFrame = 0
                    moving = null
                    dismissMagnifier()
                    dragging(false)
                    if (event.actionMasked == MotionEvent.ACTION_UP) performClick()
                }
            }
            return true
        }

        private fun selectAtPointer(input: TextInputState) {
            val editor = input.editorBounds ?: return
            val left = editor[0] * density
            val top = editor[1] * density
            val x = pointerX.coerceIn(left, maxOf(left, editor[2] * density - 1f))
            val y = pointerY.coerceIn(top, maxOf(top, minOf(view.viewportHeight().toFloat(), editor[3] * density) - 1f))
            val index = view.inputIndex(epoch, x, y)
            if (index >= 0 && (insertion || index != fixed)) {
                val anchor = if (insertion || isAnchor) index else fixed
                val head = if (insertion || !isAnchor) index else fixed
                if ((anchor != input.anchor || head != input.head) && view.selectText(epoch, anchor, head)) changed()
            }
        }

        fun scrollFrame(time: Long) {
            val input = current() ?: return
            if (!active || input.epoch != epoch) return
            val previous = lastFrame
            lastFrame = time
            if (previous == 0L) return
            val editor = input.editorBounds ?: return
            val elapsed = ((time - previous) / 1_000_000_000f).coerceIn(0f, .05f)
            fun speed(position: Float, start: Float, end: Float): Float {
                val edge = minOf(24f * density, (end - start) / 4).coerceAtLeast(0f)
                if (edge <= 0f) return 0f
                val amount = when {
                    position < start + edge -> (start + edge - position) / edge
                    position > end - edge -> (end - edge - position) / edge
                    else -> 0f
                }
                return amount.coerceIn(-1f, 1f) * 480f * density * elapsed
            }
            val dx = if (input.multiline) 0f else speed(pointerX, editor[0] * density, editor[2] * density)
            val dy = if (!input.multiline) 0f else speed(pointerY, editor[1] * density,
                minOf(editor[3] * density, view.viewportHeight().toFloat()))
            if ((dx != 0f || dy != 0f) && view.scrollInput(epoch, dx, dy)) selectionPending = true
        }

        fun afterFrame(input: TextInputState) {
            if (!selectionPending) return
            selectionPending = false
            if (active && input.epoch == epoch) selectAtPointer(input)
        }

        fun refreshMagnifier(input: TextInputState) {
            val caret = bounds ?: return
            showMagnifier(input, caret[0] * density, (caret[1] + caret[3]) * density / 2)
            if (Build.VERSION.SDK_INT >= 28) magnifier?.update()
        }

        private fun showMagnifier(input: TextInputState, x: Float, y: Float) {
            if (!input.sensitive && Build.VERSION.SDK_INT >= 28) {
                if (magnifier == null) magnifier = if (Build.VERSION.SDK_INT >= 29)
                    Magnifier.Builder(view).setCornerRadius(16f * density).build() else Magnifier(view)
                magnifier?.show(x, y)
            }
        }

        override fun performClick(): Boolean { super.performClick(); return true }
    }
}
