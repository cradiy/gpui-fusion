package dev.gpui.android

import android.content.ContentResolver
import android.content.ContentValues
import android.net.Uri
import android.provider.MediaStore
import android.provider.OpenableColumns
import java.io.FileNotFoundException
import java.io.OutputStream

/** Owns no Activity. Metadata and descriptor access run on Rust background workers. */
internal class SelectedDocument(private val resolver: ContentResolver, private val uri: Uri, private val writable: Boolean, private var pending: Boolean = false) {
    private var outputOpen = false
    private var discarded = false
    fun canWrite(): Boolean = writable
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
