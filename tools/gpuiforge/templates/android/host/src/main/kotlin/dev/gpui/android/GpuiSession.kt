package dev.gpui.android

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Surface
import java.lang.ref.WeakReference
import java.util.function.Consumer
import java.util.concurrent.atomic.AtomicBoolean

/** Owns a Rust application independently of its current View or Surface. */
class GpuiSession : AutoCloseable {
    private val handler = Handler(Looper.getMainLooper())
    private var view = WeakReference<GpuiView>(null)
    @Volatile private var closed = false
    private var id = 0L
    private val frameWakePosted = AtomicBoolean(false)
    private val pendingUrls = ArrayList<String>()
    private val pendingShares = ArrayList<IncomingShare>()
    private var phase = BACKGROUND
    private var closeRequested: Runnable? = null
    private var errorHandler: Consumer<RuntimeException>? = null
    private var backEnabled = false
    private var backGestureActive = false
    private var backProgress = 0f
    private var backEdge = 0
    private var backChanged: Consumer<Boolean>? = null
    private var keyboardRequestVersion = 0L
    private var permissionHostVersion = 0L
    private val permissions = PermissionHost { token, status ->
        handler.postAtTime({
            if (!closed && id != 0L) nativePermissionResult(id, token, status)
        }, this, SystemClock.uptimeMillis())
    }
    private val files = FilePickerHost { token, documents, error ->
        handler.postAtTime({
            if (!closed && id != 0L) nativeFileResult(id, token, documents, error)
        }, this, SystemClock.uptimeMillis())
    }

    /** Attach the Activity used by the system document picker. */
    fun attachFileHost(activity: Activity) { checkThread(); check(!closed); files.attach(activity) }

    /** Retained configuration changes keep the pending picker; other detachments cancel it. */
    fun detachFileHost(activity: Activity) { checkThread(); files.detach(activity) }

    /** Returns true for a file result owned by this session. Codes 0x8000..0xbfff are reserved. */
    fun onActivityResult(activity: Activity, code: Int, result: Int, data: Intent?): Boolean {
        checkThread()
        return !closed && files.result(activity, code, result, data)
    }

    private fun requestFiles(token: Long, multiple: Boolean, writable: Boolean) {
        handler.postAtTime({
            if (!closed) files.request(token, multiple, writable, active())
        }, this, SystemClock.uptimeMillis())
    }

    private fun requestFileSave(token: Long, name: String, mime: String) {
        handler.postAtTime({
            if (!closed) files.create(token, name, mime, active())
        }, this, SystemClock.uptimeMillis())
    }

    private fun fileStore(): FileStore = FileStore(requireContext())
    private fun credentialStore(): CredentialStore = CredentialStore(requireContext())

    init { checkThread() }

    internal fun bind(next: GpuiView) {
        checkThread()
        check(!closed) { "GpuiSession is closed" }
        val current = view.get()
        check(current == null || current === next) {
            "Detach the previous GpuiView before attaching another"
        }
        view = WeakReference(next)
    }

    internal fun unbind(previous: GpuiView) {
        checkThread()
        if (view.get() === previous) {
            keyboardRequestVersion++
            view.clear()
        }
    }

    internal fun surface(surface: Surface, width: Int, height: Int, density: Float) {
        checkThread()
        if (closed) return
        if (id == 0L) {
            id = nativeCreate(this, surface, width, height, density)
            nativeLifecycle(id, phase)
        } else {
            nativeAttach(id, surface, width, height, density)
        }
        updateAppearance()
        for (url in pendingUrls) nativeOpenUrl(id, url)
        pendingUrls.clear()
        deliverShares()
    }

    /** Forwards VIEW URLs and SEND/SEND_MULTIPLE shares, including before the first Surface. */
    fun onOpenIntent(intent: Intent): Boolean {
        checkThread()
        if (closed) return false
        if (intent.action == Intent.ACTION_SEND || intent.action == Intent.ACTION_SEND_MULTIPLE) {
            pendingShares.add(IncomingShare.parse(intent))
            deliverShares()
            view.get()?.requestFrame()
            return true
        }
        if (intent.action != Intent.ACTION_VIEW) return false
        val uri = intent.data ?: return false
        if (uri.scheme.isNullOrEmpty()) return false
        val url = uri.toString()
        if (id == 0L) pendingUrls.add(url) else nativeOpenUrl(id, url)
        view.get()?.requestFrame()
        return true
    }

