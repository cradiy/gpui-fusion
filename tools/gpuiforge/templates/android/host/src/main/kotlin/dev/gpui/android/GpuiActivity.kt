package dev.gpui.android

import android.app.Activity
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.TextView
import kotlin.math.roundToInt

/** Full-page host retaining the Rust application across configuration changes. */
abstract class GpuiActivity : Activity() {
    private lateinit var session: GpuiSession
    protected abstract fun nativeLibraryName(): String

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        System.loadLibrary(nativeLibraryName())
        session = lastNonConfigurationInstance as? GpuiSession ?: GpuiSession()
        session.setOnCloseRequested { finish() }
        session.setOnError { error ->
            Log.e("GPUI", "Unable to display GPUI", error)
            val message = TextView(this)
            val padding = (24 * resources.displayMetrics.density).roundToInt()
            message.setPadding(padding, padding * 3, padding, padding)
            message.text = "Unable to start GPUI\n\n${error.message}"
            setContentView(message)
        }
        val content = FrameLayout(this)
        content.addView(GpuiView(this, session), FrameLayout.LayoutParams(-1, -1))
        content.setOnApplyWindowInsetsListener { view, insets ->
            if (Build.VERSION.SDK_INT >= 30) {
                val safe = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
                val keyboard = insets.getInsets(WindowInsets.Type.ime())
                view.setPadding(safe.left, safe.top, safe.right, maxOf(safe.bottom, keyboard.bottom))
            } else {
                @Suppress("DEPRECATION")
                view.setPadding(insets.systemWindowInsetLeft, insets.systemWindowInsetTop,
                    insets.systemWindowInsetRight, insets.systemWindowInsetBottom)
            }
            insets
        }
        setContentView(content)
        content.requestApplyInsets()
    }

    override fun onRetainNonConfigurationInstance(): Any? = if (session.isClosed()) null else session
    override fun onStart() { super.onStart(); session.setLifecycle(GpuiSession.FOREGROUND) }
    override fun onResume() { super.onResume(); session.setLifecycle(GpuiSession.ACTIVE) }
    override fun onPause() { session.setLifecycle(GpuiSession.INACTIVE); super.onPause() }
    override fun onStop() { session.setLifecycle(GpuiSession.BACKGROUND); super.onStop() }
    override fun onDestroy() {
        session.setOnCloseRequested(null)
        session.setOnError(null)
        if (!isChangingConfigurations) session.close()
        super.onDestroy()
    }
}
