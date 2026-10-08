package dev.gpui.android

/** Icon and text colors; backgrounds are drawn by the application. */
enum class SystemBarStyle { AUTOMATIC, LIGHT, DARK }

data class SystemBarAppearance(
    val status: SystemBarStyle = SystemBarStyle.AUTOMATIC,
    val navigation: SystemBarStyle = SystemBarStyle.AUTOMATIC,
)
