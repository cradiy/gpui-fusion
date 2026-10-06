package dev.gpui.android

internal class TextInputState(
    val epoch: Long,
    val text: String?,
    val offset: Int,
    val anchor: Int,
    val head: Int,
    val composingStart: Int,
    val composingEnd: Int,
    val hit: Boolean,
    val multiline: Boolean,
    val sensitive: Boolean,
)
