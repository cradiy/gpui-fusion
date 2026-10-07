package dev.gpui.android

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File
import java.io.FileNotFoundException

/** Storage access independent of a View or Activity. Methods run on I/O workers. */
internal class FileStore(context: Context) {
    private val context = context.applicationContext
    private val resolver = this.context.contentResolver

    fun restore(value: String, writable: Boolean): SelectedDocument = DocumentGrants.restore(resolver, value, writable)
    fun release(value: String, writable: Boolean) = DocumentGrants.release(resolver, value, writable)

    fun privatePath(kind: Int): String = when (kind) {
        0 -> File(context.filesDir, "Data")
        1 -> File(context.filesDir, "Config")
        2 -> context.cacheDir
        3 -> context.noBackupFilesDir
        else -> throw IllegalArgumentException("Unknown private location")
    }.absolutePath

    fun collectionsSupported(): Boolean = Build.VERSION.SDK_INT >= 29

    fun create(kind: Int, name: String, mime: String): SelectedDocument {
        check(collectionsSupported()) { "Public collections require Android 10 or later" }
        require(name.isNotBlank() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\') && !name.contains('\u0000')) { "Invalid filename" }
        require(mime.matches(Regex("[^/\\s]+/[^/\\s]+")) && !mime.contains('*')) { "A concrete MIME type is required" }
        val (collection, directory) = when (kind) {
            0 -> MediaStore.Downloads.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_DOWNLOADS
            1 -> { require(mime.startsWith("image/")); MediaStore.Images.Media.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_PICTURES }
            2 -> { require(mime.startsWith("audio/")); MediaStore.Audio.Media.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_MUSIC }
            3 -> { require(mime.startsWith("video/")); MediaStore.Video.Media.EXTERNAL_CONTENT_URI to Environment.DIRECTORY_MOVIES }
            else -> throw IllegalArgumentException("Unknown public location")
        }
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, name)
            put(MediaStore.MediaColumns.MIME_TYPE, mime)
            put(MediaStore.MediaColumns.RELATIVE_PATH, directory)
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val uri = resolver.insert(collection, values) ?: throw FileNotFoundException("Unable to create file")
        return SelectedDocument(resolver, uri, true, true)
    }
}
