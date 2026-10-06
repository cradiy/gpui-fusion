package dev.gpui.android

import android.app.Activity
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.WindowManager
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher
import android.widget.FrameLayout
import android.widget.TextView
import kotlin.math.roundToInt

/** Full-page host retaining the Rust application across configuration changes. */
abstract class GpuiActivity : Activity() {
    private lateinit var session: GpuiSession
    private var backEnabled = false
    private var resumed = false
    private var imeVisible = false
    private var backRegistration: AutoCloseable? = null
    protected abstract fun nativeLibraryName(): String

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        if (Build.VERSION.SDK_INT >= 30) {
            window.setDecorFitsSystemWindows(false)
            window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
        } else {
            window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        }
        session = lastNonConfigurationInstance as? GpuiSession ?: GpuiSession()
        session.attachPermissionHost(this)
        session.setOnBackEnabledChanged { enabled ->
            backEnabled = enabled
            updateBackRegistration()
        }
        session.setOnCloseRequested { finish() }
        session.setOnError { error ->
            Log.e("GPUI", "Unable to display GPUI", error)
            val message = TextView(this)
            val padding = (24 * resources.displayMetrics.density).roundToInt()
            message.setPadding(padding, padding * 3, padding, padding)
            message.text = "Unable to start GPUI\n\n${error.message}"
            setContentView(message)
        }
        try {
            System.loadLibrary(nativeLibraryName())
        } catch (error: LinkageError) {
            session.fail(IllegalStateException(
                "Unable to load the GPUI native library. Rebuild GPUiForge and regenerate the Android host " +
                    "with matching GPUI sources.\n\n${error.message}", error))
            return
        }
        val content = FrameLayout(this)
        val gpui = GpuiView(this, session)
        content.addView(gpui, FrameLayout.LayoutParams(-1, -1))
        if (Build.VERSION.SDK_INT >= 30) {
            KeyboardInsets(content, gpui) { visible ->
                imeVisible = visible
                updateBackRegistration()
            }
        } else {
            content.setOnApplyWindowInsetsListener { view, insets ->
                @Suppress("DEPRECATION")
                view.setPadding(insets.systemWindowInsetLeft, insets.systemWindowInsetTop,
                    insets.systemWindowInsetRight, insets.systemWindowInsetBottom)
                insets
            }
        }
        setContentView(content)
        content.requestApplyInsets()
    }

    override fun onRetainNonConfigurationInstance(): Any? = if (session.isClosed()) null else session
    override fun onStart() { super.onStart(); session.setLifecycle(GpuiSession.FOREGROUND) }
    override fun onResume() {
        super.onResume()
        session.setLifecycle(GpuiSession.ACTIVE)
        resumed = true
        updateBackRegistration()
    }
    override fun onPause() {
        resumed = false
        updateBackRegistration()
        session.setLifecycle(GpuiSession.INACTIVE)
        super.onPause()
    }
    override fun onStop() { session.setLifecycle(GpuiSession.BACKGROUND); super.onStop() }
    override fun onDestroy() {
        session.detachPermissionHost(this)
        backRegistration?.close()
        backRegistration = null
        session.setOnBackEnabledChanged(null)
        session.setOnCloseRequested(null)
        session.setOnError(null)
        if (!isChangingConfigurations) session.close()
        super.onDestroy()
    }

    @Suppress("DEPRECATION", "OVERRIDE_DEPRECATION")
    override fun onBackPressed() {
        if (!session.handleSystemBack()) super.onBackPressed()
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        if (!session.onRequestPermissionsResult(this, requestCode, permissions, grantResults)) {
            super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        }
    }

    @Suppress("DEPRECATION")
    private fun updateBackRegistration() {
        if (Build.VERSION.SDK_INT < 33) return
        val enabled = resumed && backEnabled && !imeVisible
        if (enabled && backRegistration == null) {
            backRegistration = BackApi33.register(this) { onBackPressed() }
        } else if (!enabled) {
            backRegistration?.close()
            backRegistration = null
        }
    }

    private object BackApi33 {
        fun register(activity: Activity, action: () -> Unit): AutoCloseable {
            val dispatcher = activity.onBackInvokedDispatcher
            val callback = OnBackInvokedCallback { action() }
            dispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, callback)
            return AutoCloseable { dispatcher.unregisterOnBackInvokedCallback(callback) }
        }
    }
}
