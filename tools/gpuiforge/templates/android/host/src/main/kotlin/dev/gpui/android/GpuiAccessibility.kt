package dev.gpui.android

import android.graphics.Rect
import android.os.Bundle
import android.view.MotionEvent
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider

internal class GpuiAccessibility(private val view: GpuiView, private val session: GpuiSession) {
    private val manager = view.context.getSystemService(AccessibilityManager::class.java)
    private val stateListener = AccessibilityManager.AccessibilityStateChangeListener { enabled ->
        if (enabled) invalidate() else session.resetAccessibility()
    }

    val provider = object : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? =
            session.accessibilityNode(view, virtualViewId, false)?.also {
                configureNode(it, virtualViewId == HOST_VIEW_ID)
            }

        override fun findFocus(focus: Int): AccessibilityNodeInfo? =
            session.accessibilityNode(view, focus, true)?.also { configureNode(it, false) }

        override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean {
            val node = createAccessibilityNodeInfo(virtualViewId) ?: return false
            if (action != AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS &&
                (!node.isVisibleToUser || !node.isEnabled)) return false
            return session.accessibilityAction(view, virtualViewId, action, arguments)
        }
    }

    private fun configureNode(node: AccessibilityNodeInfo, root: Boolean) {
        node.isClickable = node.actionList.any { it.id == AccessibilityNodeInfo.ACTION_CLICK }
        val visible = Rect()
        val shown = view.isShown && view.getGlobalVisibleRect(visible)
        val bounds = Rect()
        if (root) bounds.set(visible) else node.getBoundsInScreen(bounds)
        node.isVisibleToUser = shown && bounds.intersect(visible)
        if (!node.isVisibleToUser) bounds.setEmpty()
        node.setBoundsInScreen(bounds)
    }

    fun attach() { manager.addAccessibilityStateChangeListener(stateListener) }

    fun detach() { manager.removeAccessibilityStateChangeListener(stateListener) }

    fun invalidate() {
        if (manager.isEnabled && view.isAttachedToWindow) {
            view.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
        }
    }

    fun hover(event: MotionEvent): Boolean =
        manager.isEnabled && manager.isTouchExplorationEnabled &&
            session.accessibilityHover(view, event.actionMasked, event.x, event.y)
}
