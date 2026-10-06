package dev.gpui.android

import android.annotation.TargetApi
import android.os.Build
import android.text.Editable
import android.text.TextUtils
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.TextAttribute

internal open class GpuiInputConnection(
    private val view: GpuiView,
    private val session: GpuiSession,
    private val epoch: Long,
) : BaseInputConnection(view, true) {
    private var closed = false
    private var batches = 0
    private var extractedToken = 0
    private var monitorExtracted = false
    private val cursorAnchor = CursorAnchorUpdates(view, epoch) { state() }
    private val connectionId = System.identityHashCode(this)

    init { inputDiagnostic { "create connection=$connectionId epoch=$epoch" } }

    override fun getEditable(): Editable? = null

    companion object {
        fun create(view: GpuiView, session: GpuiSession, epoch: Long): GpuiInputConnection =
            if (Build.VERSION.SDK_INT >= 34) Api34Connection(view, session, epoch)
            else GpuiInputConnection(view, session, epoch)
    }

    @TargetApi(34)
    private class Api34Connection(view: GpuiView, session: GpuiSession, epoch: Long) :
        GpuiInputConnection(view, session, epoch) {
        override fun replaceText(start: Int, end: Int, text: CharSequence, cursor: Int, attributes: TextAttribute?): Boolean {
            if (start < 0 || end < 0 || !beginBatchEdit()) return false
            try {
                return finishComposingText() && setSelection(minOf(start, end), maxOf(start, end)) && commitText(text, cursor)
            } finally {
                endBatchEdit()
            }
        }
    }

    private fun state(): TextInputState? =
        if (closed) null else session.inputState()?.takeIf { it.epoch == epoch }

    private fun edit(operation: Int, text: String, a: Int, b: Int): Boolean {
        if (closed) {
            inputDiagnostic { "edit connection=$connectionId op=$operation closed" }
            return false
        }
        return try {
            val result = session.edit(epoch, operation, text, a, b)
            inputDiagnostic { "edit connection=$connectionId epoch=$epoch op=$operation batch=$batches accepted=$result" }
            if (batches == 0) view.syncInput(false)
            result
        } catch (error: RuntimeException) {
            session.fail(error)
            false
        }
    }

    override fun commitText(text: CharSequence, cursor: Int) = edit(0, text.toString(), cursor, 0)
    override fun setComposingText(text: CharSequence, cursor: Int) = edit(1, text.toString(), cursor, 0)
    override fun finishComposingText() = edit(2, "", 0, 0)
    override fun setSelection(start: Int, end: Int) = edit(3, "", start, end)
    override fun setComposingRegion(start: Int, end: Int) = edit(4, "", start, end)
    override fun deleteSurroundingText(before: Int, after: Int) = edit(5, "", before, after)
    override fun deleteSurroundingTextInCodePoints(before: Int, after: Int) = edit(6, "", before, after)

    override fun beginBatchEdit(): Boolean {
        if (closed) return false
        batches++
        inputDiagnostic { "begin connection=$connectionId batch=$batches" }
        return true
    }
    override fun endBatchEdit(): Boolean {
        if (closed || batches == 0) return false
        --batches
        inputDiagnostic { "end connection=$connectionId batch=$batches" }
        if (batches == 0) view.syncInput(false)
        return batches > 0
    }
    fun batching() = batches > 0

    override fun requestCursorUpdates(mode: Int) = cursorAnchor.request(mode)
    override fun requestCursorUpdates(mode: Int, filter: Int) = cursorAnchor.request(mode or filter)
    fun updateCursorAnchor(state: TextInputState?) = cursorAnchor.update(state)

    override fun closeConnection() {
        if (closed) return
        inputDiagnostic { "close connection=$connectionId epoch=$epoch batch=$batches" }
        // Invalidate first: finishing composition can cause a focus refresh.
        closed = true
        cursorAnchor.close()
        batches = 0
        view.inputConnectionClosed(this)
        try { session.edit(epoch, 2, "", 0, 0) }
        finally { super.closeConnection() }
    }

    private fun slice(state: TextInputState, start: Int, end: Int): CharSequence? {
        val text = state.text ?: return null
        var from = (start - state.offset).coerceIn(0, text.length)
        var to = (end - state.offset).coerceIn(from, text.length)
        // A UTF-16 limit must not expose half a surrogate pair.
        if (from > 0 && from < text.length && text[from].isLowSurrogate()) from++
        if (to > from && to < text.length && text[to - 1].isHighSurrogate()) to--
        return text.substring(from, maxOf(from, to))
    }

    override fun getTextBeforeCursor(length: Int, flags: Int): CharSequence? {
        val state = state() ?: return null
        if (length < 0) return null
        val end = minOf(state.anchor, state.head)
        return slice(state, maxOf(0, end - length), end)
    }
    override fun getTextAfterCursor(length: Int, flags: Int): CharSequence? {
        val state = state() ?: return null
        if (length < 0) return null
        val start = maxOf(state.anchor, state.head)
        return slice(state, start, minOf(Int.MAX_VALUE.toLong(), start.toLong() + length).toInt())
    }
    override fun getSelectedText(flags: Int): CharSequence? {
        val state = state() ?: return null
        return slice(state, minOf(state.anchor, state.head), maxOf(state.anchor, state.head))
    }
    override fun getCursorCapsMode(modes: Int): Int {
        val state = state() ?: return 0
        val text = state.text ?: return 0
        val cursor = (state.head - state.offset).coerceIn(0, text.length)
        return TextUtils.getCapsMode(text, cursor, modes)
    }

    private fun extracted(state: TextInputState?): ExtractedText? {
        val text = state?.text ?: return null
        return ExtractedText().apply {
            this.text = text
            startOffset = state.offset
            partialStartOffset = -1
            partialEndOffset = -1
            selectionStart = state.anchor - state.offset
            selectionEnd = state.head - state.offset
        }
    }
    override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText? {
        if (request == null || closed) return null
        monitorExtracted = flags and InputConnection.GET_EXTRACTED_TEXT_MONITOR != 0
        extractedToken = request.token
        return extracted(state())
    }
    fun updateExtracted(state: TextInputState) {
        val text = extracted(state)
        if (monitorExtracted && text != null) view.inputManager().updateExtractedText(view, extractedToken, text)
    }

    override fun sendKeyEvent(event: KeyEvent) = state() != null && view.dispatchKeyEvent(event)
    override fun performEditorAction(action: Int): Boolean {
        return try {
            val current = state() ?: return false
            val expected = current.action
            if (action != expected && action != EditorInfo.IME_ACTION_UNSPECIFIED) return false
            if (!session.edit(epoch, 2, "", 0, 0)) return false
            if (state() == null) return false
            val handled = expected != EditorInfo.IME_ACTION_NONE && session.inputAction(epoch, expected)
            if (!handled) {
                if (state() == null || (expected != EditorInfo.IME_ACTION_NONE && expected != EditorInfo.IME_ACTION_DONE)) return false
                session.key("enter", 0, true)
                session.key("enter", 0, false)
            }
            view.syncInput(false)
            if (expected == EditorInfo.IME_ACTION_DONE && state() != null) view.requestSoftKeyboard(false)
            true
        } catch (error: RuntimeException) {
            session.fail(error)
            false
        }
    }
    override fun performContextMenuAction(id: Int): Boolean {
        return state() != null && view.performTextAction(epoch, id)
    }
}
