package dev.gpui.android

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentCallbacks2
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
import androidx.core.view.HapticFeedbackConstantsCompat
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
    private val delayedFrameWake = Runnable { if (!closed) view.get()?.requestFrame() }
    private val pendingUrls = ArrayList<String>()
// gpuiforge:if sharing
    private val pendingShares = ArrayList<IncomingShare>()
// gpuiforge:endif
    private var phase = BACKGROUND
    private var closeRequested: Runnable? = null
    private var errorHandler: Consumer<RuntimeException>? = null
    private var backEnabled = false
    private var backGestureActive = false
    private var backProgress = 0f
    private var backEdge = 0
    private var backChanged: Consumer<Boolean>? = null
    private var fullscreen = false
    private var fullscreenChanged: Consumer<Boolean>? = null
    private var systemBarAppearance = SystemBarAppearance()
    private var systemBarAppearanceChanged: Consumer<SystemBarAppearance>? = null
    private var systemBarAppearancePosted = false
    private var pictureInPicture = false
// gpuiforge:if media
    private var pictureInPictureHost: PictureInPictureHost? = null
    private var pictureInPictureSource: android.graphics.RectF? = null
    private var pictureInPictureSourcePosted = false

    /** Attach an Activity declaring supportsPictureInPicture in its manifest. */
    fun attachPictureInPictureHost(activity: Activity) {
        checkThread()
        check(!closed)
        pictureInPictureHost = PictureInPictureHost(activity)
        onPictureInPictureModeChanged(activity.isInPictureInPictureMode)
        updatePictureInPictureSource()
    }

    /** Detach the Activity before it is destroyed. */
    fun detachPictureInPictureHost(activity: Activity) {
        checkThread()
        if (pictureInPictureHost?.activity === activity) pictureInPictureHost = null
    }
