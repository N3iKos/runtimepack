package io.runtimepack.host

import android.content.res.AssetFileDescriptor
import java.io.Closeable
import java.io.EOFException
import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.channels.FileChannel

class GpackStreamReader private constructor(
    private val source: SeekableSource,
    val key: ByteArray?,
    val version: Int,
    val flags: Int,
    val indexOffset: Long,
    val indexLength: Long,
    val fileCount: Int,
    private val fileIndexMap: Map<String, IndexEntry>
) : AutoCloseable, Closeable {

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

    private class RafSource(private val raf: RandomAccessFile) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
            synchronized(raf) {
                raf.seek(position)
                raf.readFully(buffer, offset, length)
            }
        }

        override fun close() {
            raf.close()
        }
    }

    private class ChannelSource(
        private val channel: FileChannel,
        private val baseOffset: Long = 0L,
        private val closeable: AutoCloseable? = null
    ) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
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
                channel.close()
            } finally {
                closeable?.close()
            }
        }
    }

    private class ByteArraySource(private val data: ByteArray) : SeekableSource {
        override fun readFully(position: Long, buffer: ByteArray, offset: Int, length: Int) {
            val pos = position.toInt()
            if (position < 0 || position > data.size || pos + length > data.size) {
                throw EOFException("Invalid position $position or length $length for byte array of size ${data.size}")
            }
            System.arraycopy(data, pos, buffer, offset, length)
        }

        override fun close() {
            // In-memory byte array does not need explicit cleanup
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
            val source = ChannelSource(channel, afd.startOffset, afd)
            return open(source, key)
        }

        fun open(source: SeekableSource, key: ByteArray? = null): GpackStreamReader {
            try {
                // 1. Read 64-byte header
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

                // 2. Read Index Table
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

    fun hasFile(path: String): Boolean {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        return fileIndexMap.containsKey(canonical)
    }

    fun getEntry(path: String): IndexEntry? {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        return fileIndexMap[canonical]
    }

    fun openStream(path: String): InputStream? {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        val entry = fileIndexMap[canonical] ?: return null

        require(entry.storedSize <= Int.MAX_VALUE) { "File stored size exceeds supported 2GB buffer: ${entry.storedSize}" }
        val stored = ByteArray(entry.storedSize.toInt())
        source.readFully(entry.offset, stored, 0, stored.size)

        val plaintext = if (entry.encryption.toInt() == 1) {
            checkNotNull(key) { "Container chunk is encrypted but no key provided" }
            GpackCrypto.decryptChunk(key, 0, entry.index, stored)
        } else {
            stored
        }

        return plaintext.inputStream()
    }

    override fun close() {
        source.close()
    }
}