    private fun deliverShares() {
        if (id == 0L || pendingShares.isEmpty()) return
        val context = view.get()?.context?.applicationContext ?: return
        val shares = pendingShares.toList()
        pendingShares.clear()
        for (share in shares) {
            val documents = share.uris.map { SelectedDocument(context.contentResolver, it, false) }.toTypedArray()
            nativeReceiveShare(id, share.text, share.mime, documents, share.error)
        }
    }

    private fun darkAppearance(): Boolean =
        view.get()?.resources?.configuration?.uiMode?.and(Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES

    internal fun updateAppearance() {
        checkThread()
        if (!closed && id != 0L) nativeAppearance(id, darkAppearance())
    }

    internal fun detachSurface() { checkThread(); cancelBackGesture(); if (id != 0L) nativeDetach(id) }
    internal fun frame(): Boolean { checkThread(); return id != 0L && nativeFrame(id) }
    internal fun redraw() { checkThread(); if (!closed && id != 0L) nativeRedraw(id) }
    internal fun viewport(width: Int, height: Int, density: Float, insets: GpuiWindowInsets) {
        checkThread()
        if (!closed && id != 0L) nativeViewport(id, width, height, density, insets.values())
    }

    private fun systemFontPaths(): Array<String> {
        if (Build.VERSION.SDK_INT >= 29) {
            return android.graphics.fonts.SystemFonts.getAvailableFonts()
                .mapNotNull { it.file?.absolutePath }.distinct().sorted().toTypedArray()
        }
        return java.io.File("/system/fonts").listFiles().orEmpty()
            .filter { it.isFile && it.extension.lowercase() in setOf("ttf", "otf", "ttc") }
            .map { it.absolutePath }.sorted().toTypedArray()
    }
    internal fun inputState(): TextInputState? { checkThread(); return if (id != 0L) nativeInputState(id) else null }
    internal fun inputIndex(epoch: Long, x: Float, y: Float): Int {
        checkThread()
        return if (!closed && id != 0L) nativeInputIndex(id, epoch, x, y) else -1
    }
    internal fun scrollInput(epoch: Long, dx: Float, dy: Float): Boolean {
        checkThread()
        return !closed && id != 0L && nativeScrollInput(id, epoch, dx, dy)
    }
    internal fun edit(epoch: Long, operation: Int, text: String, a: Int, b: Int, cursor: Int = 1): Boolean {
        checkThread()
        return !closed && id != 0L && nativeEdit(id, epoch, operation, text, a, b, cursor)
    }
    internal fun key(name: String, modifiers: Int, down: Boolean): Boolean {
        checkThread()
        return !closed && id != 0L && nativeKey(id, name, modifiers, down)
    }

    internal fun inputAction(epoch: Long, action: Int): Boolean {
        checkThread()
        return !closed && id != 0L && nativeInputAction(id, epoch, action)
    }
    internal fun focus(focused: Boolean) {
        checkThread()
        if (!focused) keyboardRequestVersion++
        if (id != 0L) nativeFocus(id, focused)
    }
    internal fun touch(pointer: Int, phase: Int, x: Float, y: Float) =
        id != 0L && nativeTouch(id, pointer, phase, x, y)
    internal fun tap(x: Float, y: Float) { if (id != 0L) nativeTap(id, x, y) }
    internal fun focusTextInput(x: Float, y: Float): Boolean {
        checkThread()
        return !closed && id != 0L && nativeFocusTextInput(id, x, y)
    }
    internal fun scroll(phase: Int, x: Float, y: Float, dx: Float, dy: Float) {
        if (id != 0L) nativeScroll(id, phase, x, y, dx, dy)
    }
    internal fun pinch(phase: Int, x: Float, y: Float, delta: Float) {
        if (id != 0L) nativePinch(id, phase, x, y, delta)
    }
    internal fun longPress(x: Float, y: Float): Boolean =
        !closed && id != 0L && nativeLongPress(id, x, y)
    internal fun mouse(kind: Int, x: Float, y: Float, button: Int, pressed: Int,
                       clicks: Int, modifiers: Int, dx: Float = 0f, dy: Float = 0f) {
        if (id != 0L) nativeMouse(id, kind, x, y, button, pressed, clicks, modifiers, dx, dy)
    }
    internal fun active() = !closed && phase == ACTIVE
    fun isClosed() = closed

    /** Attach an Activity and forward its permission results to this session. */
    fun attachPermissionHost(activity: Activity) { checkThread(); check(!closed); permissions.attach(activity); permissionHostVersion++ }

    /** Detaching cancels pending Rust requests, including during configuration changes. */
    fun detachPermissionHost(activity: Activity) { checkThread(); permissions.detach(activity); permissionHostVersion++ }

    /** Returns true for a permission result owned by this session. Codes 0x4700..0x7fff are reserved. */
    fun onRequestPermissionsResult(activity: Activity, code: Int, names: Array<out String>, results: IntArray): Boolean {
        checkThread()
        return !closed && permissions.result(activity, code, names, results)
    }

    private fun permissionStatus(permission: String): Int { checkThread(); return if (closed) -4 else permissions.status(permission) }

    private fun requestPermission(permission: String, token: Long) {
        val version = permissionHostVersion
        handler.postAtTime({
            if (!closed) {
                if (version == permissionHostVersion) permissions.request(permission, token, active())
                else if (id != 0L) nativePermissionResult(id, token, -1)
            }
        }, this, SystemClock.uptimeMillis())
    }

    private fun cancelPermission(token: Long) {
        handler.postAtTime({ if (!closed) permissions.cancel(token) }, this, SystemClock.uptimeMillis())
    }

    /** Reports whether Rust currently handles Back. The host owns callback registration. */
    fun setOnBackEnabledChanged(callback: Consumer<Boolean>?) {
        checkThread()
        backChanged = callback
        callback?.accept(!closed && backEnabled)
    }

    /** Dispatches committed system Back after the IME has had a chance to consume it. */
    fun handleSystemBack(): Boolean {
        checkThread()
        if (!active() || !backEnabled || id == 0L) { cancelBackGesture(); return false }
        keyboardRequestVersion++
        view.get()?.requestSoftKeyboard(false)
        return try {
            finishBackGesture(2)
            nativeBack(id)
        } catch (error: RuntimeException) { fail(error); false }
    }

    /** Starts a preview from the platform's Back animation callback. Main thread only. */
    fun startBackGesture(progress: Float, edge: GpuiBackEdge) {
        checkThread()
        require(progress.isFinite() && progress in 0f..1f)
        cancelBackGesture()
        if (!active() || !backEnabled || id == 0L) return
        backGestureActive = true
        backProgress = progress
        backEdge = edge.ordinal
        try { nativeBackGesture(id, 0, backProgress, backEdge) }
        catch (error: RuntimeException) { fail(error) }
    }

    /** Updates an active preview; it does not navigate. Main thread only. */
    fun progressBackGesture(progress: Float) {
        checkThread()
        require(progress.isFinite() && progress in 0f..1f)
        if (!backGestureActive) return
        backProgress = progress
        try { nativeBackGesture(id, 1, backProgress, backEdge) }
        catch (error: RuntimeException) { fail(error) }
    }

    /** Cancels an unfinished preview when the gesture or host registration ends. */
    fun cancelBackGesture() {
        checkThread()
        try { finishBackGesture(3) }
        catch (error: RuntimeException) { fail(error) }
    }

    /** Hides the soft keyboard without navigating or clearing input focus. */
    fun hideSoftKeyboard() {
        checkThread()
        keyboardRequestVersion++
        cancelBackGesture()
        view.get()?.requestSoftKeyboard(false)
    }

    private fun finishBackGesture(phase: Int) {
        if (!backGestureActive) return
        backGestureActive = false
        if (id != 0L) nativeBackGesture(id, phase, backProgress, backEdge)
    }

    /** Forward the host's onStart/onResume/onPause/onStop transitions. */
    fun setLifecycle(next: Int) {
        checkThread()
        require(next in FOREGROUND..BACKGROUND) { "Invalid lifecycle phase" }
        if (closed || next == phase) return
        if (next != ACTIVE) cancelBackGesture()
        phase = next
        if (next != ACTIVE) keyboardRequestVersion++
        if (id != 0L) nativeLifecycle(id, phase)
        view.get()?.updateFrameScheduling()
    }

    /** Supplies the host-specific action for a GPUI quit request. */
    fun setOnCloseRequested(callback: Runnable?) { checkThread(); closeRequested = callback }

    /** Receives terminal rendering/initialization errors after the session closes. */
    fun setOnError(callback: Consumer<RuntimeException>?) { checkThread(); errorHandler = callback }

    internal fun fail(error: RuntimeException) {
        val callback = errorHandler
        try { close() } catch (closeError: RuntimeException) { error.addSuppressed(closeError) }
        if (callback != null) callback.accept(error)
        else handler.post { throw error }
    }

    // JNI callbacks retain their JVM names. Dispatch is asynchronous to avoid
    // re-entering a borrowed GPUI App from foreground or background threads.
    private fun requestFrame() {
        if (closed || !frameWakePosted.compareAndSet(false, true)) return
        handler.postAtTime({
            frameWakePosted.set(false)
            if (!closed) view.get()?.requestFrame()
        }, this, SystemClock.uptimeMillis())
    }

    private fun scheduleTask(token: Long, delayMillis: Long) {
        if (closed) return
        handler.postAtTime({
            if (!closed && id != 0L) {
                try { nativeRunTask(id, token) } catch (error: RuntimeException) { fail(error) }
            }
        }, this, SystemClock.uptimeMillis() + delayMillis)
    }

    private fun requestClose() {
        handler.postAtTime({
            if (!closed) closeRequested?.run()
        }, this, SystemClock.uptimeMillis())
    }

    private fun setBackEnabled(enabled: Boolean) {
        handler.postAtTime({
            if (!closed && backEnabled != enabled) {
                backEnabled = enabled
                if (!enabled) cancelBackGesture()
                backChanged?.accept(enabled)
            }
        }, this, SystemClock.uptimeMillis())
    }

    private fun setKeyboardVisible(visible: Boolean) {
        if (!active()) return
        val target = view.get() ?: return
        val version = ++keyboardRequestVersion
        handler.postAtTime({
            if (active() && keyboardRequestVersion == version && view.get() === target) {
                target.requestSoftKeyboard(visible)
            }
        }, this, SystemClock.uptimeMillis())
    }

    private fun setCursor(type: Int) {
        view.get()?.let { target ->
            target.pointerIcon = android.view.PointerIcon.getSystemIcon(target.context, type)
        }
    }

    private fun requireContext(): Context {
        checkThread()
        val current = view.get()
        check(!closed && current != null) { "Android system services require an attached GpuiView" }
        return current.context
    }

    private fun readClipboard(): String? {
        val clipboard = requireContext().getSystemService(ClipboardManager::class.java)
        val clip = clipboard.primaryClip ?: return null
        val items = (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).text }
        return if (items.isEmpty()) null else items.joinToString("\n")
    }

