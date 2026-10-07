package dev.gpui.android

import android.content.ClipDescription
import android.content.ClipboardManager
import android.graphics.Rect
import android.icu.text.BreakIterator
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.hypot

internal class TextEditMenu(
    private val view: GpuiView,
    private val current: () -> TextInputState?,
) : ActionMode.Callback2() {
    private var mode: ActionMode? = null
    private var state: TextInputState? = null
    private var downX = 0f
    private var downY = 0f
    private val slop = ViewConfiguration.get(view.context).scaledTouchSlop
    private val handles = TextSelectionHandles(view, current,
        { dragging -> mode?.hide(if (dragging) 1000 else 0) },
        { update(current()) })

    fun tapped(input: TextInputState?) {
        if (input?.hit != true || input.anchor != input.head) return
        state = input
        handles.update(input)
    }

    fun beforeFrame(time: Long) { handles.beforeFrame(time) }
    fun needsFrame() = handles.needsFrame()

    fun touch(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                close()
                downX = event.x
                downY = event.y
            }
            MotionEvent.ACTION_MOVE -> {
                if (hypot(event.x - downX, event.y - downY) > slop) close()
            }
            MotionEvent.ACTION_POINTER_DOWN, MotionEvent.ACTION_CANCEL -> close()
        }
    }

    fun longPress(x: Float, y: Float): Boolean {
        downX = x
        downY = y
        val next = view.focusTextInput(x, y) ?: return false
        selectWord(next)
        state = current()
        mode = view.startActionMode(this, ActionMode.TYPE_FLOATING)
        if (mode == null) { state = null; return false }
        state?.let { handles.update(it) }
        return true
    }

    private fun selectWord(input: TextInputState) {
        val index = view.inputIndex(input.epoch, downX, downY)
        if (index < 0) return
        val text = input.text
        if (input.sensitive || text.isNullOrEmpty()) {
            view.selectText(input.epoch, index, index)
            return
        }
        val local = index - input.offset
        if (local !in 0..text.length) return
        val probe = local.coerceAtMost(text.length - 1)
        val words = BreakIterator.getWordInstance(view.resources.configuration.locales[0])
        words.setText(text)
        val start = words.preceding(probe + 1)
        val end = words.following(probe)
        if (start != BreakIterator.DONE && end != BreakIterator.DONE) {
            view.selectText(input.epoch, input.offset + start, input.offset + end)
        }
    }

    fun close() {
        handles.close()
        mode?.finish()
        mode = null
        state = null
    }

    fun update(next: TextInputState?) {
        val previous = state ?: return
        if (next == null || next.epoch != previous.epoch) { close(); return }
        state = next
        handles.update(next)
        if (next.anchor != previous.anchor || next.head != previous.head) mode?.invalidate()
        if (!next.headBounds.contentEquals(previous.headBounds) ||
            !next.editorBounds.contentEquals(previous.editorBounds)) mode?.invalidateContentRect()
    }

    override fun onCreateActionMode(mode: ActionMode, menu: Menu) = prepare(menu)
    override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = prepare(menu)

    private fun prepare(menu: Menu): Boolean {
        val input = state ?: return false
        menu.clear()
        fun item(id: Int, label: Int) { menu.add(0, id, Menu.NONE, label).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM) }
        item(android.R.id.selectAll, android.R.string.selectAll)
        if (!input.sensitive && input.text != null && input.anchor != input.head) {
            item(android.R.id.cut, android.R.string.cut)
            item(android.R.id.copy, android.R.string.copy)
        }
        val clipboard = view.context.getSystemService(ClipboardManager::class.java)
        if (clipboard.primaryClipDescription?.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN) == true) {
            item(android.R.id.paste, android.R.string.paste)
        }
        return true
    }

    override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
        val input = state ?: return false
        val handled = view.performTextAction(input.epoch, item.itemId)
        if (item.itemId == android.R.id.selectAll && handled) {
            update(current())
            mode.invalidate()
        } else close()
        return handled
    }

    override fun onDestroyActionMode(mode: ActionMode) { handles.close(); this.mode = null; state = null }

    override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) {
        val input = state
        val bounds = input?.headBounds ?: input?.editorBounds
        if (bounds == null) { outRect.setEmpty(); return }
        val density = view.resources.displayMetrics.density
        val height = this.view.viewportHeight()
        val left = floor(bounds[0] * density).toInt().coerceIn(0, view.width)
        val top = floor(bounds[1] * density).toInt().coerceIn(0, height)
        outRect.set(left, top,
            ceil(bounds[2] * density).toInt().coerceIn(left, view.width),
            ceil(bounds[3] * density).toInt().coerceIn(top, height))
    }
}
