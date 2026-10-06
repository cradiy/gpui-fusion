package dev.gpui.android

import android.app.Activity
import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import java.io.FileNotFoundException
import java.lang.ref.WeakReference

internal class FilePickerHost(private val deliver: (Long, Array<SelectedDocument>?, String?) -> Unit) {
    private var owner = WeakReference<Activity>(null)
    private data class Request(val token: Long, val code: Int, val multiple: Boolean)
    private var pending: Request? = null

    fun attach(activity: Activity) {
        check(owner.get() == null || owner.get() === activity) { "Detach the previous file picker Activity first" }
        owner = WeakReference(activity)
    }

    fun detach(activity: Activity) {
        if (owner.get() !== activity) return
        if (!activity.isChangingConfigurations) finish(null, "File picker host detached")
        owner.clear()
    }

    @Suppress("DEPRECATION")
    fun request(token: Long, multiple: Boolean, active: Boolean) {
        val activity = owner.get()?.takeUnless { it.isFinishing || it.isDestroyed }
        if (!active || activity == null) { deliver(token, null, "File selection requires an active Activity"); return }
        if (pending != null) { deliver(token, null, "Another file selection is pending"); return }
        if (nextCode > 0xbfff) { deliver(token, null, "File request codes exhausted"); return }
        val request = Request(token, nextCode++, multiple)
        pending = request
        try {
            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = "*/*"
                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, multiple)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }, request.code)
        } catch (error: RuntimeException) { finish(null, error.message ?: "Unable to open file picker") }
    }

    fun result(activity: Activity, code: Int, result: Int, data: Intent?): Boolean {
        val request = pending ?: return false
        if (owner.get() !== activity || request.code != code) return false
        if (result == Activity.RESULT_CANCELED) { finish(null, null); return true }
        try {
            check(result == Activity.RESULT_OK) { "File picker failed" }
            val uris = LinkedHashSet<Uri>()
            data?.clipData?.let { clip ->
                for (index in 0 until clip.itemCount) uris.add(requireNotNull(clip.getItemAt(index).uri))
            }
            data?.data?.let { uris.add(it) }
            require(uris.isNotEmpty() && uris.all { it.scheme == "content" }) { "File picker returned no readable document URIs" }
            require(request.multiple || uris.size == 1) { "File picker returned multiple documents for a single selection" }
            val resolver = activity.applicationContext.contentResolver
            finish(uris.map { SelectedDocument(resolver, it) }.toTypedArray(), null)
        } catch (error: RuntimeException) { finish(null, error.message ?: "Invalid file selection") }
        return true
    }

    fun close() { pending = null; owner.clear() }

    private fun finish(files: Array<SelectedDocument>?, error: String?) {
        val request = pending ?: return
        pending = null
        deliver(request.token, files, error)
    }

    companion object { private var nextCode = 0x8000 }
}

/** Owns no Activity. Metadata and descriptor access run on Rust background workers. */
internal class SelectedDocument(private val resolver: ContentResolver, private val uri: Uri) {
    fun displayName(): String {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst() && !cursor.isNull(0)) return cursor.getString(0)
        }
        return uri.lastPathSegment ?: "document"
    }

    fun openRead(): Int = (resolver.openFileDescriptor(uri, "r")
        ?: throw FileNotFoundException("Document provider returned no descriptor")).use { it.detachFd() }
}
