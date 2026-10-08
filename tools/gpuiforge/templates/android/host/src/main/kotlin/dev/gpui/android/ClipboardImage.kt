package dev.gpui.android

import android.content.ClipData
import android.content.Context
import androidx.core.content.FileProvider
import java.io.File
import java.util.UUID

/** Owns an unpublished export; published images remain in the application cache. */
internal class ClipboardImage(context: Context) : AutoCloseable {
    private val context = context.applicationContext
    private var file: File? = null
    private var published = false
    private var mime = ""

    fun prepare(bytes: ByteArray, mime: String, extension: String) {
        check(file == null)
        require(bytes.isNotEmpty() && bytes.size <= 32 * 1024 * 1024)
        require(extension.matches(Regex("[a-z]+")))
        val directory = File(context.cacheDir, "gpui-clipboard")
        check(directory.isDirectory || directory.mkdirs())
        val target = File(directory, "${UUID.randomUUID()}.$extension")
        file = target
        target.writeBytes(bytes)
        this.mime = mime
    }

    fun clip(): ClipData {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.gpui.files", checkNotNull(file))
        return ClipData("", arrayOf(mime), ClipData.Item(uri))
    }

    fun retain() { published = true }

    fun cleanOld() {
        val current = file ?: return
        val cutoff = System.currentTimeMillis() - 24 * 60 * 60 * 1000L
        current.parentFile?.listFiles()?.forEach { candidate ->
            if (candidate != current && candidate.isFile && candidate.lastModified() < cutoff) candidate.delete()
        }
    }

    override fun close() {
        if (!published) file?.delete()
    }
}
