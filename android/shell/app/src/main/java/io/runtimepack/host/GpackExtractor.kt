package io.runtimepack.host

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

object GpackExtractor {
    private const val TAG = "GpackExtractor"
    private val GPACK_MAGIC = byteArrayOf('G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(), 'K'.code.toByte(), '1'.code.toByte(), 0, 0)

    fun interface ProgressCallback {
        fun onProgress(current: Int, total: Int, fileName: String)
    }

    data class ExtractedEntry(
        val logicalPath: String,
        val storedSize: Long
    )

    fun isExtracted(destDir: File): Boolean {
        val marker = File(destDir, ".gpack_extracted")
        val ini = File(destDir, "Game.ini")
        return destDir.exists() && marker.exists() && ini.exists()
    }

    fun extract(context: Context, destDir: File, callback: ProgressCallback? = null): Boolean {
        destDir.mkdirs()
        Log.i(TAG, "Extracting game.pack into ${destDir.absolutePath}...")

        var inputStream: InputStream? = null
        try {
            inputStream = context.assets.open("game.pack")

            // 1. Read 64-byte header
            val headerBuf = ByteArray(64)
            readFully(inputStream, headerBuf)

            for (i in 0 until 8) {
                if (headerBuf[i] != GPACK_MAGIC[i]) {
                    throw IllegalStateException("Invalid GPACK magic header")
                }
            }

            val headerWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
            headerWrap.position(8)
            val version = headerWrap.int
            val flags = headerWrap.int
            val indexOffset = headerWrap.long
            val indexLength = headerWrap.long
            val fileCount = headerWrap.int

            Log.i(TAG, "GPACK v$version, $fileCount files, indexLength: $indexLength")

            // 2. Read index table immediately (it is at the front)
            val indexData = ByteArray(indexLength.toInt())
            readFully(inputStream, indexData)

            val indexWrap = ByteBuffer.wrap(indexData).order(ByteOrder.LITTLE_ENDIAN)
            val entries = ArrayList<ExtractedEntry>(fileCount)

            for (i in 0 until fileCount) {
                val pathLen = indexWrap.int
                val pathBytes = ByteArray(pathLen)
                indexWrap.get(pathBytes)
                val logicalPath = String(pathBytes, Charsets.UTF_8).replace('\\', '/')

                val offset = indexWrap.long
                val size = indexWrap.long
                val storedSize = indexWrap.long
                val hash = ByteArray(32)
                indexWrap.get(hash)
                val compression = indexWrap.get()
                val encryption = indexWrap.get()

                entries.add(ExtractedEntry(logicalPath, storedSize))
            }

            // 3. Sequentially stream each file directly into destDir
            val buffer = ByteArray(65536)
            for (i in 0 until fileCount) {
                val entry = entries[i]
                val outFile = File(destDir, entry.logicalPath)

                // Path traversal protection
                if (!outFile.canonicalPath.startsWith(destDir.canonicalPath)) {
                    skipBytes(inputStream, entry.storedSize)
                    continue
                }

                outFile.parentFile?.mkdirs()

                FileOutputStream(outFile).use { fos ->
                    var remaining = entry.storedSize
                    while (remaining > 0) {
                        val toRead = if (remaining < buffer.size) remaining.toInt() else buffer.size
                        val read = inputStream.read(buffer, 0, toRead)
                        if (read == -1) {
                            throw IllegalStateException("Unexpected EOF while extracting ${entry.logicalPath}")
                        }
                        fos.write(buffer, 0, read)
                        remaining -= read
                    }
                }

                callback?.onProgress(i + 1, fileCount, entry.logicalPath)
            }

            // 4. Install Win32API shim and mkxp.json
            try {
                context.assets.open("win32api_shim.rb").use { shimInput ->
                    FileOutputStream(File(destDir, "win32api_shim.rb")).use { shimOutput ->
                        shimInput.copyTo(shimOutput)
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Could not copy win32api_shim.rb from assets: ${e.message}")
            }

            val mkxpConfig = """
                {
                    "fixedAspectRatio": true,
                    "smoothScaling": true,
                    "pathCache": true,
                    "subImageFix": true,
                    "execName": "Game",
                    "preloadScript": ["win32api_shim.rb"]
                }
            """.trimIndent()
            File(destDir, "mkxp.json").writeText(mkxpConfig)

            File(destDir, ".gpack_extracted").writeText("ok")
            Log.i(TAG, "Successfully extracted all $fileCount files to ${destDir.absolutePath}")
            return true
        } catch (e: Exception) {
            Log.e(TAG, "Extraction failed: ${e.message}", e)
            return false
        } finally {
            try {
                inputStream?.close()
            } catch (_: Exception) {}
        }
    }

    private fun readFully(stream: InputStream, buffer: ByteArray) {
        var offset = 0
        while (offset < buffer.size) {
            val read = stream.read(buffer, offset, buffer.size - offset)
            if (read == -1) {
                throw IllegalStateException("Unexpected EOF")
            }
            offset += read
        }
    }

    private fun skipBytes(stream: InputStream, count: Long) {
        var remaining = count
        val skipBuffer = ByteArray(4096)
        while (remaining > 0) {
            val toRead = if (remaining < skipBuffer.size) remaining.toInt() else skipBuffer.size
            val read = stream.read(skipBuffer, 0, toRead)
            if (read == -1) break
            remaining -= read
        }
    }
}
