package dev.gpui.android

import android.content.ContentValues
import android.content.Context
import android.content.ClipData
import android.content.Intent
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileNotFoundException

/** Storage access independent of a View or Activity. Methods run on I/O workers. */
internal class FileStore(context: Context) {
    private val context = context.applicationContext
    private val resolver = this.context.contentResolver

    fun restore(value: String, writable: Boolean): SelectedDocument = DocumentGrants.restore(resolver, value, writable)
    fun restoreDirectory(value: String): SelectedDirectory = DocumentGrants.restoreDirectory(resolver, value)
    fun release(value: String, writable: Boolean) = DocumentGrants.release(resolver, value, writable)

    fun viewPathIntent(path: String): Intent {
        require(!path.contains('\u0000')) { "Invalid file path" }
        val original = File(path)
        require(original.isAbsolute) { "An absolute file path is required" }
        val file = original.canonicalFile
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.gpui.files", file)
        if (!file.isFile) throw FileNotFoundException("File is missing or is not a regular file")
        if (!file.canRead()) throw SecurityException("File is not readable")
        val mime = resolver.getType(uri) ?: "application/octet-stream"
        return Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mime)
            clipData = ClipData("", arrayOf(mime), ClipData.Item(uri))
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    }

    fun privatePath(kind: Int): String = when (kind) {
        0 -> File(context.filesDir, "Data")
        1 -> File(context.filesDir, "Config")
        2 -> context.cacheDir
        3 -> context.noBackupFilesDir
        else -> throw IllegalArgumentException("Unknown private location")
    }.absolutePath

    fun collectionsSupported(): Boolean = Build.VERSION.SDK_INT >= 29

    fun create(kind: Int, name: String, mime: String, subdirectory: String): SelectedDocument {
        check(collectionsSupported()) { "Public collections require Android 10 or later" }
        require(name.isNotBlank() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\') && !name.contains('\u0000')) { "Invalid filename" }
        require(mime.matches(Regex("[^/\\s]+/[^/\\s]+")) && !mime.contains('*')) { "A concrete MIME type is required" }
        require(subdirectory.isEmpty() || subdirectory.split('/').all { part ->
            part.isNotEmpty() && part != "." && part != ".." &&
                !part.endsWith('.') && !part.endsWith(' ') &&
                !part.contains('\\') && !part.contains(':') && !part.contains('\u0000')
        }) { "Invalid relative directory" }
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
            put(MediaStore.MediaColumns.RELATIVE_PATH,
                if (subdirectory.isEmpty()) directory else "$directory/$subdirectory/")
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val uri = resolver.insert(collection, values) ?: throw FileNotFoundException("Unable to create file")
        return SelectedDocument(resolver, uri, true, true)
    }
}
