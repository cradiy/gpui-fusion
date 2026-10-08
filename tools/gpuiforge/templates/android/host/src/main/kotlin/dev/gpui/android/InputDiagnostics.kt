package dev.gpui.android

import android.util.Log

private const val INPUT_LOG_TAG = "GPUIInput"

internal inline fun inputDiagnostic(message: () -> String) {
    if (Log.isLoggable(INPUT_LOG_TAG, Log.DEBUG)) Log.d(INPUT_LOG_TAG, message())
}
