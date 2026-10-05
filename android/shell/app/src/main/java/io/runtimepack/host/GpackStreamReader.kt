package io.runtimepack.host

import android.content.res.AssetFileDescriptor
import java.io.Closeable
import java.io.EOFException
import java.io.File
import java.io.FileInputStream
import java.io.IOException
import java.io.InputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.channels.FileChannel

class GpackStreamReader private constructor(
    internal val source: SeekableSource,
    val key: ByteArray?,
    val version: Int,
    val flags: Int,
    val indexOffset: Long,
    val indexLength: Long,
    val fileCount: Int,
    val fileIndexMap: Map<String, IndexEntry>
) : AutoCloseable, Closeable {

    @Volatile
    var isClosed: Boolean = false
        private set

    val isEncrypted: Boolean
        get() = (flags and 1) != 0

    val totalFiles: Int
        get() = fileCount

    val entries: Collection<IndexEntry>
        get() = fileIndexMap.values

    data class IndexEntry(
        val index: Int,
        val logicalPath: String,
        val offset: Long,
        val size: Long,
        val storedSize: Long,
        val hash: ByteArray,
        val compression: Byte,
        val encryption: Byte
    ) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (javaClass != other?.javaClass) return false

            other as IndexEntry

            if (index != other.index) return false
            if (logicalPath != other.logicalPath) return false
            if (offset != other.offset) return false
            if (size != other.size) return false
            if (storedSize != other.storedSize) return false
            if (!hash.contentEquals(other.hash)) return false
            if (compression != other.compression) return false
            if (encryption != other.encryption) return false

            return true
        }

        override fun hashCode(): Int {
            var result = index
            result = 31 * result + logicalPath.hashCode()
            result = 31 * result + offset.hashCode()
            result = 31 * result + size.hashCode()
            result = 31 * result + storedSize.hashCode()
            result = 31 * result + hash.contentHashCode()
            result = 31 * result + compression
            result = 31 * result + encryption
            return result
        }
    }

    interface SeekableSource : AutoCloseable {
        fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int)
    }

    internal class RafSource(private val raf: RandomAccessFile) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
            if (position < 0L || length < 0) {
                throw EOFException("Invalid position $position or length $length")
            }
            synchronized(raf) {
                raf.seek(position)
                raf.readFully(buffer, offset, length)
            }
        }

        override fun close() {
            synchronized(raf) {
                raf.close()
            }
        }
    }

    internal class ChannelSource(
        private val channel: FileChannel,
        val baseOffset: Long = 0L,
        private val closeable: AutoCloseable? = null,
        private val declaredLength: Long = -1L
    ) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
            if (position < 0L || length < 0) {
                throw EOFException("Invalid position $position or length $length")
            }
            if (declaredLength >= 0L && (position > declaredLength || length.toLong() > declaredLength - position)) {
                throw EOFException("Position $position and length $length exceed declared length $declaredLength")
            }
            if (baseOffset > 0L && position > Long.MAX_VALUE - baseOffset) {
                throw EOFException("Position $position + baseOffset $baseOffset overflows Long")
            }
            synchronized(channel) {
                val targetPos = baseOffset + position
                channel.position(targetPos)
                val buf = ByteBuffer.wrap(buffer, offset, length)
                while (buf.hasRemaining()) {
                    val read = channel.read(buf)
                    if (read == -1) {
                        throw EOFException("Unexpected EOF at position $targetPos while reading $length bytes")
                    }
                }
            }
        }

        override fun close() {
            try {
                synchronized(channel) {
                    channel.close()
                }
            } finally {
                closeable?.close()
            }
        }
    }

    internal class ByteArraySource(private val data: ByteArray) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
            if (position < 0L || length < 0 || position > data.size || length.toLong() > data.size - position) {
                throw EOFException("Invalid position $position or length $length for byte array of size ${data.size}")
            }
            System.arraycopy(data, position.toInt(), buffer, offset, length)
        }

        override fun close() {
        }
    }

    internal class BoundedEntryStream(
        private val reader: GpackStreamReader,
        private val entry: IndexEntry
    ) : InputStream() {
        private var cursor: Long = 0L
        private var remaining: Long = entry.size
        @Volatile
        private var isClosed = false
        private val singleByteBuf = ByteArray(1)

        override fun read(): Int {
            synchronized(this) {
                val n = read(singleByteBuf, 0, 1)
                if (n == -1) return -1
                return singleByteBuf[0].toInt() and 0xFF
            }
        }

        override fun read(b: ByteArray, off: Int, len: Int): Int {
            synchronized(this) {
                if (isClosed || reader.isClosed) {
                    throw IOException("Stream is closed")
                }
                if (off < 0 || len < 0 || len > b.size - off) {
                    throw IndexOutOfBoundsException("Invalid offset $off or length $len for buffer size ${b.size}")
                }
                if (len == 0) {
                    return 0
                }
                if (remaining <= 0L) {
                    return -1
                }
                val toRead = minOf(len.toLong(), remaining).toInt()
                reader.readFullyInternal(entry.offset + cursor, b, off, toRead)
                cursor += toRead
                remaining -= toRead
                return toRead
            }
        }

        override fun skip(n: Long): Long {
            synchronized(this) {
                if (isClosed || reader.isClosed) {
                    throw IOException("Stream is closed")
                }
                if (n <= 0L) {
                    return 0L
                }
                val toSkip = minOf(n, remaining)
                cursor += toSkip
                remaining -= toSkip
                return toSkip
            }
        }

        override fun available(): Int {
            if (isClosed || reader.isClosed) {
                throw IOException("Stream is closed")
            }
            return minOf(remaining, Int.MAX_VALUE.toLong()).toInt()
        }

        override fun markSupported(): Boolean = false

        override fun reset() {
            throw IOException("mark/reset not supported")
        }

        override fun close() {
            isClosed = true
        }
    }

    companion object {
        val GPACK_MAGIC = byteArrayOf(
            'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
            'K'.code.toByte(), 0, 1, 0
        )
        val GPACK_MAGIC_ALT = byteArrayOf(
            'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
            'K'.code.toByte(), '1'.code.toByte(), 0, 0
        )

        private fun matchesMagic(header: ByteArray): Boolean {
            if (header.size < 8) return false
            val matchesV1 = (0 until 8).all { header[it] == GPACK_MAGIC[it] }
            val matchesAlt = (0 until 8).all { header[it] == GPACK_MAGIC_ALT[it] }
            return matchesV1 || matchesAlt
        }

        fun open(packFile: File, key: ByteArray? = null): GpackStreamReader {
            val raf = RandomAccessFile(packFile, "r")
            return open(RafSource(raf), key)
        }

        fun open(raf: RandomAccessFile, key: ByteArray? = null): GpackStreamReader {
            return open(RafSource(raf), key)
        }

        fun open(inputStream: InputStream, key: ByteArray? = null): GpackStreamReader {
            val bytes = inputStream.readBytes()
            return open(ByteArraySource(bytes), key)
        }

        fun open(afd: AssetFileDescriptor, key: ByteArray? = null): GpackStreamReader {
            val fis = FileInputStream(afd.fileDescriptor)
            val channel = fis.channel
            val declaredLength = afd.declaredLength
            val source = ChannelSource(
                channel = channel,
                baseOffset = afd.startOffset,
                closeable = afd,
                declaredLength = if (declaredLength >= 0L) declaredLength else -1L
            )
            return open(source, key)
        }

        fun open(source: SeekableSource, key: ByteArray? = null): GpackStreamReader {
            try {
                val headerBuf = ByteArray(64)
                source.readFully(0L, headerBuf, 0, 64)

                if (!matchesMagic(headerBuf)) {
                    throw IllegalStateException("Invalid GPACK magic header")
                }

                val headerWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
                headerWrap.position(8)
                val version = headerWrap.int
                check(version == 1) { "Unsupported GPACK version: $version" }

                val flags = headerWrap.int
                val indexOffset = headerWrap.long
                val indexLength = headerWrap.long
                val fileCount = headerWrap.int

                val indexBytes = ByteArray(indexLength.toInt())
                source.readFully(indexOffset, indexBytes, 0, indexLength.toInt())

                val indexWrap = ByteBuffer.wrap(indexBytes).order(ByteOrder.LITTLE_ENDIAN)
                val map = HashMap<String, IndexEntry>(fileCount)

                for (i in 0 until fileCount) {
                    val pathLen = indexWrap.int
                    val pathBytes = ByteArray(pathLen)
                    indexWrap.get(pathBytes)
                    val rawPath = String(pathBytes, Charsets.UTF_8)
                    val logicalPath = rawPath.replace('\\', '/').trimStart('/')

                    val offset = indexWrap.long
                    val size = indexWrap.long
                    val storedSize = indexWrap.long
                    val hash = ByteArray(32)
                    indexWrap.get(hash)
                    val compression = indexWrap.get()
                    val encryption = indexWrap.get()

                    if (offset < 0L || size < 0L || storedSize < 0L) {
                        throw IllegalStateException("Negative offset or size in entry: $logicalPath")
                    }
                    if (Long.MAX_VALUE - offset < storedSize) {
                        throw IllegalStateException("Offset + storedSize overflow in entry: $logicalPath")
                    }
                    if (compression.toInt() != 0) {
                        throw IllegalStateException("Unsupported compression format: $compression in entry: $logicalPath")
                    }
                    if (encryption.toInt() !in 0..1) {
                        throw IllegalStateException("Unsupported encryption format: $encryption in entry: $logicalPath")
                    }
                    if (encryption.toInt() == 0 && size != storedSize) {
                        throw IllegalStateException("Raw entry size mismatch: size=$size storedSize=$storedSize in entry: $logicalPath")
                    }

                    val entry = IndexEntry(
                        index = i,
                        logicalPath = logicalPath,
                        offset = offset,
                        size = size,
                        storedSize = storedSize,
                        hash = hash,
                        compression = compression,
                        encryption = encryption
                    )
                    map[logicalPath.lowercase()] = entry
                }

                return GpackStreamReader(
                    source = source,
                    key = key,
                    version = version,
                    flags = flags,
                    indexOffset = indexOffset,
                    indexLength = indexLength,
                    fileCount = fileCount,
                    fileIndexMap = map
                )
            } catch (e: Exception) {
                try {
                    source.close()
                } catch (_: Exception) {
                }
                throw e
            }
        }
    }

    internal fun readFullyInternal(position: Long, buffer: ByteArray, offset: Int, length: Int) {
        if (isClosed) {
            throw IOException("GpackStreamReader is closed")
        }
        source.readFully(position, buffer, offset, length)
    }

    fun hasFile(path: String): Boolean {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        return fileIndexMap.containsKey(canonical)
    }

    fun getEntry(path: String): IndexEntry? {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        return fileIndexMap[canonical]
    }

    fun openStream(path: String): InputStream? {
        if (isClosed) {
            throw IOException("GpackStreamReader is closed")
        }
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        val entry = fileIndexMap[canonical] ?: return null

        if (entry.offset < 0L || entry.size < 0L || entry.storedSize < 0L) {
            throw IllegalStateException("Negative offset or size in entry: ${entry.logicalPath}")
        }
        if (Long.MAX_VALUE - entry.offset < entry.storedSize) {
            throw IllegalStateException("Offset + storedSize overflow in entry: ${entry.logicalPath}")
        }
        if (entry.compression.toInt() != 0) {
            throw IllegalStateException("Unsupported GPACK compression: ${entry.compression} in entry: ${entry.logicalPath}")
        }
        if (entry.encryption.toInt() !in 0..1) {
            throw IllegalStateException("Unsupported GPACK encryption: ${entry.encryption} in entry: ${entry.logicalPath}")
        }
        if (entry.encryption.toInt() == 0 && entry.size != entry.storedSize) {
            throw IllegalStateException("Raw entry size mismatch: size=${entry.size} storedSize=${entry.storedSize} in entry: ${entry.logicalPath}")
        }

        if (entry.encryption.toInt() == 1) {
            checkNotNull(key) { "Container chunk is encrypted but no key provided" }
            require(entry.storedSize <= Int.MAX_VALUE) { "File stored size exceeds supported 2GB buffer: ${entry.storedSize}" }
            val stored = ByteArray(entry.storedSize.toInt())
            source.readFully(entry.offset, stored, 0, stored.size)
            val plaintext = GpackCrypto.decryptChunk(key, 0, entry.index, stored)
            return plaintext.inputStream()
        }

        return BoundedEntryStream(this, entry)
    }

    override fun close() {
        isClosed = true
        source.close()
    }
}
