package dev.gpui.android

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import java.io.FileNotFoundException
import java.io.IOException

/** Directory operations run on I/O workers; a tree URI is never treated as a filesystem path. */
internal class SelectedDirectory(
    private val resolver: ContentResolver,
    private val tree: Uri,
    private val persistableFlags: Int,
) {
    private val root: Uri get() = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))

    fun validate() {
        require(DocumentsContract.isTreeUri(tree)) { "Invalid directory URI" }
        val cursor = resolver.query(root, arrayOf(Document.COLUMN_MIME_TYPE, Document.COLUMN_FLAGS), null, null, null)
            ?: throw FileNotFoundException("Directory unavailable")
        cursor.use {
            if (!it.moveToFirst() || it.getString(0) != Document.MIME_TYPE_DIR) throw FileNotFoundException("Directory unavailable")
            if (it.getLong(1) and Document.FLAG_DIR_SUPPORTS_CREATE.toLong() == 0L) throw UnsupportedOperationException("Directory does not support creating files")
        }
    }

    fun persist(): String = DocumentGrants.persist(resolver, tree, true, persistableFlags)

    private fun child(parent: Uri, name: String): Pair<Uri, String>? {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent))
        val cursor = resolver.query(children, arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE), null, null, null)
            ?: throw IOException("Unable to list directory")
        cursor.use {
            while (it.moveToNext()) {
                if (it.getString(1) == name) return DocumentsContract.buildDocumentUriUsingTree(tree, it.getString(0)) to it.getString(2)
            }
        }
        return null
    }

    @Synchronized fun create(relativePath: String, mime: String): SelectedDocument {
        val parts = relativePath.split('/')
        require(parts.all { it.isNotEmpty() && it != "." && it != ".." && !it.endsWith('.') && !it.endsWith(' ') && !it.contains('\\') && !it.contains(':') && !it.contains('\u0000') }) { "Invalid relative path" }
        require(mime.matches(Regex("[^/\\s]+/[^/\\s]+")) && !mime.contains('*') && mime != Document.MIME_TYPE_DIR) { "A concrete file MIME type is required" }
        var parent = root
        for (part in parts.dropLast(1)) {
            val existing = child(parent, part)
            parent = if (existing != null) {
                require(existing.second == Document.MIME_TYPE_DIR) { "Parent path is not a directory" }
                existing.first
            } else {
                DocumentsContract.createDocument(resolver, parent, Document.MIME_TYPE_DIR, part)
                    ?: throw IOException("Unable to create parent directory")
            }
        }
        if (child(parent, parts.last()) != null) throw java.nio.file.FileAlreadyExistsException(relativePath)
        val uri = DocumentsContract.createDocument(resolver, parent, mime, parts.last())
            ?: throw IOException("Unable to create file")
        return SelectedDocument(resolver, uri, true)
    }
}
