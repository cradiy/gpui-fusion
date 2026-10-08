package dev.gpui.android

import android.text.InputType

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
    val purpose: Int,
    val decimal: Boolean,
    val signed: Boolean,
    val action: Int,
    val caretBounds: FloatArray?,
    val editorBounds: FloatArray?,
    val anchorBounds: FloatArray?,
    val headBounds: FloatArray?,
) {
    val inputType: Int
        get() = when {
            sensitive -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            multiline -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            else -> when (purpose) {
                1 -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                2 -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
                3 -> InputType.TYPE_CLASS_PHONE
                4 -> InputType.TYPE_CLASS_NUMBER or
                    (if (decimal) InputType.TYPE_NUMBER_FLAG_DECIMAL else 0) or
                    (if (signed) InputType.TYPE_NUMBER_FLAG_SIGNED else 0)
                else -> InputType.TYPE_CLASS_TEXT
            }
        }
}