// gpuiforge:endif

    /** Forward the Activity's actual picture-in-picture mode changes. */
    fun onPictureInPictureModeChanged(enabled: Boolean) {
        checkThread()
        if (closed || pictureInPicture == enabled) return
        pictureInPicture = enabled
        if (id != 0L) nativePictureInPictureChanged(id, enabled)
        if (!enabled) updatePictureInPictureSource()
        view.get()?.requestFrame()
    }

    private fun setPictureInPictureSourceBounds(left: Float, top: Float, right: Float, bottom: Float, valid: Boolean) {
        checkThread()
// gpuiforge:if media
        if (closed || pictureInPicture) return
        val next = if (valid) android.graphics.RectF(left, top, right, bottom) else null
        if (pictureInPictureSource == next) return
        pictureInPictureSource = next
        updatePictureInPictureSource()
// gpuiforge:endif
    }

    internal fun updatePictureInPictureSource() {
// gpuiforge:if media
        if (closed || pictureInPicture || pictureInPictureSourcePosted) return
        pictureInPictureSourcePosted = true
        handler.postAtTime({
            pictureInPictureSourcePosted = false
            if (!closed) pictureInPictureHost?.updateSource(view.get(), pictureInPictureSource)
        }, this, SystemClock.uptimeMillis())
// gpuiforge:endif
    }

    internal fun inPictureInPicture() = !closed && pictureInPicture && phase != BACKGROUND

    private fun supportsPictureInPicture(): Boolean {
// gpuiforge:if media
        return !closed && pictureInPictureHost?.supported() == true
// gpuiforge:else
        return false
// gpuiforge:endif
    }

    private fun enterPictureInPicture(width: Int, height: Int) {
        checkThread()
        // Activity transitions may synchronously resize the Rust window.
        handler.postAtTime({
            if (!closed && id != 0L) {
                val error = try {
// gpuiforge:if media
                    check(active()) { "Picture-in-picture requires an active Activity" }
                    val host = checkNotNull(pictureInPictureHost) { "No picture-in-picture host attached" }
                    host.updateSource(view.get(), pictureInPictureSource)
                    hideSoftKeyboard()
                    host.enter(width, height)
                    null
// gpuiforge:else
                    "Enable the Android media feature for picture-in-picture"
// gpuiforge:endif
                } catch (error: RuntimeException) {
                    error.message ?: "Unable to enter picture-in-picture"
                }
                nativePictureInPictureResult(id, error)
            }
        }, this, SystemClock.uptimeMillis())
    }
    private var keyboardRequestVersion = 0L
    private var permissionHostVersion = 0L
    private var networkMonitor: NetworkMonitor? = null
    private var thermalMonitor: AutoCloseable? = null
    private var thermalStatus = 0
    private var motionPreferences: MotionPreferences? = null

    private fun prefersReducedMotion(): Boolean { checkThread(); return motionPreferences?.reduced ?: false }

    private fun thermalStatus(): Int { checkThread(); return thermalStatus }

    private fun monitorThermalState(context: Context) {
        if (Build.VERSION.SDK_INT < 29 || thermalMonitor != null) return
        try {
            val monitor = ThermalMonitor(context, handler) { status ->
                thermalStatus = status
                if (!closed && id != 0L) nativeThermalStateChanged(id, status)
            }
            thermalMonitor = monitor
            thermalStatus = monitor.status
        } catch (error: RuntimeException) {
            android.util.Log.w("GPUI", "Thermal status monitoring unavailable", error)
        }
    }
    private var componentContext: Context? = null
    private val componentCallbacks = object : ComponentCallbacks2 {
        override fun onConfigurationChanged(configuration: Configuration) = Unit
        override fun onTrimMemory(level: Int) { dispatchMemoryTrim(level) }
        @Suppress("OVERRIDE_DEPRECATION", "DEPRECATION")
        override fun onLowMemory() { dispatchMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_COMPLETE) }
    }

    private fun dispatchMemoryTrim(level: Int) {
        // A system callback can arrive during an Activity or Surface operation.
        handler.postAtTime({
            if (!closed && id != 0L) nativeTrimMemory(id, level)
        }, this, SystemClock.uptimeMillis())
    }

    private fun networks(): NetworkMonitor {
        checkThread()
        check(!closed) { "GpuiSession is closed" }
        return networkMonitor ?: NetworkMonitor(requireContext(), handler) { token, status ->
            if (!closed && id != 0L) nativeNetworkChanged(id, token, status)
        }.also { networkMonitor = it }
    }

    private fun networkStatus(): Int = networks().snapshot()

    private fun observeNetwork(token: Long, enable: Boolean) {
        checkThread()
        if (enable) networks().subscribe(token) else networkMonitor?.unsubscribe(token)
    }

    private fun openAppSettings(token: Long, page: Int) {
        checkThread()
        check(!closed) { "GpuiSession is closed" }
        // Activity launch may change focus or resize the Surface. Leave Rust first.
        handler.postAtTime({
            if (!closed && id != 0L) {
                val error = try {
                    check(active()) { "Opening app settings requires an active GpuiView" }
                    val context = requireContext()
                    val intent = when (page) {
                        0 -> Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                            Uri.fromParts("package", context.packageName, null))
                        1 -> Intent(android.provider.Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                            .putExtra(android.provider.Settings.EXTRA_APP_PACKAGE, context.packageName)
                        else -> error("Unknown app settings page")
                    }
                    startIntent(intent)
                    null
                } catch (error: Exception) {
                    error.toString()
                }
                if (!closed && id != 0L) nativeSettingsResult(id, token, error)
            }
        }, this, SystemClock.uptimeMillis())
    }
    private val permissions = PermissionHost { token, status ->
        handler.postAtTime({
            if (!closed && id != 0L) nativePermissionResult(id, token, status)
        }, this, SystemClock.uptimeMillis())
    }
