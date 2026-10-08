package dev.gpui.android

import android.content.ClipData
import android.content.ContentResolver
import java.io.ByteArrayOutputStream

/** A main-thread clipboard snapshot whose URI data is read on an I/O worker. */
internal class ClipboardSnapshot(private val resolver: ContentResolver, private val clip: ClipData) {
    fun count(): Int = clip.itemCount
    fun text(index: Int): String? = clip.getItemAt(index).text?.toString()
    fun imageType(index: Int): String? {
        val uri = clip.getItemAt(index).uri ?: return null
        if (uri.scheme != "content") return null
        return resolver.getType(uri)?.takeIf { it.startsWith("image/") }
    }
    fun image(index: Int, limit: Int): ByteArray {
        val uri = requireNotNull(clip.getItemAt(index).uri)
        require(uri.scheme == "content")
        return requireNotNull(resolver.openInputStream(uri)) { "Clipboard image is unavailable" }.use { input ->
            val output = ByteArrayOutputStream()
            val buffer = ByteArray(16384)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                require(count <= limit - output.size()) { "Clipboard images exceed 32 MiB" }
                output.write(buffer, 0, count)
            }
            output.toByteArray()
        }
    }
}
