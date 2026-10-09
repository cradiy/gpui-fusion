package dev.gpui.android

import android.content.pm.ActivityInfo

/** Activity-wide orientation request; Android may override it on large screens or in multi-window mode. */
enum class ScreenOrientation(val activityOrientation: Int) {
    AUTOMATIC(ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED),
    PORTRAIT(ActivityInfo.SCREEN_ORIENTATION_PORTRAIT),
    LANDSCAPE(ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE),
    REVERSE_PORTRAIT(ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT),
    REVERSE_LANDSCAPE(ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE),
    LOCKED(ActivityInfo.SCREEN_ORIENTATION_LOCKED),
}