// gpuiforge:if files
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

    private fun requestFiles(token: Long, multiple: Boolean, writable: Boolean, mimeTypes: Array<String>) {
        handler.postAtTime({
            if (!closed) files.request(token, multiple, writable, active(), mimeTypes)
        }, this, SystemClock.uptimeMillis())
    }

    private fun requestDirectory(token: Long) {
        handler.postAtTime({
            if (!closed) files.directory(token, active())
        }, this, SystemClock.uptimeMillis())
    }

    private fun requestFileSave(token: Long, name: String, mime: String) {
        handler.postAtTime({
            if (!closed) files.create(token, name, mime, active())
        }, this, SystemClock.uptimeMillis())
    }

    private fun fileStore(): Any = FileStore(requireContext())
// gpuiforge:else
    private fun requestFiles(token: Long, multiple: Boolean, writable: Boolean, mimeTypes: Array<String>) { unsupported("files") }
    private fun requestDirectory(token: Long) { unsupported("files") }
    private fun requestFileSave(token: Long, name: String, mime: String) { unsupported("files") }
    private fun fileStore(): Any = unsupported("files")
// gpuiforge:endif
// gpuiforge:if credentials
    private fun credentialStore(): Any = CredentialStore(requireContext())
// gpuiforge:else
    private fun credentialStore(): Any = unsupported("credentials")
// gpuiforge:endif
    private fun unsupported(feature: String): Nothing = error("Enable '$feature' in platforms.android.features and run gpuiforge sync")
// gpuiforge:if data-sync
    private var dataSync: DataSyncHost? = null
// gpuiforge:endif
    private fun backgroundOperation(operation: String, payload: String): String? = try {
        checkThread()
        check(!closed) { "GPUI session is closed" }
        if (operation.startsWith("media_")) {
// gpuiforge:if background-media
            when (operation) {
                "media_start" -> {
                    check(active()) { "Start background playback from an active Activity after a user action" }
                    val request = org.json.JSONObject(payload)
                    val media = checkNotNull(mediaNotifications[request.getString("session")]) { "Media session closed" }
                    MediaPlaybackService.start(requireContext().applicationContext, media, request.getString("token")) { token, event, error ->
                        handler.post { if (!closed && id != 0L) nativeBackgroundEvent(id, token, event, error) }
                    }
                }
                "media_stop" -> MediaPlaybackService.stop(payload)
                else -> error("Unknown background media operation")
            }
            null
// gpuiforge:else
            unsupported("background-media")
// gpuiforge:endif
        } else {
// gpuiforge:if data-sync
            val host = dataSync ?: DataSyncHost(requireContext().applicationContext) { token, event, error ->
                handler.post { if (!closed && id != 0L) nativeBackgroundEvent(id, token, event, error) }
            }.also { dataSync = it }
            host.operation(operation, payload, active())
            null
// gpuiforge:else
            unsupported("data-sync")
// gpuiforge:endif
        }
    } catch (error: RuntimeException) { error.message ?: error.javaClass.simpleName }
// gpuiforge:if notifications
    private var notificationStore: NotificationStore? = null
// gpuiforge:endif
// gpuiforge:if media-notifications
    private val mediaNotifications = mutableMapOf<String, MediaNotification>()
