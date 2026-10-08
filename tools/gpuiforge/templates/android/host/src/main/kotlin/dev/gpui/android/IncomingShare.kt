package dev.gpui.android

import android.content.Intent
import android.net.Uri
import androidx.core.content.IntentCompat

/** Parses share envelopes without querying providers or retaining an Activity. */
internal data class IncomingShare(
    val text: String? = null,
    val mime: String? = null,
    val uris: List<Uri> = emptyList(),
    val error: String? = null,
) {
    companion object {
        fun parse(intent: Intent): IncomingShare = try {
            val uris = LinkedHashSet<Uri>()
            if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
                val streams = IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
                require(!intent.hasExtra(Intent.EXTRA_STREAM) || streams != null) { "Invalid shared file list" }
                streams?.let { uris.addAll(it) }
            } else {
                val stream = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
                require(!intent.hasExtra(Intent.EXTRA_STREAM) || stream != null) { "Invalid shared file" }
                stream?.let { uris.add(it) }
            }
            val clip = intent.clipData
            if (clip != null) {
                for (index in 0 until clip.itemCount) clip.getItemAt(index).uri?.let { uris.add(it) }
            }
            require(uris.all { it.scheme == "content" && !it.authority.isNullOrEmpty() }) {
                "Shared files must use content URIs"
            }
            val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
            require(!text.isNullOrEmpty() || uris.isNotEmpty()) { "Shared content is empty" }
            IncomingShare(text, intent.type, uris.toList())
        } catch (error: RuntimeException) {
            IncomingShare(error = error.toString())
        }
    }
}