    private fun writeClipboard(text: String) {
        requireContext().getSystemService(ClipboardManager::class.java)
            .setPrimaryClip(ClipData.newPlainText("", text))
    }

    private fun openUrl(url: String) {
        val uri = Uri.parse(url)
        require(uri.scheme != null) { "URL must have a scheme" }
        startIntent(Intent(Intent.ACTION_VIEW, uri))
    }

    private fun startIntent(intent: Intent) {
        val context = requireContext()
        var owner = context
        while (owner !is Activity && owner is ContextWrapper) {
            val base = owner.baseContext
            if (base === owner) break
            owner = base
        }
        if (owner !is Activity) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
    }

    private fun openFileIntent(intent: Intent): String? = try {
        startIntent(intent)
        null
    } catch (error: Exception) {
        error.toString()
    }

    private fun share(text: String?, title: String?, files: Array<Intent>): String? = try {
        startIntent(ShareIntent.create(text, title, files))
        null
    } catch (error: Exception) {
        error.toString()
    }

    /** Releases the Rust application. Do not close during a retained Activity recreation. */
    override fun close() {
        checkThread()
        if (closed) return
        closed = true
        cancelBackGesture()
        pendingUrls.clear()
        pendingShares.clear()
        permissions.close()
        files.close()
        try {
            view.get()?.releaseSurface()
        } finally {
            handler.removeCallbacksAndMessages(this)
            val previous = id
            id = 0L
            try {
                if (previous != 0L) nativeClose(previous)
            } finally {
                backEnabled = false
                backChanged?.accept(false)
                backChanged = null
                closeRequested = null
                errorHandler = null
                view.clear()
            }
        }
    }