// gpuiforge:endif
    private fun notificationOperation(operation: String, payload: String): String {
        checkThread()
        check(!closed) { "GPUI session is closed" }
// gpuiforge:if media-notifications
        when (operation) {
            "media_create" -> {
                val options = org.json.JSONObject(payload)
                require(options.getString("app_id") == requireContext().packageName)
                val key = options.getString("id")
                mediaNotifications[key] = MediaNotification(requireContext(), key) { event ->
                    handler.post { if (!closed && id != 0L) nativeMediaCommand(id, event) }
                }
                return ""
            }
            "media_update" -> {
                val update = org.json.JSONObject(payload)
                checkNotNull(mediaNotifications[update.getString("id")]) { "Media session closed" }.update(update.getJSONObject("state"))
                return ""
            }
            "media_artwork" -> {
                val update = org.json.JSONObject(payload)
                checkNotNull(mediaNotifications[update.getString("id")]) { "Media session closed" }
                    .setArtwork(if (update.isNull("png")) null else update.getString("png"))
                return ""
            }
            "media_close" -> { mediaNotifications.remove(payload)?.close(); return "" }
        }
// gpuiforge:else
        if (operation.startsWith("media_")) unsupported("media-notifications")
// gpuiforge:endif
// gpuiforge:if notifications
        val store = notificationStore ?: NotificationStore(requireContext()) {
            handler.post {
                if (!closed && id != 0L) notificationStore?.deliver { nativeNotificationEvent(id, it) }
            }
        }.also { notificationStore = it }
        return store.operation(operation, payload)
// gpuiforge:else
        unsupported("notifications")
// gpuiforge:endif
    }

    init { checkThread() }

    internal fun bind(next: GpuiView) {
        checkThread()
        check(!closed) { "GpuiSession is closed" }
        val current = view.get()
        check(current == null || current === next) {
            "Detach the previous GpuiView before attaching another"
        }
        view = WeakReference(next)
        if (componentContext == null) {
            val context = next.context.applicationContext
            context.registerComponentCallbacks(componentCallbacks)
            componentContext = context
        }
        monitorThermalState(next.context.applicationContext)
        if (motionPreferences == null) {
            try {
                motionPreferences = MotionPreferences(next.context, handler) { reduced ->
                    if (!closed && id != 0L) nativeReducedMotionChanged(id, reduced)
                }
            } catch (error: RuntimeException) {
                android.util.Log.w("GPUI", "Motion preferences unavailable", error)
            }
        }
    }

    internal fun unbind(previous: GpuiView) {
        checkThread()
        if (view.get() === previous) {
            resetAccessibility()
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
            nativePictureInPictureChanged(id, pictureInPicture)
        } else {
            nativeAttach(id, surface, width, height, density)
        }
        updateConfiguration()
        for (url in pendingUrls) nativeOpenUrl(id, url)
        pendingUrls.clear()
// gpuiforge:if sharing
        deliverShares()
// gpuiforge:endif
    }

    /** Forwards VIEW URLs and SEND/SEND_MULTIPLE shares, including before the first Surface. */
    fun onOpenIntent(intent: Intent): Boolean {
        checkThread()
        if (closed) return false
// gpuiforge:if sharing
        if (intent.action == Intent.ACTION_SEND || intent.action == Intent.ACTION_SEND_MULTIPLE) {
            pendingShares.add(IncomingShare.parse(intent))
            deliverShares()
            view.get()?.requestFrame()
            return true
        }
// gpuiforge:endif
        if (intent.action != Intent.ACTION_VIEW) return false
        val uri = intent.data ?: return false
        if (uri.scheme.isNullOrEmpty()) return false
        val url = uri.toString()
        if (id == 0L) pendingUrls.add(url) else nativeOpenUrl(id, url)
        view.get()?.requestFrame()
        return true
    }

