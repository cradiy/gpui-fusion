package dev.gpui.android

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.pm.PackageManager
import android.util.Rational

internal class PictureInPictureHost(val activity: Activity) {
    fun supported(): Boolean = !activity.isFinishing && !activity.isDestroyed &&
        activity.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    fun enter(width: Int, height: Int) {
        check(supported()) { "Picture-in-picture is unavailable on this device or host" }
        require(width > 0 && height > 0) { "Picture-in-picture aspect ratio must be positive" }
        if (activity.isInPictureInPictureMode) return
        val params = PictureInPictureParams.Builder().setAspectRatio(Rational(width, height)).build()
        check(activity.enterPictureInPictureMode(params)) { "The system rejected picture-in-picture" }
    }
}
