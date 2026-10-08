package dev.gpui.android

import android.app.Activity
import android.content.Intent
import android.net.Uri
import java.lang.ref.WeakReference

internal class FilePickerHost(private val deliver: (Long, Array<out Any>?, String?) -> Unit) {
    private var owner = WeakReference<Activity>(null)
    private data class Request(val token: Long, val code: Int, val multiple: Boolean, val writable: Boolean, val directory: Boolean)
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

    fun request(token: Long, multiple: Boolean, writable: Boolean, active: Boolean, mimeTypes: Array<String>) {
        launch(token, multiple, writable, active, Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            type = mimeTypes.singleOrNull() ?: "*/*"
            if (mimeTypes.size > 1) putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes)
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, multiple)
        })
    }

    fun create(token: Long, name: String, mime: String, active: Boolean) {
        if (name.isBlank() || name.contains('/') || name.contains('\\') || name.contains('\u0000') ||
            !mime.matches(Regex("[^/\\s]+/[^/\\s]+")) || mime.contains('*')) {
            deliver(token, null, "Save requires a filename and a concrete MIME type"); return
        }
        launch(token, false, true, active, Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            type = mime
            putExtra(Intent.EXTRA_TITLE, name)
        })
    }

    fun directory(token: Long, active: Boolean) {
        launch(token, false, true, active, Intent(Intent.ACTION_OPEN_DOCUMENT_TREE))
    }

    @Suppress("DEPRECATION")
    private fun launch(token: Long, multiple: Boolean, writable: Boolean, active: Boolean, intent: Intent) {
        val activity = owner.get()?.takeUnless { it.isFinishing || it.isDestroyed }
        if (!active || activity == null) { deliver(token, null, "File selection requires an active Activity"); return }
        if (pending != null) { deliver(token, null, "Another file selection is pending"); return }
        if (nextCode > 0xbfff) { deliver(token, null, "File request codes exhausted"); return }
        val request = Request(token, nextCode++, multiple, writable, intent.action == Intent.ACTION_OPEN_DOCUMENT_TREE)
        pending = request
        try {
            activity.startActivityForResult(intent.apply {
                if (!request.directory) addCategory(Intent.CATEGORY_OPENABLE)
                else addFlags(Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                if (writable) addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
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
            require(!request.writable || ((data?.flags ?: 0) and Intent.FLAG_GRANT_WRITE_URI_PERMISSION) != 0) {
                "The document provider did not grant write access"
            }
            val resolver = activity.applicationContext.contentResolver
            val flags = data?.flags ?: 0
            val persistent = if (flags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION != 0) {
                flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            } else 0
            if (request.directory) {
                val uri = uris.single()
                require(android.provider.DocumentsContract.isTreeUri(uri)) { "Picker did not return a directory tree" }
                require(flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0) { "Provider did not grant read access" }
                finish(arrayOf(SelectedDirectory(resolver, uri, persistent)), null)
                return true
            }
            finish(uris.map { SelectedDocument(resolver, it, request.writable, persistableFlags = persistent) }.toTypedArray(), null)
        } catch (error: RuntimeException) { finish(null, error.message ?: "Invalid file selection") }
        return true
    }

    fun close() { pending = null; owner.clear() }

    private fun finish(files: Array<out Any>?, error: String?) {
        val request = pending ?: return
        pending = null
        deliver(request.token, files, error)
    }

    companion object { private var nextCode = 0x8000 }
}
