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
import java.io.EOFException
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.concurrent.Callable
import java.util.concurrent.Executors

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

    @Test
    fun testVirtualHugeRawEntryDoesNotAllocateFullPayloadOnOpenStream() {
        val hugeSize = 100_000_000L
        val path = "huge_virtual.dat"
        val pathBytes = path.toByteArray(Charsets.UTF_8)

        val headerSize = 64
        val indexLength = (4 + pathBytes.size + 8 + 8 + 8 + 32 + 1 + 1).toLong()
        val dataOffset = headerSize + indexLength

        val headerBuf = ByteArray(headerSize)
        System.arraycopy(GPACK_MAGIC, 0, headerBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(1)
        hWrap.putInt(0)
        hWrap.putLong(headerSize.toLong())
        hWrap.putLong(indexLength)
        hWrap.putInt(1)

        val indexBuf = ByteArray(indexLength.toInt())
        val iWrap = ByteBuffer.wrap(indexBuf).order(ByteOrder.LITTLE_ENDIAN)
        iWrap.putInt(pathBytes.size)
        iWrap.put(pathBytes)
        iWrap.putLong(dataOffset)
        iWrap.putLong(hugeSize)
        iWrap.putLong(hugeSize)
        iWrap.put(ByteArray(32))
        iWrap.put(0.toByte())
        iWrap.put(0.toByte())

        val metadataBytes = ByteArray(headerSize + indexLength.toInt())
        System.arraycopy(headerBuf, 0, metadataBytes, 0, headerSize)
        System.arraycopy(indexBuf, 0, metadataBytes, headerSize, indexLength.toInt())

        class BoundedSpySource : GpackStreamReader.SeekableSource {
            var maxRequestedLength = 0
            var readFullyCallCount = 0

            override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
                readFullyCallCount++
                if (length > maxRequestedLength) {
                    maxRequestedLength = length
                }
                if (position < metadataBytes.size) {
                    val avail = metadataBytes.size - position.toInt()
                    val toCopy = minOf(length, avail)
                    System.arraycopy(metadataBytes, position.toInt(), buffer, offset, toCopy)
                    if (toCopy < length) {
                        for (i in toCopy until length) {
                            buffer[offset + i] = ((position + i) and 0xFF).toByte()
                        }
                    }
                } else {
                    if (length > 65536) {
                        throw AssertionError("Spy guard tripped: unbounded read requested ($length bytes > 65536)")
                    }
                    for (i in 0 until length) {
                        buffer[offset + i] = ((position + i) and 0xFF).toByte()
                    }
                }
            }

            override fun close() {}
        }

        val spySource = BoundedSpySource()
        GpackStreamReader.open(spySource, null).use { reader ->
            val stream = reader.openStream(path)
            assertNotNull(stream)
            val smallBuf = ByteArray(16)
            val bytesRead = stream!!.read(smallBuf)
            assertEquals(16, bytesRead)
        }
    }

    private fun sha256(bytes: ByteArray): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        return md.digest(bytes)
    }

    @Test
    fun testIrregularBulkReadsAcross64KiBAndAdjacentEntriesNoOverrun() {
        val sizeA = 131072
        val dataA = ByteArray(sizeA) { i -> ((i * 31 + 7) and 0xFF).toByte() }
        val sizeB = 65536
        val dataB = ByteArray(sizeB) { i -> ((i * 17 + 13) and 0xFF).toByte() }

        val shaA = sha256(dataA)
        val shaB = sha256(dataB)

        val items = listOf(
            PackItem("entry_a.bin", dataA),
            PackItem("entry_b.bin", dataB)
        )
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("bulk_test.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            val streamA = reader.openStream("entry_a.bin")
            assertNotNull(streamA)

            val irregularSizes = intArrayOf(17, 4099, 65537, 3, 32768, 1024, 27624)
            val outA = ByteArrayOutputStream()
            var sizeIdx = 0

            while (true) {
                val chunkSize = irregularSizes[sizeIdx % irregularSizes.size]
                sizeIdx++
                val buf = ByteArray(chunkSize)
                val read = streamA!!.read(buf, 0, buf.size)
                if (read == -1) break
                outA.write(buf, 0, read)
            }

            val readA = outA.toByteArray()
            assertEquals(sizeA, readA.size)
            assertArrayEquals(shaA, sha256(readA))
            assertEquals(-1, streamA.read())

            val streamB = reader.openStream("entry_b.bin")
            assertNotNull(streamB)
            val readB = streamB!!.readBytes()
            assertEquals(sizeB, readB.size)
            assertArrayEquals(shaB, sha256(readB))
        }
    }

    @Test
    fun testReadWithInvalidOffsetsAndLengthAndZeroLenAtEof() {
        val data = ByteArray(20) { it.toByte() }
        val items = listOf(PackItem("bounds.dat", data))
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("bounds.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            val stream = reader.openStream("bounds.dat")
            assertNotNull(stream)

            val b = ByteArray(10)
            assertThrows(IndexOutOfBoundsException::class.java) { stream!!.read(b, -1, 5) }
            assertThrows(IndexOutOfBoundsException::class.java) { stream!!.read(b, 0, -1) }
            assertThrows(IndexOutOfBoundsException::class.java) { stream!!.read(b, 8, 5) }
            assertThrows(IndexOutOfBoundsException::class.java) { stream!!.read(b, Int.MAX_VALUE, 1) }
            assertThrows(IndexOutOfBoundsException::class.java) { stream!!.read(b, 1, Int.MAX_VALUE) }

            assertEquals(0, stream!!.read(b, 0, 0))

            val sink = ByteArray(20)
            var totalRead = 0
            while (totalRead < 20) {
                val r = stream.read(sink, totalRead, 20 - totalRead)
                if (r == -1) break
                totalRead += r
            }
            assertEquals(20, totalRead)

            assertEquals(0, stream.read(b, 0, 0))
            assertEquals(-1, stream.read(b, 0, 1))
            assertEquals(-1, stream.read())
        }
    }

    @Test
    fun testReadUnsignedByteValues() {
        val testBytes = byteArrayOf(
            0x00.toByte(),
            0x7F.toByte(),
            0x80.toByte(),
            0xFE.toByte(),
            0xFF.toByte()
        )
        val items = listOf(PackItem("unsigned.dat", testBytes))
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("unsigned.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            val stream = reader.openStream("unsigned.dat")
            assertNotNull(stream)

            assertEquals(0, stream!!.read())
            assertEquals(127, stream.read())
            assertEquals(128, stream.read())
            assertEquals(254, stream.read())
            assertEquals(255, stream.read())
            assertEquals(-1, stream.read())
        }
    }

    @Test
    fun testAvailableAndSkipBehavior() {
        val data = ByteArray(100) { it.toByte() }
        val items = listOf(PackItem("skip.dat", data))
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("skip.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            val stream = reader.openStream("skip.dat")
            assertNotNull(stream)

            assertEquals(100, stream!!.available())
            assertEquals(0L, stream.skip(-10L))
            assertEquals(0L, stream.skip(0L))
            assertEquals(100, stream.available())

            assertEquals(30L, stream.skip(30L))
            assertEquals(70, stream.available())

            assertEquals(70L, stream.skip(200L))
            assertEquals(0, stream.available())
            assertEquals(-1, stream.read())
            assertEquals(0L, stream.skip(10L))

            assertFalse(stream.markSupported())
            assertThrows(IOException::class.java) { stream.reset() }
        }
    }

    @Test
    fun testStreamCloseIndependentOfParentAndReaderCloseFailsSubsequentOpenAndRead() {
        val items = listOf(
            PackItem("file1.txt", "file1 content".toByteArray(Charsets.UTF_8)),
            PackItem("file2.txt", "file2 content".toByteArray(Charsets.UTF_8))
        )
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("close_test.pack")
        packFile.writeBytes(packBytes)

        val reader = GpackStreamReader.open(packFile, null)
        val s1 = reader.openStream("file1.txt")
        val s2 = reader.openStream("file2.txt")
        assertNotNull(s1)
        assertNotNull(s2)

        s1!!.close()

        assertThrows(IOException::class.java) { s1.read() }
        assertThrows(IOException::class.java) { s1.read(ByteArray(5), 0, 5) }
        assertThrows(IOException::class.java) { s1.available() }
        assertThrows(IOException::class.java) { s1.skip(5L) }

        assertEquals("file2 content", s2!!.readBytes().toString(Charsets.UTF_8))

        val s1Reopened = reader.openStream("file1.txt")
        assertNotNull(s1Reopened)
        assertEquals("file1 content", s1Reopened!!.readBytes().toString(Charsets.UTF_8))

        reader.close()
        assertTrue(reader.isClosed)

        assertThrows(IOException::class.java) { reader.openStream("file1.txt") }
        assertThrows(IOException::class.java) { s1Reopened.read() }
        assertThrows(IOException::class.java) { s2.read() }
    }

    @Test
    fun testTwoRealConcurrentSourceStreamsDetectSeekRaces() {
        val sizeA = 100000
        val sizeB = 100000
        val dataA = ByteArray(sizeA) { i -> ((i * 19 + 3) and 0xFF).toByte() }
        val dataB = ByteArray(sizeB) { i -> ((i * 23 + 11) and 0xFF).toByte() }
        val shaA = sha256(dataA)
        val shaB = sha256(dataB)

        val items = listOf(
            PackItem("concurrent_a.bin", dataA),
            PackItem("concurrent_b.bin", dataB)
        )
        val packBytes = createPackData(items, null)
        val packFile = tempFolder.newFile("concurrent_test.pack")
        packFile.writeBytes(packBytes)

        RandomAccessFile(packFile, "r").use { raf ->
            GpackStreamReader.open(raf, null).use { reader ->
                val streamA = reader.openStream("concurrent_a.bin")
                val streamB = reader.openStream("concurrent_b.bin")
                assertNotNull(streamA)
                assertNotNull(streamB)

                val executor = Executors.newFixedThreadPool(2)
                val futureA = executor.submit(Callable {
                    val out = ByteArrayOutputStream()
                    val irregularChunksA = intArrayOf(13, 257, 1023, 7, 4096, 64)
                    var idx = 0
                    while (true) {
                        val len = irregularChunksA[idx % irregularChunksA.size]
                        idx++
                        val buf = ByteArray(len)
                        val r = streamA!!.read(buf, 0, buf.size)
                        if (r == -1) break
                        out.write(buf, 0, r)
                    }
                    out.toByteArray()
                })

                val futureB = executor.submit(Callable {
                    val out = ByteArrayOutputStream()
                    val irregularChunksB = intArrayOf(77, 513, 4096, 19, 128, 2048)
                    var idx = 0
                    while (true) {
                        val len = irregularChunksB[idx % irregularChunksB.size]
                        idx++
                        val buf = ByteArray(len)
                        val r = streamB!!.read(buf, 0, buf.size)
                        if (r == -1) break
                        out.write(buf, 0, r)
                    }
                    out.toByteArray()
                })

                val resultA = futureA.get()
                val resultB = futureB.get()
                executor.shutdown()

                assertEquals(sizeA, resultA.size)
                assertArrayEquals(shaA, sha256(resultA))
                assertEquals(sizeB, resultB.size)
                assertArrayEquals(shaB, sha256(resultB))
            }
        }
    }

    @Test
    fun testFileChannelAssetFdNonzeroBaseOffsetPreservesOffsets() {
        val payload = "Hello BaseOffset".toByteArray(Charsets.UTF_8)
        val items = listOf(PackItem("offset_file.txt", payload))
        val packBytes = createPackData(items, null)

        val baseOffset = 1024L
        val prefix = ByteArray(baseOffset.toInt()) { 0x55.toByte() }
        val testFile = tempFolder.newFile("base_offset.pack")
        FileOutputStream(testFile).use { fos ->
            fos.write(prefix)
            fos.write(packBytes)
        }

        RandomAccessFile(testFile, "r").use { raf ->
            val channel = raf.channel
            val source = GpackStreamReader.ChannelSource(
                channel = channel,
                baseOffset = baseOffset,
                declaredLength = packBytes.size.toLong()
            )
            GpackStreamReader.open(source, null).use { reader ->
                assertTrue(reader.hasFile("offset_file.txt"))
                val stream = reader.openStream("offset_file.txt")
                assertNotNull(stream)
                val readData = stream!!.readBytes()
                assertArrayEquals(payload, readData)
            }
        }
    }

    @Test
    fun testRejectCorruptOrInvalidIndexEntriesAndTruncatedSource() {
        fun makePackHeaderAndIndex(
            offset: Long,
            size: Long,
            storedSize: Long,
            compression: Byte,
            encryption: Byte
        ): ByteArray {
            val path = "entry.dat"
            val pBytes = path.toByteArray(Charsets.UTF_8)
            val headerSize = 64
            val indexLen = (4 + pBytes.size + 8 + 8 + 8 + 32 + 1 + 1).toLong()

            val hBuf = ByteArray(headerSize)
            System.arraycopy(GPACK_MAGIC, 0, hBuf, 0, 8)
            val hw = ByteBuffer.wrap(hBuf).order(ByteOrder.LITTLE_ENDIAN)
            hw.position(8)
            hw.putInt(1)
            hw.putInt(0)
            hw.putLong(headerSize.toLong())
            hw.putLong(indexLen)
            hw.putInt(1)

            val iBuf = ByteArray(indexLen.toInt())
            val iw = ByteBuffer.wrap(iBuf).order(ByteOrder.LITTLE_ENDIAN)
            iw.putInt(pBytes.size)
            iw.put(pBytes)
            iw.putLong(offset)
            iw.putLong(size)
            iw.putLong(storedSize)
            iw.put(ByteArray(32))
            iw.put(compression)
            iw.put(encryption)

            val out = ByteArray(headerSize + indexLen.toInt())
            System.arraycopy(hBuf, 0, out, 0, headerSize)
            System.arraycopy(iBuf, 0, out, headerSize, indexLen.toInt())
            return out
        }

        val rawMismatch = makePackHeaderAndIndex(100L, 50L, 60L, 0.toByte(), 0.toByte())
        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(GpackStreamReader.ByteArraySource(rawMismatch), null)
        }

        val negativeOffset = makePackHeaderAndIndex(-1L, 50L, 50L, 0.toByte(), 0.toByte())
        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(GpackStreamReader.ByteArraySource(negativeOffset), null)
        }

        val overflowOffset = makePackHeaderAndIndex(Long.MAX_VALUE - 10L, 20L, 20L, 0.toByte(), 0.toByte())
        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(GpackStreamReader.ByteArraySource(overflowOffset), null)
        }

        val unknownComp = makePackHeaderAndIndex(100L, 50L, 50L, 1.toByte(), 0.toByte())
        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(GpackStreamReader.ByteArraySource(unknownComp), null)
        }

        val unknownEnc = makePackHeaderAndIndex(100L, 50L, 50L, 0.toByte(), 2.toByte())
        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(GpackStreamReader.ByteArraySource(unknownEnc), null)
        }

        val validMetaTruncatedPayload = makePackHeaderAndIndex(200L, 50L, 50L, 0.toByte(), 0.toByte())
        GpackStreamReader.open(GpackStreamReader.ByteArraySource(validMetaTruncatedPayload), null).use { reader ->
            val stream = reader.openStream("entry.dat")
            assertNotNull(stream)
            assertThrows(EOFException::class.java) {
                stream!!.read(ByteArray(50), 0, 50)
            }
        }
    }

    @Test
    fun testEncryptedChunkAuthenticationTamperFailsAndKeyCheckedBeforeAlloc() {
        val key = GpackCrypto.derivePackageKey("com.example.game", "Test Game")
        val secretPayload = "Very Sensitive Content".toByteArray(Charsets.UTF_8)
        val items = listOf(PackItem("secret.dat", secretPayload, encrypt = true))
        val packBytes = createPackData(items, key)

        val packFile = tempFolder.newFile("encrypted_tamper.pack")
        packFile.writeBytes(packBytes)

        GpackStreamReader.open(packFile, null).use { reader ->
            val ex = assertThrows(IllegalStateException::class.java) {
                reader.openStream("secret.dat")
            }
            assertTrue(ex.message?.contains("Container chunk is encrypted but no key provided") == true)
        }

        val tamperedBytes = packBytes.clone()
        tamperedBytes[tamperedBytes.size - 5] = (tamperedBytes[tamperedBytes.size - 5].toInt() xor 0x55).toByte()
        val tamperedFile = tempFolder.newFile("encrypted_tampered_data.pack")
        tamperedFile.writeBytes(tamperedBytes)

        GpackStreamReader.open(tamperedFile, key).use { reader ->
            assertThrows(Exception::class.java) {
                reader.openStream("secret.dat")
            }
        }
    }
}
