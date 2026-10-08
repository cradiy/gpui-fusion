package dev.gpui.android

import android.graphics.Matrix
import android.graphics.Rect
import android.graphics.RectF
import android.os.Build
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorBoundsInfo
import android.view.inputmethod.InputConnection

internal class CursorAnchorUpdates(
    private val view: GpuiView,
    private val epoch: Long,
    private val current: () -> TextInputState?,
) {
    private var closed = false
    private var monitoring = false
    private var immediate = false
    private var filters = 0
    private var last: CursorAnchorInfo? = null

    fun request(mode: Int): Boolean {
        val supported = InputConnection.CURSOR_UPDATE_IMMEDIATE or InputConnection.CURSOR_UPDATE_MONITOR or
            (if (Build.VERSION.SDK_INT >= 33) InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER or
                InputConnection.CURSOR_UPDATE_FILTER_EDITOR_BOUNDS else 0)
        if (closed || mode and supported.inv() != 0 || current() == null) return false
        monitoring = mode and InputConnection.CURSOR_UPDATE_MONITOR != 0
        immediate = mode and InputConnection.CURSOR_UPDATE_IMMEDIATE != 0
        filters = if (Build.VERSION.SDK_INT >= 33) mode and
            (InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER or InputConnection.CURSOR_UPDATE_FILTER_EDITOR_BOUNDS) else 0
        if (immediate) view.post { update(current()) }
        return true
    }

    fun close() { closed = true; monitoring = false; immediate = false; last = null }

    fun update(state: TextInputState?) {
        if (closed || (!monitoring && !immediate) || state == null || state.epoch != epoch || !view.isAttachedToWindow) return
        val density = view.resources.displayMetrics.density
        val matrix = Matrix()
        if (Build.VERSION.SDK_INT >= 29) view.transformMatrixToGlobal(matrix)
        else {
            val location = IntArray(2)
            view.getLocationOnScreen(location)
            matrix.setTranslate(location[0].toFloat(), location[1].toFloat())
        }
        matrix.preScale(density, density)
        val builder = CursorAnchorInfo.Builder().setMatrix(matrix).setSelectionRange(state.anchor, state.head)
        if (!state.sensitive && state.text != null && state.composingStart >= state.offset) {
            val start = state.composingStart - state.offset
            val end = state.composingEnd - state.offset
            if (start in 0..state.text.length && end in start..state.text.length) {
                builder.setComposingText(state.composingStart, state.text.substring(start, end))
            }
        }
        if (filters == 0 || filters and InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER != 0) {
            state.caretBounds?.let { caret ->
                val visible = Rect()
                val hasVisible = view.getLocalVisibleRect(visible)
                visible.bottom = minOf(visible.bottom, view.viewportHeight())
                val x = caret[0] * density
                val top = caret[1] * density
                val bottom = caret[3] * density
                val inside = hasVisible && x >= visible.left && x <= visible.right &&
                    bottom > visible.top && top < visible.bottom
                val outside = !inside || top < visible.top || bottom > visible.bottom
                val flags = (if (inside) CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION else 0) or
                    (if (outside) CursorAnchorInfo.FLAG_HAS_INVISIBLE_REGION else 0)
                // GPUI exposes the caret rectangle, but not its text baseline.
                builder.setInsertionMarkerLocation(caret[0], caret[1], Float.NaN, caret[3], flags)
            }
        }
        if (Build.VERSION.SDK_INT >= 33 && (filters == 0 || filters and InputConnection.CURSOR_UPDATE_FILTER_EDITOR_BOUNDS != 0)) {
            state.editorBounds?.let { bounds ->
                builder.setEditorBoundsInfo(EditorBoundsInfo.Builder()
                    .setEditorBounds(RectF(bounds[0], bounds[1], bounds[2], bounds[3])).build())
            }
        }
        val info = builder.build()
        if (immediate || info != last) {
            view.inputManager().updateCursorAnchorInfo(view, info)
            last = info
        }
        immediate = false
    }
}
