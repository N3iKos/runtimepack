package io.runtimepack.host

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class GpackCryptoTest {

    private fun ByteArray.toHexString(): String {
        return joinToString("") { "%02x".format(it) }
    }

    @Test
    fun testDerivePackageKeyMatchesSpecification() {
        val key1 = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        assertEquals(32, key1.size)

        val key2 = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        assertArrayEquals(key1, key2)

        val keyDiffPackage = GpackCrypto.derivePackageKey("com.example.othergame", "Lona RPG")
        assertEquals(32, keyDiffPackage.size)
        assertFalseArrayEquals(key1, keyDiffPackage)

        val keyDiffName = GpackCrypto.derivePackageKey("com.example.lonarpg", "Other Game")
        assertEquals(32, keyDiffName.size)
        assertFalseArrayEquals(key1, keyDiffName)
    }

    @Test
    fun testDerivePackageKeyHkdfVector() {
        val key = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        val expectedHex = "f966700e5e7bc80873900e2c2db5d08d464a34d92485397b3ba2b28cffa1006a"
        assertEquals(expectedHex, key.toHexString())
    }

    @Test
    fun testMakeNonceSpecification() {
        val fileIndex = 1
        val chunkIndex = 2
        val nonce = GpackCrypto.makeNonce(fileIndex, chunkIndex)
        assertEquals(12, nonce.size)

        val expected = byteArrayOf(
            0x01, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00
        )
        assertArrayEquals(expected, nonce)
    }

    @Test
    fun testEncryptDecryptRoundtrip() {
        val key = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        val plaintext = "Hello RuntimePack Encrypted Streaming VFS!".toByteArray(Charsets.UTF_8)
        val fileIndex = 42
        val chunkIndex = 7

        val encrypted = GpackCrypto.encryptChunk(key, chunkIndex, fileIndex, plaintext)
        assertEquals(plaintext.size + 16, encrypted.size)

        val decrypted = GpackCrypto.decryptChunk(key, chunkIndex, fileIndex, encrypted)
        assertArrayEquals(plaintext, decrypted)
    }

    @Test
    fun testDecryptTamperedChunkFails() {
        val key = ByteArray(32) { 0x42 }
        val dummyCiphertextWithTag = ByteArray(48) { 0xAA.toByte() }
        assertThrows(Exception::class.java) {
            GpackCrypto.decryptChunk(key, 0, 0, dummyCiphertextWithTag)
        }
    }

    @Test
    fun testDecryptValidation() {
        val validKey = ByteArray(32) { 0x11 }
        val invalidKey = ByteArray(16) { 0x11 }
        val shortCiphertext = ByteArray(15) { 0x22 }

        assertThrows(IllegalArgumentException::class.java) {
            GpackCrypto.decryptChunk(invalidKey, 0, 0, ByteArray(32))
        }

        assertThrows(IllegalArgumentException::class.java) {
            GpackCrypto.decryptChunk(validKey, 0, 0, shortCiphertext)
        }
    }

    private fun assertFalseArrayEquals(a: ByteArray, b: ByteArray) {
        var equal = a.size == b.size
        if (equal) {
            for (i in a.indices) {
                if (a[i] != b[i]) {
                    equal = false
                    break
                }
            }
        }
        org.junit.Assert.assertFalse("Arrays should not be equal", equal)
    }
}