    companion object {
        const val FOREGROUND = 0
        const val ACTIVE = 1
        const val INACTIVE = 2
        const val BACKGROUND = 3

        internal fun checkThread() {
            check(Looper.myLooper() == Looper.getMainLooper()) {
                "GpuiSession must be accessed on the main Looper"
            }
        }

        @JvmStatic private external fun nativeCreate(host: GpuiSession, surface: Surface, width: Int, height: Int, density: Float): Long
        @JvmStatic private external fun nativeAttach(id: Long, surface: Surface, width: Int, height: Int, density: Float)
        @JvmStatic private external fun nativeDetach(id: Long)
        @JvmStatic private external fun nativeFrame(id: Long): Boolean
        @JvmStatic private external fun nativeInputState(id: Long): TextInputState?
        @JvmStatic private external fun nativeInputIndex(id: Long, epoch: Long, x: Float, y: Float): Int
        @JvmStatic private external fun nativeScrollInput(id: Long, epoch: Long, dx: Float, dy: Float): Boolean
        @JvmStatic private external fun nativeEdit(id: Long, epoch: Long, operation: Int, text: String, a: Int, b: Int, cursor: Int): Boolean
        @JvmStatic private external fun nativeKey(id: Long, name: String, modifiers: Int, down: Boolean): Boolean
        @JvmStatic private external fun nativeInputAction(id: Long, epoch: Long, action: Int): Boolean
        @JvmStatic private external fun nativeLifecycle(id: Long, phase: Int)
        @JvmStatic private external fun nativeFocus(id: Long, active: Boolean)
        @JvmStatic private external fun nativeAppearance(id: Long, dark: Boolean)
        @JvmStatic private external fun nativeBack(id: Long): Boolean
        @JvmStatic private external fun nativeBackGesture(id: Long, phase: Int, progress: Float, edge: Int)
        @JvmStatic private external fun nativeTouch(id: Long, pointer: Int, phase: Int, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativePinch(id: Long, phase: Int, x: Float, y: Float, delta: Float)
        @JvmStatic private external fun nativeLongPress(id: Long, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativeTap(id: Long, x: Float, y: Float)
        @JvmStatic private external fun nativeOpenUrl(id: Long, url: String)
        @JvmStatic private external fun nativeReceiveShare(id: Long, text: String?, mime: String?, documents: Array<SelectedDocument>, error: String?)
        @JvmStatic private external fun nativeFocusTextInput(id: Long, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativeScroll(id: Long, phase: Int, x: Float, y: Float, dx: Float, dy: Float)
        @JvmStatic private external fun nativeMouse(id: Long, kind: Int, x: Float, y: Float,
            button: Int, pressed: Int, clicks: Int, modifiers: Int, dx: Float, dy: Float)
        @JvmStatic private external fun nativeRunTask(id: Long, token: Long)
        @JvmStatic private external fun nativeClose(id: Long)
        @JvmStatic private external fun nativePermissionResult(id: Long, token: Long, status: Int)
        @JvmStatic private external fun nativeFileResult(id: Long, token: Long, documents: Array<SelectedDocument>?, error: String?)
        @JvmStatic private external fun nativeRedraw(id: Long)
        @JvmStatic private external fun nativeViewport(id: Long, width: Int, height: Int, density: Float, insets: IntArray)
    }
}
