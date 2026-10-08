package dev.gpui.android

import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import java.io.FileNotFoundException

internal object DocumentGrants {
    private fun flags(writable: Boolean): Int = Intent.FLAG_GRANT_READ_URI_PERMISSION or
        (if (writable) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)

    private fun uri(value: String): Uri {
        require(value.toByteArray(Charsets.UTF_8).size <= 16384 && !value.contains('\u0000')) { "Invalid document bookmark" }
        return Uri.parse(value).also {
            require(it.scheme == "content" && !it.authority.isNullOrEmpty()) { "Invalid document URI" }
        }
    }

    private fun held(resolver: ContentResolver, uri: Uri): Int {
        val grant = resolver.persistedUriPermissions.firstOrNull { it.uri == uri } ?: return 0
        return (if (grant.isReadPermission) Intent.FLAG_GRANT_READ_URI_PERMISSION else 0) or
            (if (grant.isWritePermission) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)
    }

    @Synchronized fun persist(resolver: ContentResolver, uri: Uri, writable: Boolean, offered: Int): String {
        val required = flags(writable)
        val value = uri.toString()
        uri(value)
        if (held(resolver, uri) and required != required) {
            if (offered and required != required) throw UnsupportedOperationException("Provider did not offer persistent access")
            resolver.takePersistableUriPermission(uri, required)
        }
        return value
    }

    @Synchronized fun restore(resolver: ContentResolver, value: String, writable: Boolean): SelectedDocument {
        val uri = uri(value)
        val required = flags(writable)
        if (held(resolver, uri) and required != required) throw SecurityException("Persistent document access is unavailable")
        (resolver.openFileDescriptor(uri, "r") ?: throw FileNotFoundException("Document unavailable")).close()
        return SelectedDocument(resolver, uri, writable, persistableFlags = required)
    }

    @Synchronized fun restoreDirectory(resolver: ContentResolver, value: String): SelectedDirectory {
        val uri = uri(value)
        require(android.provider.DocumentsContract.isTreeUri(uri)) { "Invalid directory bookmark" }
        val required = flags(true)
        if (held(resolver, uri) and required != required) throw SecurityException("Persistent directory access is unavailable")
        return SelectedDirectory(resolver, uri, required).also { it.validate() }
    }

    @Synchronized fun release(resolver: ContentResolver, value: String, writable: Boolean) {
        val uri = uri(value)
        val release = held(resolver, uri) and flags(writable)
        if (release != 0) resolver.releasePersistableUriPermission(uri, release)
    }
}
