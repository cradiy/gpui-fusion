package dev.gpui.android

/** Nonnegative distances from the host window edges, in physical pixels. */
data class EdgeInsets(val left: Int = 0, val top: Int = 0, val right: Int = 0, val bottom: Int = 0) {
    init { require(left >= 0 && top >= 0 && right >= 0 && bottom >= 0) }
}

/** System occlusion and space already excluded from the GPUI viewport by the host. */
data class GpuiWindowInsets(
    val safeArea: EdgeInsets = EdgeInsets(),
    val ime: EdgeInsets = EdgeInsets(),
    val consumed: EdgeInsets = EdgeInsets(),
) {
    internal fun values() = intArrayOf(
        safeArea.left, safeArea.top, safeArea.right, safeArea.bottom,
        ime.left, ime.top, ime.right, ime.bottom,
        consumed.left, consumed.top, consumed.right, consumed.bottom,
    )
}
