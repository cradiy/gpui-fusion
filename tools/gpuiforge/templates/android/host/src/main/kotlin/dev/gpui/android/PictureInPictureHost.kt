package dev.gpui.android

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.pm.PackageManager
import android.graphics.Matrix
import android.graphics.Rect
import android.graphics.RectF
import android.os.Build
import android.util.Log
import android.util.Rational
import android.view.View

private const val PIP_LOG_TAG = "GPUI"

internal class PictureInPictureHost(val activity: Activity) {
    private var lastSource: Rect? = null

    fun supported(): Boolean = !activity.isFinishing && !activity.isDestroyed &&
        activity.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    fun enter(width: Int, height: Int) {
        check(supported()) { "Picture-in-picture is unavailable on this device or host" }
        require(width > 0 && height > 0) { "Picture-in-picture aspect ratio must be positive" }
        if (activity.isInPictureInPictureMode) return
        val params = PictureInPictureParams.Builder().setAspectRatio(Rational(width, height))
            .setSourceRectHint(lastSource).build()
        check(activity.enterPictureInPictureMode(params)) { "The system rejected picture-in-picture" }
    }

    fun updateSource(view: View?, bounds: RectF?) {
        if (!supported() || activity.isInPictureInPictureMode) return
        val source = windowBounds(view, bounds) ?: Rect(0, 0,
            activity.window.decorView.width, activity.window.decorView.height)
        if (source.isEmpty || source == lastSource) return
        try {
            activity.setPictureInPictureParams(PictureInPictureParams.Builder()
                .setSourceRectHint(source).build())
            lastSource = source
            if (Log.isLoggable(PIP_LOG_TAG, Log.DEBUG)) Log.d(PIP_LOG_TAG, "PiP source bounds: $source")
        } catch (error: RuntimeException) {
            Log.w(PIP_LOG_TAG, "Unable to update picture-in-picture source bounds", error)
        }
    }

    private fun windowBounds(view: View?, bounds: RectF?): Rect? {
        if (view == null || bounds == null || !view.isAttachedToWindow || !view.isShown) return null
        val density = view.resources.displayMetrics.density
        val local = RectF(bounds.left * density, bounds.top * density,
            bounds.right * density, bounds.bottom * density)
        val visible = Rect()
        if (!view.getLocalVisibleRect(visible) || !local.intersect(RectF(visible))) return null
        val matrix = Matrix()
        val location = IntArray(2)
        if (Build.VERSION.SDK_INT >= 29) {
            view.transformMatrixToGlobal(matrix)
            val screen = IntArray(2)
            view.rootView.getLocationOnScreen(screen)
            view.rootView.getLocationInWindow(location)
            matrix.postTranslate((location[0] - screen[0]).toFloat(), (location[1] - screen[1]).toFloat())
        } else {
            view.getLocationInWindow(location)
            matrix.setTranslate(location[0].toFloat(), location[1].toFloat())
        }
        matrix.mapRect(local)
        return Rect().also { local.roundOut(it) }.takeUnless { it.isEmpty }
    }
}
