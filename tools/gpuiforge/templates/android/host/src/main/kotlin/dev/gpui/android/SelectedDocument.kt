package dev.gpui.android

import android.content.ContentResolver
import android.content.ContentValues
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.provider.DocumentsContract
import java.io.FileNotFoundException
import java.io.OutputStream

/** Owns no Activity. Metadata and descriptor access run on Rust background workers. */
internal class SelectedDocument(private val resolver: ContentResolver, private val uri: Uri, private val writable: Boolean, private var pending: Boolean = false, private val persistableFlags: Int = 0, private val ownedCollection: Boolean = pending) {
    private var outputOpen = false
    private var discarded = false
    fun canWrite(): Boolean = writable
    @Synchronized fun canDelete(): Boolean = supports(DocumentsContract.Document.FLAG_SUPPORTS_DELETE)
    @Synchronized fun canRename(): Boolean = !pending && supports(DocumentsContract.Document.FLAG_SUPPORTS_RENAME)
    private fun supports(flag: Int): Boolean {
        if (!writable || discarded || outputOpen) return false
        if (ownedCollection) return true
        // Only document URIs support DocumentsContract operations; shared content URIs do not.
        try { DocumentsContract.getDocumentId(uri) } catch (_: IllegalArgumentException) { return false }
        val columns = arrayOf(DocumentsContract.Document.COLUMN_FLAGS, DocumentsContract.Document.COLUMN_MIME_TYPE)
        return resolver.query(uri, columns, null, null, null)?.use {
            it.moveToFirst() && it.getString(1) != DocumentsContract.Document.MIME_TYPE_DIR &&
                it.getLong(0) and flag.toLong() != 0L
        } ?: false
    }
    @Synchronized fun rename(newName: String): SelectedDocument {
        require(newName.isNotEmpty() && newName != "." && newName != ".." && !newName.endsWith('.') && !newName.endsWith(' ') && newName.none { it == '/' || it == '\\' || it == ':' || it == '\u0000' }) { "Invalid filename" }
        check(canRename()) { "File does not support renaming or has an active writer" }
        val renamed = if (ownedCollection) {
            val values = ContentValues().apply { put(MediaStore.MediaColumns.DISPLAY_NAME, newName) }
            check(resolver.update(uri, values, null, null) == 1) { "Provider did not rename the file" }
            uri
        } else {
            DocumentsContract.renameDocument(resolver, uri, newName)
                ?: throw FileNotFoundException("Provider did not return the renamed file")
        }
        return SelectedDocument(resolver, renamed, writable, persistableFlags = persistableFlags, ownedCollection = ownedCollection)
    }
    @Synchronized fun delete() {
        // An aborted pending MediaStore output has already been removed.
        if (discarded) return
        check(canDelete()) { "File does not support deletion or has an active writer" }
        val removed = if (ownedCollection) resolver.delete(uri, null, null) == 1
            else DocumentsContract.deleteDocument(resolver, uri)
        check(removed) { "Provider did not delete the file" }
        pending = false
        discarded = true
    }
    fun url(): String = uri.toString()
    @Synchronized fun viewIntent(): Intent {
        check(!pending && !discarded && !outputOpen) { "Finish writing before opening the file" }
        require(uri.scheme == "content" && !uri.authority.isNullOrEmpty()) { "A content URI is required" }
        val mime = mimeType() ?: "application/octet-stream"
        return Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mime)
            clipData = ClipData("", arrayOf(mime), ClipData.Item(uri))
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    }
    @Synchronized fun persist(): String {
        check(!pending && !discarded) { "File is not published" }
        return DocumentGrants.persist(resolver, uri, writable, persistableFlags)
    }
    fun mimeType(): String? = resolver.getType(uri)
    fun byteLength(): Long {
        resolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst() && !cursor.isNull(0)) return cursor.getLong(0)
        }
        return -1L
    }
    fun displayName(): String {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst() && !cursor.isNull(0)) return cursor.getString(0)
        }
        return uri.lastPathSegment ?: "document"
    }

    fun openRead(): Int = (resolver.openFileDescriptor(uri, "r")
        ?: throw FileNotFoundException("Document provider returned no descriptor")).use { it.detachFd() }

    @Synchronized fun openWrite(): DocumentOutput {
        check(writable) { "File handle is read-only" }
        check(!discarded) { "Pending file was discarded" }
        check(!outputOpen) { "A writer is already open for this file" }
        val output = resolver.openOutputStream(uri, "wt")
            ?: throw FileNotFoundException("Document provider returned no output stream")
        outputOpen = true
        return DocumentOutput(this, output)
    }

    @Synchronized fun publish() {
        if (pending) {
            val values = ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }
            check(resolver.update(uri, values, null, null) == 1) { "Unable to publish file" }
            pending = false
        }
    }
    @Synchronized fun discardPending() {
        if (pending) {
            resolver.delete(uri, null, null)
            pending = false
            discarded = true
        }
    }
    @Synchronized fun releaseWriter() { outputOpen = false }
}

internal class DocumentOutput(private val file: SelectedDocument, private var output: OutputStream?) {
    private var released = false
    fun write(bytes: ByteArray) { checkNotNull(output) { "Writer closed" }.write(bytes) }
    fun flush() { checkNotNull(output) { "Writer closed" }.flush() }
    fun finish() {
        try {
            checkNotNull(output) { "Writer closed" }.flush()
            closeOutput()
            file.publish()
        } catch (error: Exception) {
            try { abort() } catch (cleanup: Exception) { error.addSuppressed(cleanup) }
            throw error
        } finally { releaseWriter() }
    }
    fun abort() {
        try { closeOutput() } finally {
            try { file.discardPending() } finally { releaseWriter() }
        }
    }
    private fun closeOutput() {
        val stream = output
        output = null
        stream?.close()
    }
    private fun releaseWriter() {
        if (!released) { released = true; file.releaseWriter() }
    }
}