// gpuiforge:if sharing
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
// gpuiforge:endif

    private fun darkAppearance(): Boolean =
        view.get()?.resources?.configuration?.uiMode?.and(Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES

    private var fontConfiguration: Pair<Float, Int>? = null

    private fun scaledFontSize(baseSize: Float): Float {
        checkThread()
        val metrics = requireContext().resources.displayMetrics
        return android.util.TypedValue.applyDimension(
            android.util.TypedValue.COMPLEX_UNIT_SP, baseSize, metrics
        ) / metrics.density
    }

    internal fun updateConfiguration() {
        checkThread()
        if (closed || id == 0L) return
        nativeAppearance(id, darkAppearance())
        val configuration = requireContext().resources.configuration
        val next = configuration.fontScale to configuration.densityDpi
        if (fontConfiguration != next) {
            fontConfiguration = next
            nativeFontSizeChanged(id)
        }
    }

    internal fun detachSurface() { checkThread(); cancelBackGesture(); if (id != 0L) nativeDetach(id) }
    internal fun frame(): Boolean { checkThread(); return id != 0L && nativeFrame(id) }

    private fun accessibilityView(): android.view.View? = view.get()?.takeIf {
        it.isAttachedToWindow && it.context.getSystemService(android.view.accessibility.AccessibilityManager::class.java).isEnabled
    }

    internal fun accessibilityNode(host: GpuiView, node: Int, focus: Boolean): android.view.accessibility.AccessibilityNodeInfo? {
        checkThread()
        if (closed || id == 0L || view.get() !== host) return null
        return nativeAccessibilityNode(id, host, node, focus)
    }

    internal fun accessibilityAction(host: GpuiView, node: Int, action: Int, arguments: android.os.Bundle?): Boolean {
        checkThread()
        return !closed && id != 0L && view.get() === host &&
            nativeAccessibilityAction(id, host, node, action, arguments)
    }

    internal fun accessibilityHover(host: GpuiView, action: Int, x: Float, y: Float): Boolean {
        checkThread()
        return !closed && id != 0L && view.get() === host &&
            nativeAccessibilityHover(id, host, action, x, y)
    }

    internal fun resetAccessibility() {
        checkThread()
        if (!closed && id != 0L) nativeAccessibilityReset(id)
    }
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
    private fun updateAutofill(fields: String) { view.get()?.autofillHost?.update(fields) }
    private fun finishAutofill(commit: Boolean) { view.get()?.autofillHost?.finish(commit) }
    internal fun autofill(field: String, value: String) {
        checkThread()
        if (id != 0L && !closed) { nativeAutofill(id, field, value); view.get()?.requestFrame() }
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

    /** Handles window-wide fullscreen requests. Receives the retained mode on attachment. */
    fun setOnFullscreenChanged(callback: Consumer<Boolean>?) {
        checkThread()
        fullscreenChanged = callback
        callback?.accept(!closed && fullscreen)
    }

    private fun setFullscreen(enabled: Boolean): Boolean {
        checkThread()
        if (closed || fullscreenChanged == null) return false
        fullscreen = enabled
        // Apply outside the Rust JNI call: system-bar changes can resize the View.
        handler.postAtTime({
            if (!closed) fullscreenChanged?.accept(fullscreen)
        }, this, SystemClock.uptimeMillis())
        return true
    }

    /** Applies retained foreground styles to the current host window. Main thread only. */
    fun setOnSystemBarAppearanceChanged(callback: Consumer<SystemBarAppearance>?) {
        checkThread()
        systemBarAppearanceChanged = callback
        callback?.accept(systemBarAppearance)
    }

    private fun setSystemBarAppearance(status: Int, navigation: Int): Boolean {
        checkThread()
        if (closed || systemBarAppearanceChanged == null) return false
        val next = SystemBarAppearance(SystemBarStyle.entries[status], SystemBarStyle.entries[navigation])
        if (systemBarAppearance == next) return true
        systemBarAppearance = next
        if (!systemBarAppearancePosted) {
            systemBarAppearancePosted = true
            handler.postAtTime({
                systemBarAppearancePosted = false
                if (!closed) systemBarAppearanceChanged?.accept(systemBarAppearance)
            }, this, SystemClock.uptimeMillis())
        }
        return true
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

    private fun requestFrameAfter(delayMillis: Long) {
        checkThread()
        if (closed) return
        handler.removeCallbacks(delayedFrameWake)
        handler.postAtTime(delayedFrameWake, this, SystemClock.uptimeMillis() + delayMillis)
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

    private fun performHaptic(kind: Int): Boolean {
        checkThread()
        if (!active()) return false
        val feedback = when (kind) {
            0 -> HapticFeedbackConstantsCompat.SEGMENT_TICK
            1 -> HapticFeedbackConstantsCompat.CONFIRM
            2 -> HapticFeedbackConstantsCompat.REJECT
            3 -> HapticFeedbackConstantsCompat.LONG_PRESS
            4 -> HapticFeedbackConstantsCompat.GESTURE_START
            5 -> HapticFeedbackConstantsCompat.GESTURE_END
            else -> return false
        }
        return view.get()?.systemHapticFeedback(feedback) ?: false
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

    private fun clipboardSnapshot(): Any? {
        val context = requireContext()
        val clip = context.getSystemService(ClipboardManager::class.java).primaryClip ?: return null
        return ClipboardSnapshot(context.contentResolver, clip)
    }

    private fun clipboardImage(): Any {
// gpuiforge:if files
        return ClipboardImage(requireContext())
// gpuiforge:else
        unsupported("files")
// gpuiforge:endif
    }

    private fun publishClipboardImage(image: Any) {
// gpuiforge:if files
        val export = image as ClipboardImage
        requireContext().getSystemService(ClipboardManager::class.java).setPrimaryClip(export.clip())
        export.retain()
// gpuiforge:else
        unsupported("files")
// gpuiforge:endif
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

// gpuiforge:if sharing
    private fun share(text: String?, title: String?, files: Array<Intent>): String? = try {
        startIntent(ShareIntent.create(text, title, files))
        null
    } catch (error: Exception) {
        error.toString()
    }
// gpuiforge:else
    private fun share(text: String?, title: String?, files: Array<Intent>): String? = "Enable 'sharing' in platforms.android.features and run gpuiforge sync"
// gpuiforge:endif

    /** Releases the Rust application. Do not close during a retained Activity recreation. */
    override fun close() {
// gpuiforge:if data-sync
        dataSync?.close()
        dataSync = null
// gpuiforge:endif
// gpuiforge:if media-notifications
        mediaNotifications.values.forEach { it.close() }
        mediaNotifications.clear()
// gpuiforge:endif
// gpuiforge:if notifications
        notificationStore?.close()
        notificationStore = null
// gpuiforge:endif
        checkThread()
        if (closed) return
        closed = true
        try {
            thermalMonitor?.close()
        } catch (error: Exception) {
            android.util.Log.w("GPUI", "Could not unregister thermal status listener", error)
        }
        thermalMonitor = null
        try {
            motionPreferences?.close()
        } catch (error: Exception) {
            android.util.Log.w("GPUI", "Could not unregister motion preference observer", error)
        }
        motionPreferences = null
        componentContext?.unregisterComponentCallbacks(componentCallbacks)
        componentContext = null
        cancelBackGesture()
        pendingUrls.clear()
// gpuiforge:if sharing
        pendingShares.clear()
// gpuiforge:endif
        permissions.close()
        networkMonitor?.close()
        networkMonitor = null
// gpuiforge:if files
        files.close()
// gpuiforge:endif
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
                fullscreen = false
                fullscreenChanged?.accept(false)
                fullscreenChanged = null
                systemBarAppearance = SystemBarAppearance()
                systemBarAppearanceChanged?.accept(systemBarAppearance)
                systemBarAppearanceChanged = null
                systemBarAppearancePosted = false
                pictureInPicture = false
// gpuiforge:if media
                pictureInPictureHost = null
                pictureInPictureSource = null
                pictureInPictureSourcePosted = false
// gpuiforge:endif
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
        @JvmStatic private external fun nativeAccessibilityNode(id: Long, host: android.view.View, node: Int, focus: Boolean): android.view.accessibility.AccessibilityNodeInfo?
        @JvmStatic private external fun nativeAccessibilityAction(id: Long, host: android.view.View, node: Int, action: Int, arguments: android.os.Bundle?): Boolean
        @JvmStatic private external fun nativeAccessibilityHover(id: Long, host: android.view.View, action: Int, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativeAccessibilityReset(id: Long)
        @JvmStatic private external fun nativeAttach(id: Long, surface: Surface, width: Int, height: Int, density: Float)
        @JvmStatic private external fun nativeDetach(id: Long)
        @JvmStatic private external fun nativeFrame(id: Long): Boolean
        @JvmStatic private external fun nativeInputState(id: Long): TextInputState?
        @JvmStatic private external fun nativeAutofill(id: Long, field: String, value: String)
        @JvmStatic private external fun nativeInputIndex(id: Long, epoch: Long, x: Float, y: Float): Int
        @JvmStatic private external fun nativeScrollInput(id: Long, epoch: Long, dx: Float, dy: Float): Boolean
        @JvmStatic private external fun nativeEdit(id: Long, epoch: Long, operation: Int, text: String, a: Int, b: Int, cursor: Int): Boolean
        @JvmStatic private external fun nativeKey(id: Long, name: String, modifiers: Int, down: Boolean): Boolean
        @JvmStatic private external fun nativeInputAction(id: Long, epoch: Long, action: Int): Boolean
        @JvmStatic private external fun nativeLifecycle(id: Long, phase: Int)
        @JvmStatic private external fun nativeTrimMemory(id: Long, level: Int)
        @JvmStatic private external fun nativeThermalStateChanged(id: Long, status: Int)
        @JvmStatic private external fun nativeFontSizeChanged(id: Long)
        @JvmStatic private external fun nativeReducedMotionChanged(id: Long, reduced: Boolean)
        @JvmStatic private external fun nativePictureInPictureChanged(id: Long, enabled: Boolean)
        @JvmStatic private external fun nativePictureInPictureResult(id: Long, error: String?)
        @JvmStatic private external fun nativeFocus(id: Long, active: Boolean)
        @JvmStatic private external fun nativeAppearance(id: Long, dark: Boolean)
        @JvmStatic private external fun nativeBack(id: Long): Boolean
        @JvmStatic private external fun nativeBackGesture(id: Long, phase: Int, progress: Float, edge: Int)
        @JvmStatic private external fun nativeTouch(id: Long, pointer: Int, phase: Int, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativePinch(id: Long, phase: Int, x: Float, y: Float, delta: Float)
        @JvmStatic private external fun nativeLongPress(id: Long, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativeTap(id: Long, x: Float, y: Float)
        @JvmStatic private external fun nativeOpenUrl(id: Long, url: String)
        @JvmStatic private external fun nativeReceiveShare(id: Long, text: String?, mime: String?, documents: Array<out Any>, error: String?)
        @JvmStatic private external fun nativeFocusTextInput(id: Long, x: Float, y: Float): Boolean
        @JvmStatic private external fun nativeScroll(id: Long, phase: Int, x: Float, y: Float, dx: Float, dy: Float)
        @JvmStatic private external fun nativeMouse(id: Long, kind: Int, x: Float, y: Float,
            button: Int, pressed: Int, clicks: Int, modifiers: Int, dx: Float, dy: Float)
        @JvmStatic private external fun nativeRunTask(id: Long, token: Long)
        @JvmStatic private external fun nativeClose(id: Long)
        @JvmStatic private external fun nativePermissionResult(id: Long, token: Long, status: Int)
        @JvmStatic private external fun nativeNetworkChanged(id: Long, token: Long, status: Int)
        @JvmStatic private external fun nativeSettingsResult(id: Long, token: Long, error: String?)
        @JvmStatic private external fun nativeBackgroundEvent(id: Long, token: String, event: String, error: String)
        @JvmStatic private external fun nativeNotificationEvent(id: Long, event: String): Boolean
        @JvmStatic private external fun nativeMediaCommand(id: Long, event: String)
        @JvmStatic private external fun nativeFileResult(id: Long, token: Long, documents: Array<out Any>?, error: String?)
        @JvmStatic private external fun nativeRedraw(id: Long)
        @JvmStatic private external fun nativeViewport(id: Long, width: Int, height: Int, density: Float, insets: IntArray)
    }
}
