package dev.gpui.android

import android.util.Log

internal inline fun inputDiagnostic(message: () -> String) {
    if (Log.isLoggable("GPUIInput", Log.DEBUG)) Log.d("GPUIInput", message())
}
