package io.runtimepack.host

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

class GpackStreamReaderTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    private val GPACK_MAGIC = byteArrayOf(
        'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
        'K'.code.toByte(), 0, 1, 0
    )

    private data class PackItem(
        val path: String,
        val content: ByteArray,
        val encrypt: Boolean = false
    )

    private fun createPackData(items: List<PackItem>, key: ByteArray?): ByteArray {
        val anyEncrypted = items.any { it.encrypt }
        val flags = if (anyEncrypted) 1 else 0

        // 1. Process files and calculate index
        class FilePlan(
            val path: String,
            val rawData: ByteArray,
            val storedData: ByteArray,
            val encryptByte: Byte
        )

        val plans = items.mapIndexed { index, item ->
            val (stored, encByte) = if (item.encrypt) {
                requireNotNull(key) { "Key required for encryption" }
                val cipher = GpackCrypto.encryptChunk(key, 0, index, item.content)
                Pair(cipher, 1.toByte())
            } else {
                Pair(item.content, 0.toByte())
            }
            FilePlan(item.path, item.content, stored, encByte)
        }

        var indexBytesLen = 0
        for (p in plans) {
            val pathLen = p.path.toByteArray(Charsets.UTF_8).size
            // 4 + pathLen + 8 (offset) + 8 (size) + 8 (storedSize) + 32 (hash) + 1 (comp) + 1 (enc)
            indexBytesLen += 4 + pathLen + 8 + 8 + 8 + 32 + 1 + 1
        }

        val headerSize = 64
        val indexOffset = headerSize.toLong()
        val indexLength = indexBytesLen.toLong()
        val dataStartOffset = indexOffset + indexLength

        // Header buffer
        val headerBuf = ByteArray(64)
        System.arraycopy(GPACK_MAGIC, 0, headerBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(1) // version
        hWrap.putInt(flags) // flags
        hWrap.putLong(indexOffset)
        hWrap.putLong(indexLength)
        hWrap.putInt(plans.size)

        // Index buffer
        val indexBuf = ByteArray(indexBytesLen)
        val iWrap = ByteBuffer.wrap(indexBuf).order(ByteOrder.LITTLE_ENDIAN)

        var curOffset = dataStartOffset
        val fileOffsets = mutableListOf<Long>()
        for (p in plans) {
            fileOffsets.add(curOffset)
            curOffset += p.storedData.size
        }

        for ((i, p) in plans.withIndex()) {
            val pBytes = p.path.toByteArray(Charsets.UTF_8)
            iWrap.putInt(pBytes.size)
            iWrap.put(pBytes)
            iWrap.putLong(fileOffsets[i])
            iWrap.putLong(p.rawData.size.toLong())
            iWrap.putLong(p.storedData.size.toLong())
            iWrap.put(ByteArray(32)) // dummy hash
            iWrap.put(0.toByte()) // compression
            iWrap.put(p.encryptByte)
        }

        val out = ByteArrayOutputStream()
        out.write(headerBuf)
        out.write(indexBuf)
        for (p in plans) {
            out.write(p.storedData)
        }
        return out.toByteArray()
    }

    @Test
    fun testRejectInvalidMagicHeader() {
        val invalidFile = tempFolder.newFile("invalid.pack")
        invalidFile.writeBytes(ByteArray(64) { 0x00 })

        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(invalidFile, null)
        }
    }

    @Test
    fun testRejectUnsupportedVersion() {
        val badVersionBuf = ByteArray(64)
        System.arraycopy(GPACK_MAGIC, 0, badVersionBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(badVersionBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(2) // unsupported version 2
        hWrap.putInt(0)
        hWrap.putLong(64L)
        hWrap.putLong(0L)
        hWrap.putInt(0)

        val badFile = tempFolder.newFile("bad_version.pack")
        badFile.writeBytes(badVersionBuf)

        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(badFile, null)
        }
    }

    @Test
    fun testOpenUnencryptedPackAndReadFiles() {
        val items = listOf(
            PackItem("data/System.json", "{\"gameTitle\":\"Test Game\"}".toByteArray(Charsets.UTF_8)),
            PackItem("js/main.js", "console.log('running');".toByteArray(Charsets.UTF_8))
        )

        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("unencrypted.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            assertFalse(reader.isEncrypted)
            assertEquals(2, reader.totalFiles)

            assertTrue(reader.hasFile("data/System.json"))
            assertTrue(reader.hasFile("data\\system.json"))
            assertTrue(reader.hasFile("/DATA/SYSTEM.JSON"))
            assertTrue(reader.hasFile("js/main.js"))
            assertFalse(reader.hasFile("non_existent.txt"))

            val sysStream = reader.openStream("Data/System.json")
            assertNotNull(sysStream)
            val sysContent = sysStream!!.readBytes().toString(Charsets.UTF_8)
            assertEquals("{\"gameTitle\":\"Test Game\"}", sysContent)

            val jsStream = reader.openStream("js/main.js")
            assertNotNull(jsStream)
            assertEquals("console.log('running');", jsStream!!.readBytes().toString(Charsets.UTF_8))

            val missingStream = reader.openStream("missing.txt")
            assertNull(missingStream)
        }
    }

    @Test
    fun testOpenEncryptedPackAndReadDecryptedFiles() {
        val key = GpackCrypto.derivePackageKey("com.example.game", "Test Game")
        val items = listOf(
            PackItem("Data/Actors.rvdata2", "Secret Actor Data".toByteArray(Charsets.UTF_8), encrypt = true),
            PackItem("Game.ini", "[Game]\nTitle=Test Game".toByteArray(Charsets.UTF_8), encrypt = false)
        )

        val packBytes = createPackData(items, key)
        val packFile = tempFolder.newFile("encrypted.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, key).use { reader ->
            assertTrue(reader.isEncrypted)
            assertEquals(2, reader.totalFiles)

            assertTrue(reader.hasFile("Data/Actors.rvdata2"))
            assertTrue(reader.hasFile("game.ini"))

            val actorStream = reader.openStream("data/actors.rvdata2")
            assertNotNull(actorStream)
            assertEquals("Secret Actor Data", actorStream!!.readBytes().toString(Charsets.UTF_8))

            val iniStream = reader.openStream("Game.ini")
            assertNotNull(iniStream)
            assertEquals("[Game]\nTitle=Test Game", iniStream!!.readBytes().toString(Charsets.UTF_8))
        }
    }

    @Test
    fun testEncryptedPackFailsWithoutKey() {
        val key = GpackCrypto.derivePackageKey("com.example.game", "Test Game")
        val items = listOf(
            PackItem("secret.dat", "Confidential".toByteArray(Charsets.UTF_8), encrypt = true)
        )

        val packBytes = createPackData(items, key)
        val packFile = tempFolder.newFile("encrypted_no_key.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            assertThrows(IllegalStateException::class.java) {
                reader.openStream("secret.dat")
            }
        }
    }

    @Test
    fun testSupportRandomAccessFileAndInputStreamSources() {
        val items = listOf(
            PackItem("assets/bg.png", byteArrayOf(1, 2, 3, 4, 5))
        )
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("source_test.pack")
        packFile.writeBytes(packBytes)

        // 1. Test via RandomAccessFile
        RandomAccessFile(packFile, "r").use { raf ->
            GpackStreamReader.open(raf, null).use { reader ->
                assertTrue(reader.hasFile("assets/bg.png"))
                val bytes = reader.openStream("assets/bg.png")!!.readBytes()
                assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5), bytes)
            }
        }

        // 2. Test via InputStream
        packFile.inputStream().use { stream ->
            GpackStreamReader.open(stream, null).use { reader ->
                assertTrue(reader.hasFile("assets/bg.png"))
                val bytes = reader.openStream("assets/bg.png")!!.readBytes()
                assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5), bytes)
            }
        }
    }
}
