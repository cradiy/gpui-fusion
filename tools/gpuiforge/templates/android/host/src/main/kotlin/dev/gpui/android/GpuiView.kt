package dev.gpui.android

import android.content.Context
import android.graphics.PointF
import android.os.Build
import android.util.SparseArray
import android.view.Choreographer
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewConfiguration
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import java.util.Collections
import java.util.WeakHashMap
import kotlin.math.hypot

/** A GPUI rendering surface. The caller owns and eventually closes its session. */
class GpuiView(context: Context, private val session: GpuiSession) :
    SurfaceView(context), SurfaceHolder.Callback2, Choreographer.FrameCallback {
    private val choreographer = Choreographer.getInstance()
    private val scroll = TouchScroll(context, session)
    private val contacts = SparseArray<PointF>()
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop
    private var initialized = false
    private var surfaceReady = false
    private var surfaceWidth = 0
    private var surfaceHeight = 0
    private var bottomInset = 0
    private var framePosted = false
    private var tapCandidate = false
    private var downX = 0f
    private var downY = 0f
    private var tapX = 0f
    private var tapY = 0f
    private var inputState: TextInputState? = null
    private var inputConnection: GpuiInputConnection? = null
    // A connection request does not necessarily replace the connection served by the IME.
    private val inputConnections = Collections.newSetFromMap(WeakHashMap<GpuiInputConnection, Boolean>())
    private enum class KeyboardRequest { TAP, SHOW, HIDE }
    private var keyboardRequest: KeyboardRequest? = null

    init {
        GpuiSession.checkThread()
        initialized = true
        holder.addCallback(this)
        isFocusableInTouchMode = true
        isClickable = true
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        session.bind(this)
    }

    override fun onDetachedFromWindow() {
        releaseSurface()
        session.unbind(this)
        super.onDetachedFromWindow()
    }

    override fun surfaceCreated(holder: SurfaceHolder) = Unit

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (width <= 0 || height <= 0) { releaseSurface(); return }
        cancelTouches()
        try {
            session.surface(holder.surface, width, height, resources.displayMetrics.density)
            surfaceWidth = width
            surfaceHeight = height
            updateViewport()
        } catch (error: RuntimeException) {
            session.fail(error)
            return
        }
        surfaceReady = true
        updateFrameScheduling()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) = releaseSurface()

    /** Excludes an occluded bottom region from GPUI layout without resizing the GPU surface. Uses physical pixels. */
    fun setViewportBottomInset(inset: Int) {
        GpuiSession.checkThread()
        require(inset >= 0)
        if (bottomInset == inset) return
        bottomInset = inset
        if (surfaceReady) {
            try { updateViewport(); session.redraw() }
            catch (error: RuntimeException) { session.fail(error) }
        }
    }

    private fun updateViewport() {
        session.viewport(surfaceWidth, (surfaceHeight - bottomInset).coerceAtLeast(1), resources.displayMetrics.density)
    }

    internal fun viewportHeight() = (height - bottomInset).coerceAtLeast(0)

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        if (!surfaceReady) return
        try { session.redraw() }
        catch (error: RuntimeException) { session.fail(error) }
    }

    internal fun releaseSurface() {
        keyboardRequest = null
        closeInput()
        choreographer.removeFrameCallback(this)
        framePosted = false
        cancelTouches()
        if (surfaceReady) {
            surfaceReady = false
            session.focus(false)
            session.detachSurface()
        }
    }

    override fun onWindowFocusChanged(focused: Boolean) {
        super.onWindowFocusChanged(focused)
        inputDiagnostic { "window focus=$focused" }
        if (initialized) updateFrameScheduling()
    }

    override fun onVisibilityChanged(changedView: View, visibility: Int) {
        super.onVisibilityChanged(changedView, visibility)
        if (initialized) updateFrameScheduling()
    }

    override fun onWindowVisibilityChanged(visibility: Int) {
        super.onWindowVisibilityChanged(visibility)
        if (initialized) updateFrameScheduling()
    }

    internal fun updateFrameScheduling() {
        val active = surfaceReady && session.active() && hasWindowFocus() && isShown
        session.focus(active)
        if (active && !framePosted) {
            framePosted = true
            choreographer.postFrameCallback(this)
        } else if (!active) {
            keyboardRequest = null
            choreographer.removeFrameCallback(this)
            framePosted = false
            cancelTouches()
        }
    }

    override fun doFrame(frameTimeNanos: Long) {
        framePosted = false
        if (surfaceReady && session.active() && hasWindowFocus() && isShown) {
            try {
                scroll.frame()
                val changed = session.frame()
                val request = keyboardRequest
                if (request == KeyboardRequest.HIDE) {
                    keyboardRequest = null
                    if (changed) syncInput(false)
                    inputManager().hideSoftInputFromWindow(windowToken, 0)
                } else if (changed || request != null) {
                    if (syncInput(request != null, requireHit = request != KeyboardRequest.SHOW)) {
                        if (keyboardRequest == request) keyboardRequest = null
                    }
                }
                for (connection in inputConnections.toList()) connection.updateCursorAnchor(inputState)
            } catch (error: RuntimeException) {
                session.fail(error)
                return
            }
        }
        updateFrameScheduling()
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!surfaceReady || !session.active()) return false
        if (event.actionMasked == MotionEvent.ACTION_DOWN && event.y >= height - bottomInset) return false
        return try { dispatchTouch(event) }
        catch (error: RuntimeException) { session.fail(error); true }
    }

    private fun dispatchTouch(event: MotionEvent): Boolean {
        val action = event.actionMasked
        val index = event.actionIndex
        if (action == MotionEvent.ACTION_DOWN) {
            requestFocus()
            tapCandidate = !scroll.begin(event)
            downX = event.x
            downY = event.y
        }
        if (event.pointerCount > 1) {
            tapCandidate = false
            scroll.block()
        }
        when (action) {
            MotionEvent.ACTION_CANCEL -> cancelTouches()
            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) updateTouch(event, i, 1)
                if (hypot(event.x - downX, event.y - downY) > touchSlop) tapCandidate = false
                if (scroll.event(event)) tapCandidate = false
            }
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                updateTouch(event, index, 0)
                scroll.event(event)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                updateTouch(event, index, 2)
                contacts.remove(event.getPointerId(index))
                if (scroll.event(event)) tapCandidate = false
                if (action == MotionEvent.ACTION_UP) {
                    if (tapCandidate && event.eventTime - event.downTime < ViewConfiguration.getLongPressTimeout()
                        && hypot(event.x - downX, event.y - downY) <= touchSlop) {
                        tapX = event.x
                        tapY = event.y
                        performClick()
                    }
                    tapCandidate = false
                }
            }
        }
        return true
    }

    private fun updateTouch(event: MotionEvent, index: Int, phase: Int) {
        val id = event.getPointerId(index)
        val x = event.getX(index)
        val y = event.getY(index)
        contacts.put(id, PointF(x, y))
        if (session.touch(id, phase, x, y)) {
            tapCandidate = false
            scroll.block()
        }
    }

    private fun cancelTouches() {
        tapCandidate = false
        scroll.cancel()
        for (i in 0 until contacts.size()) {
            val point = contacts.valueAt(i)
            session.touch(contacts.keyAt(i), 3, point.x, point.y)
        }
        contacts.clear()
    }

    override fun performClick(): Boolean {
        super.performClick()
        inputDiagnostic { "tap epoch=${inputState?.epoch}" }
        inputConnection?.finishComposingText()
        keyboardRequest = KeyboardRequest.TAP
        session.tap(tapX, tapY)
        return true
    }

    internal fun requestSoftKeyboard(visible: Boolean) {
        if (!surfaceReady || !session.active() || !hasWindowFocus() || !isShown) return
        if (visible && !requestFocus()) return
        keyboardRequest = if (visible) KeyboardRequest.SHOW else KeyboardRequest.HIDE
    }

    internal fun inputManager(): InputMethodManager = context.getSystemService(InputMethodManager::class.java)

    private fun closeInput() {
        val previous = inputConnections.toList()
        inputConnections.clear()
        inputConnection = null
        for (connection in previous) connection.closeConnection()
        inputState = null
    }

    internal fun inputConnectionClosed(connection: GpuiInputConnection) {
        inputConnections.remove(connection)
        if (inputConnection === connection) inputConnection = inputConnections.firstOrNull()
    }

    internal fun syncInput(show: Boolean, requireHit: Boolean = true): Boolean {
        if (!surfaceReady || !hasWindowFocus()) return false
        val next = session.inputState()
        if (next?.epoch != inputState?.epoch) {
            inputDiagnostic { "restart old=${inputState?.epoch} new=${next?.epoch} show=$show batch=${inputConnection?.batching()}" }
            closeInput()
            inputState = next
            inputManager().restartInput(this)
            if (next == null) inputManager().hideSoftInputFromWindow(windowToken, 0)
        } else {
            // Batches defer updates within one editor, never a change of editor.
            if (inputConnections.any { it.batching() }) return false
            inputState = next
        }
        if (next != null) {
            inputManager().updateSelection(this, next.anchor, next.head, next.composingStart, next.composingEnd)
            for (connection in inputConnections.toList()) connection.updateExtracted(next)
            if (show && (!requireHit || next.hit)) inputManager().showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
        }
        return true
    }

    override fun onCheckIsTextEditor() = inputState != null

    override fun onCreateInputConnection(info: EditorInfo): InputConnection? {
        inputDiagnostic { "onCreateInputConnection epoch=${inputState?.epoch} surface=$surfaceReady" }
        if (inputState == null || !surfaceReady) return null
        val state = session.inputState()
        inputState = state
        if (state == null) return null
        info.inputType = state.inputType
        info.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING or
            state.action or (if (state.action == EditorInfo.IME_ACTION_NONE) EditorInfo.IME_FLAG_NO_ENTER_ACTION else 0)
        info.initialSelStart = state.anchor
        info.initialSelEnd = state.head
        if (Build.VERSION.SDK_INT >= 30 && state.text != null) {
            info.setInitialSurroundingSubText(state.text, state.offset)
        }
        return GpuiInputConnection.create(this, session, state.epoch).also {
            inputConnections.add(it)
            inputConnection = it
        }
    }

    override fun onKeyDown(code: Int, event: KeyEvent) = handleKey(code, event, true) || super.onKeyDown(code, event)
    override fun onKeyUp(code: Int, event: KeyEvent) = handleKey(code, event, false) || super.onKeyUp(code, event)

    private fun handleKey(code: Int, event: KeyEvent, down: Boolean): Boolean {
        if (!surfaceReady) return false
        val key = when (code) {
            KeyEvent.KEYCODE_DEL -> "backspace"
            KeyEvent.KEYCODE_FORWARD_DEL -> "delete"
            KeyEvent.KEYCODE_ENTER -> "enter"
            KeyEvent.KEYCODE_DPAD_LEFT -> "left"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "right"
            KeyEvent.KEYCODE_DPAD_UP -> "up"
            KeyEvent.KEYCODE_DPAD_DOWN -> "down"
            KeyEvent.KEYCODE_MOVE_HOME -> "home"
            KeyEvent.KEYCODE_MOVE_END -> "end"
            else -> {
                if (event.isCtrlPressed || event.isMetaPressed) {
                    val character = event.getUnicodeChar(0)
                    if (character == 0) return false
                    String(Character.toChars(character))
                } else {
                    val character = event.unicodeChar
                    val state = inputState ?: return false
                    if (character == 0 || character and KeyCharacterMap.COMBINING_ACCENT != 0) return false
                    if (down) session.edit(state.epoch, 0, String(Character.toChars(character)), 1, 0)
                    syncInput(false)
                    return true
                }
            }
        }
        val modifiers = (if (event.isShiftPressed) 1 else 0) or (if (event.isCtrlPressed) 2 else 0) or
            (if (event.isAltPressed) 4 else 0) or (if (event.isMetaPressed) 8 else 0)
        session.key(key, modifiers, down)
        syncInput(false)
        return true
    }
}
