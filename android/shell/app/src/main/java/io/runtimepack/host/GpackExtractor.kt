package io.runtimepack.host

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

object GpackExtractor {
    private const val TAG = "GpackExtractor"
    private val GPACK_MAGIC = byteArrayOf('G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(), 'K'.code.toByte(), '1'.code.toByte(), 0, 0)

    fun extractIfNecessary(context: Context, destDir: File) {
        val markerFile = File(destDir, ".gpack_extracted")
        if (destDir.exists() && markerFile.exists()) {
            Log.i(TAG, "Game assets already extracted at: ${destDir.absolutePath}")
            return
        }

        destDir.mkdirs()
        Log.i(TAG, "Extracting game.pack to: ${destDir.absolutePath} ...")

        val tempPackFile = File(context.cacheDir, "temp_game.pack")
        try {
            // Copy game.pack from assets to temporary random-access file
            context.assets.open("game.pack").use { input ->
                FileOutputStream(tempPackFile).use { output ->
                    val buffer = ByteArray(65536)
                    var read: Int
                    while (input.read(buffer).also { read = it } != -1) {
                        output.write(buffer, 0, read)
                    }
                }
            }

            RandomAccessFile(tempPackFile, "r").use { raf ->
                val headerBuf = ByteArray(64)
                raf.readFully(headerBuf)

                for (i in 0 until 8) {
                    if (headerBuf[i] != GPACK_MAGIC[i]) {
                        throw IllegalStateException("Invalid GPACK magic")
                    }
                }

                val headerWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
                headerWrap.position(8)
                val version = headerWrap.int
                val flags = headerWrap.int
                val indexOffset = headerWrap.long
                val indexLength = headerWrap.long
                val fileCount = headerWrap.int

                Log.i(TAG, "GPACK v$version, files: $fileCount, indexOffset: $indexOffset")

                raf.seek(indexOffset)
                val indexData = ByteArray(indexLength.toInt())
                raf.readFully(indexData)

                val indexWrap = ByteBuffer.wrap(indexData).order(ByteOrder.LITTLE_ENDIAN)
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

                    // Sanitize path against traversal
                    val outFile = File(destDir, logicalPath)
                    if (!outFile.canonicalPath.startsWith(destDir.canonicalPath)) {
                        Log.w(TAG, "Path traversal rejected for: $logicalPath")
                        continue
                    }

                    outFile.parentFile?.mkdirs()

                    // Read content
                    val currentPos = raf.filePointer
                    raf.seek(offset)
                    val content = ByteArray(storedSize.toInt())
                    raf.readFully(content)
                    raf.seek(currentPos)

                    FileOutputStream(outFile).use { fos ->
                        fos.write(content)
                    }
                }

                markerFile.writeText("extracted_ok")
                Log.i(TAG, "Successfully extracted $fileCount files from game.pack")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to extract game.pack: ${e.message}", e)
        } finally {
            if (tempPackFile.exists()) {
                tempPackFile.delete()
            }
        }
    }
}
