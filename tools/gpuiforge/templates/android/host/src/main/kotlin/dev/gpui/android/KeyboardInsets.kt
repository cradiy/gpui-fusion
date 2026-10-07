package dev.gpui.android

import android.annotation.TargetApi
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsAnimation

@TargetApi(30)
internal class KeyboardInsets(
    private val container: View,
    private val gpui: GpuiView,
    private val hostInsets: Boolean,
    private val visibilityChanged: (Boolean) -> Unit,
) : WindowInsetsAnimation.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
    private val animations = mutableSetOf<WindowInsetsAnimation>()
    private var finalInsets: WindowInsets? = null

    init {
        container.setWindowInsetsAnimationCallback(this)
        container.setOnApplyWindowInsetsListener { view, insets ->
            finalInsets = insets
            val safe = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            if (hostInsets) view.setPadding(safe.left, safe.top, safe.right, safe.bottom)
            visibilityChanged(insets.isVisible(WindowInsets.Type.ime()))
            if (animations.isEmpty()) apply(insets)
            insets
        }
    }

    private fun apply(insets: WindowInsets) {
        val safe = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
        val keyboard = insets.getInsets(WindowInsets.Type.ime())
        val bottom = if (hostInsets) (keyboard.bottom - container.paddingBottom).coerceAtLeast(0) else 0
        gpui.setWindowInsets(GpuiWindowInsets(
            safeArea = EdgeInsets(safe.left, safe.top, safe.right, safe.bottom),
            ime = EdgeInsets(keyboard.left, keyboard.top, keyboard.right, keyboard.bottom),
            consumed = EdgeInsets(container.paddingLeft, container.paddingTop,
                container.paddingRight, container.paddingBottom + bottom),
        ), bottom)
    }

    override fun onPrepare(animation: WindowInsetsAnimation) {
        if (animation.typeMask and WindowInsets.Type.ime() != 0) animations.add(animation)
    }

    override fun onProgress(insets: WindowInsets, runningAnimations: MutableList<WindowInsetsAnimation>): WindowInsets {
        animations.retainAll(runningAnimations.toSet())
        apply(insets)
        return insets
    }

    override fun onEnd(animation: WindowInsetsAnimation) {
        animations.remove(animation)
        if (animations.isEmpty()) finalInsets?.let(::apply)
    }
}
