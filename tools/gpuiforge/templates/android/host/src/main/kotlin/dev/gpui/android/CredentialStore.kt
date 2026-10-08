package dev.gpui.android

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.KeyStore
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal class StoredCredential(val username: String, val password: ByteArray) {
    fun clear() { password.fill(0) }
}

/** Application-scoped credential storage. All operations run on background workers. */
internal class CredentialStore(context: Context) {
    private val context = context.applicationContext
    private val directory get() = File(context.noBackupFilesDir, "gpui-credentials")

    fun write(url: String, username: String, password: ByteArray) {
        val user = username.toByteArray(Charsets.UTF_8)
        try {
            require(4L + user.size + password.size <= MAX_PAYLOAD) { "Credential exceeds size limit" }
            locked(true) { root ->
                val id = identifier(url)
                val target = File(root, "$id.bin")
                val plain = ByteBuffer.allocate(4 + user.size + password.size)
                    .putInt(user.size).put(user).put(password).array()
                val encrypted = try {
                    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                    cipher.init(Cipher.ENCRYPT_MODE, key(root, true))
                    cipher.updateAAD(aad(id))
                    check(cipher.iv.size == IV_SIZE) { "Unexpected IV size" }
                    ByteBuffer.allocate(4 + IV_SIZE + plain.size + TAG_SIZE)
                        .putInt(MAGIC).put(cipher.iv).put(cipher.doFinal(plain)).array()
                } finally { plain.fill(0) }
                val temporary = File(root, "$id.tmp")
                try {
                    FileOutputStream(temporary).use { output ->
                        output.write(encrypted)
                        output.fd.sync()
                    }
                    Files.move(temporary.toPath(), target.toPath(),
                        StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING)
                } finally { temporary.delete() }
            }
        } finally { user.fill(0); password.fill(0) }
    }

    fun read(url: String): StoredCredential? = locked(false) { root ->
        val id = identifier(url)
        val file = File(root, "$id.bin")
        if (!file.exists()) return@locked null
        val encoded = Files.newInputStream(file.toPath()).use { input ->
            val bytes = ByteArray(MAX_RECORD + 1)
            var size = 0
            while (size < bytes.size) {
                val count = input.read(bytes, size, bytes.size - size)
                if (count < 0) break
                size += count
            }
            require(size in (4 + IV_SIZE + TAG_SIZE + 4)..MAX_RECORD) { "Invalid credential record" }
            bytes.copyOf(size)
        }
        val record = ByteBuffer.wrap(encoded)
        require(record.int == MAGIC) { "Unsupported credential format" }
        val iv = ByteArray(IV_SIZE).also { record.get(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(root, false), GCMParameterSpec(TAG_SIZE * 8, iv))
        cipher.updateAAD(aad(id))
        val plain = cipher.doFinal(encoded, record.position(), record.remaining())
        try {
            val data = ByteBuffer.wrap(plain)
            val length = data.int
            require(length in 0..data.remaining()) { "Invalid credential payload" }
            val username = Charsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(plain, 4, length)).toString()
            StoredCredential(username, plain.copyOfRange(4 + length, plain.size))
        } finally { plain.fill(0) }
    }

    fun delete(url: String) {
        locked(false) { root ->
            val id = identifier(url)
            Files.deleteIfExists(File(root, "$id.bin").toPath())
            Files.deleteIfExists(File(root, "$id.tmp").toPath())
        }
    }

    private fun <T> locked(create: Boolean, action: (File) -> T): T? = synchronized(processLock) {
        val root = directory
        if (!root.exists() && !create) return@synchronized null
        if (!root.isDirectory && !root.mkdirs()) throw IOException("Credential directory unavailable")
        RandomAccessFile(File(root, ".lock"), "rw").channel.use { channel ->
            channel.lock().use { action(root) }
        }
    }

    private fun key(root: File, create: Boolean): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val existing = store.getKey(KEY_ALIAS, null)
        if (existing != null) return existing as SecretKey
        check(create) { "Credential key unavailable" }
        val files = root.listFiles() ?: throw IOException("Credential directory unavailable")
        check(files.none { it.name.endsWith(".bin") }) { "Credential key unavailable" }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build())
        }.generateKey()
    }

    private fun identifier(url: String): String = MessageDigest.getInstance("SHA-256")
        .digest(url.toByteArray(Charsets.UTF_8)).joinToString("") { "%02x".format(it) }
    private fun aad(id: String): ByteArray = "gpui-credentials-v1:$id".toByteArray(Charsets.US_ASCII)

    companion object {
        private val processLock = Any()
        private const val KEY_ALIAS = "dev.gpui.credentials.aes.v1"
        private const val MAGIC = 0x47504331
        private const val IV_SIZE = 12
        private const val TAG_SIZE = 16
        private const val MAX_PAYLOAD = 1024 * 1024
        private const val MAX_RECORD = 4 + IV_SIZE + MAX_PAYLOAD + TAG_SIZE
    }
}
