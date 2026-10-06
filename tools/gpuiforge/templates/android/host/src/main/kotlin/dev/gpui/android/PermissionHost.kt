package dev.gpui.android

import android.app.Activity
import android.content.pm.PackageManager
import android.content.pm.PermissionInfo
import java.lang.ref.WeakReference

internal class PermissionHost(private val deliver: (Long, Int) -> Unit) {
    private var owner = WeakReference<Activity>(null)
    private data class Request(val token: Long, val permission: String, val code: Int, var cancelled: Boolean = false)
    private var pending: Request? = null

    fun attach(activity: Activity) {
        check(owner.get() == null || owner.get() === activity) { "Detach the previous permission Activity first" }
        owner = WeakReference(activity)
    }

    fun detach(activity: Activity) {
        if (owner.get() !== activity) return
        finish(-1)
        owner.clear()
    }

    @Suppress("DEPRECATION")
    fun status(permission: String): Int {
        val activity = owner.get()?.takeUnless { it.isFinishing || it.isDestroyed } ?: return -4
        return try {
            val info = activity.packageManager.getPermissionInfo(permission, 0)
            val protection = info.protectionLevel and PermissionInfo.PROTECTION_MASK_BASE
            if (protection != PermissionInfo.PROTECTION_NORMAL && protection != PermissionInfo.PROTECTION_DANGEROUS) return -3
            val declared = activity.packageManager.getPackageInfo(activity.packageName, PackageManager.GET_PERMISSIONS).requestedPermissions
            if (declared?.contains(permission) != true) return -2
            if (activity.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED) 0
            else if (activity.shouldShowRequestPermissionRationale(permission)) 2 else 1
        } catch (_: PackageManager.NameNotFoundException) { -3 }
          catch (_: RuntimeException) { -6 }
    }

    @Suppress("DEPRECATION")
    fun request(permission: String, token: Long, active: Boolean) {
        if (!active) { deliver(token, -4); return }
        if (pending != null) { deliver(token, -5); return }
        val status = status(permission)
        if (status <= 0) { deliver(token, status); return }
        val activity = owner.get() ?: run { deliver(token, -4); return }
        try {
            val info = activity.packageManager.getPermissionInfo(permission, 0)
            if (info.protectionLevel and PermissionInfo.PROTECTION_MASK_BASE != PermissionInfo.PROTECTION_DANGEROUS) {
                deliver(token, -3)
                return
            }
            if (nextCode > 0x7fff) { deliver(token, -6); return }
            val request = Request(token, permission, nextCode++)
            pending = request
            activity.requestPermissions(arrayOf(permission), request.code)
        } catch (_: RuntimeException) {
            if (pending?.token == token) finish(-6) else deliver(token, -6)
        }
          catch (_: PackageManager.NameNotFoundException) { deliver(token, -3) }
    }

    fun result(activity: Activity, code: Int, permissions: Array<out String>, grants: IntArray): Boolean {
        val request = pending ?: return false
        if (owner.get() !== activity || request.code != code) return false
        val status = if (permissions.size != 1 || grants.size != 1 || permissions[0] != request.permission) -1
            else status(request.permission)
        finish(status)
        return true
    }

    fun cancel(token: Long) {
        pending?.takeIf { it.token == token }?.cancelled = true
    }

    fun close() { pending = null; owner.clear() }

    private fun finish(status: Int) {
        val request = pending ?: return
        pending = null
        if (!request.cancelled) deliver(request.token, status)
    }

    companion object {
        // Reserved host request codes; never reuse a code for a stale Activity result.
        private var nextCode = 0x4700
    }
}
