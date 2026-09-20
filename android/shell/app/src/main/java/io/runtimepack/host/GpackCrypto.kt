package io.runtimepack.host

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.NoSuchAlgorithmException
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

object GpackCrypto {

    private const val HKDF_SALT = "runtimepack_salt_v1"
    private const val HKDF_INFO = "gpack-content-v1"

    private fun getChaCha20Poly1305Cipher(): Cipher {
        val transformations = arrayOf(
            "ChaCha20-Poly1305/None/NoPadding",
            "ChaCha20/Poly1305/NoPadding",
            "ChaCha20-Poly1305"
        )
        for (transformation in transformations) {
            try {
                return Cipher.getInstance(transformation)
            } catch (_: Exception) {
            }
        }
        throw NoSuchAlgorithmException("ChaCha20-Poly1305 cipher transformation not found")
    }

    fun derivePackageKey(packageName: String, appName: String): ByteArray {
        val ikm = "$packageName::$appName".toByteArray(Charsets.UTF_8)
        val salt = HKDF_SALT.toByteArray(Charsets.UTF_8)
        val info = HKDF_INFO.toByteArray(Charsets.UTF_8)

        val hmacExtract = Mac.getInstance("HmacSHA256")
        hmacExtract.init(SecretKeySpec(salt, "HmacSHA256"))
        val prk = hmacExtract.doFinal(ikm)

        val hmacExpand = Mac.getInstance("HmacSHA256")
        hmacExpand.init(SecretKeySpec(prk, "HmacSHA256"))
        hmacExpand.update(info)
        hmacExpand.update(0x01.toByte())
        return hmacExpand.doFinal()
    }

    fun makeNonce(fileIndex: Int, chunkIndex: Int): ByteArray {
        val nonce = ByteArray(12)
        val bb = ByteBuffer.wrap(nonce).order(ByteOrder.LITTLE_ENDIAN)
        bb.putInt(fileIndex)
        bb.putInt(chunkIndex)
        bb.putInt(0)
        return nonce
    }

    fun encryptChunk(key: ByteArray, chunkIndex: Int, fileIndex: Int, plaintext: ByteArray): ByteArray {
        require(key.size == 32) { "Key must be 256 bits (32 bytes)" }

        val nonce = makeNonce(fileIndex, chunkIndex)
        val cipher = getChaCha20Poly1305Cipher()
        val keySpec = SecretKeySpec(key, "ChaCha20")
        val ivSpec = IvParameterSpec(nonce)

        cipher.init(Cipher.ENCRYPT_MODE, keySpec, ivSpec)
        return cipher.doFinal(plaintext)
    }

    fun decryptChunk(key: ByteArray, chunkIndex: Int, fileIndex: Int, ciphertextWithTag: ByteArray): ByteArray {
        require(key.size == 32) { "Key must be 256 bits (32 bytes)" }
        require(ciphertextWithTag.size >= 16) { "Ciphertext must contain at least 16-byte Poly1305 tag" }

        val nonce = makeNonce(fileIndex, chunkIndex)
        val cipher = getChaCha20Poly1305Cipher()
        val keySpec = SecretKeySpec(key, "ChaCha20")
        val ivSpec = IvParameterSpec(nonce)

        cipher.init(Cipher.DECRYPT_MODE, keySpec, ivSpec)
        return cipher.doFinal(ciphertextWithTag)
    }
}
