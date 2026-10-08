package dev.gpui.android

import android.content.ClipData
import android.content.Intent

internal object ShareIntent {
    fun create(text: String?, title: String?, files: Array<Intent>): Intent {
        require(!text.isNullOrEmpty() || files.isNotEmpty()) { "Sharing requires text or files" }
        val uris = files.map {
            requireNotNull(it.data).also { uri ->
                require(uri.scheme == "content" && !uri.authority.isNullOrEmpty()) { "A content URI is required" }
            }
        }
        val types = files.map { it.type ?: "application/octet-stream" }.distinct()
        val mime = when {
            types.isEmpty() -> "text/plain"
            types.size == 1 -> types[0]
            types.map { it.substringBefore('/') }.distinct().size == 1 -> "${types[0].substringBefore('/')}/*"
            else -> "*/*"
        }
        val send = Intent(if (uris.size > 1) Intent.ACTION_SEND_MULTIPLE else Intent.ACTION_SEND).apply {
            type = mime
            if (text != null) putExtra(Intent.EXTRA_TEXT, text)
            if (uris.size == 1) putExtra(Intent.EXTRA_STREAM, uris[0])
            if (uris.size > 1) putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
            if (uris.isNotEmpty()) {
                clipData = ClipData("", types.toTypedArray(), ClipData.Item(uris[0])).apply {
                    uris.drop(1).forEach { addItem(ClipData.Item(it)) }
                }
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
        }
        return Intent.createChooser(send, title)
    }
}
