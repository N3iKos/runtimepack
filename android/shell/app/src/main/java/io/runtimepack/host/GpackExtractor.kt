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
    private const val EXTRACTION_VERSION = "v3"
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
        return destDir.exists() && marker.exists() && marker.readText().trim() == EXTRACTION_VERSION && ini.exists()
    }

    fun patchJsonDecoderScript(destDir: File) {
        val target = File(destDir, "Data/Scripts/101_jsonEnDecoder.rb")
        if (!target.exists()) return

        try {
            val content = target.readText(Charsets.UTF_8)

            val iterativeNextToken = """
  def self.next_token(debug = 0)
    loop do
      return TOKEN_NONE if @index >= @length
      char = @json[@index, 1]
      @index += 1
      case char
      when '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '.'
        return TOKEN_NUMBER
      when '{'
        return TOKEN_CURLY_OPEN
      when '}'
        return TOKEN_CURLY_CLOSED
      when '"'
        return TOKEN_STRING
      when ','
        return TOKEN_COMMA
      when '['
        return TOKEN_SQUARED_OPEN
      when ']'
        return TOKEN_SQUARED_CLOSED
      when ':'
        return TOKEN_COLON
      when ' ', "\n", "\t", "\r"
        # continue loop without recursion
      when '/'
        if @json[@index, 1] == '/'
          while @index < @length && @json[@index] != "\n"
            @index += 1
          end
          # continue loop without recursion
        else
          @index -= 1
          return TOKEN_NONE
        end
      else
        @index -= 1
        if @json[@index, 5] == "false"
          @index += 5
          return TOKEN_FALSE
        elsif @json[@index, 4] == "true"
          @index += 4
          return TOKEN_TRUE
        elsif @json[@index, 4] == "null"
          @index += 4
          return TOKEN_NULL
        end
        return TOKEN_NONE
      end
    end
  end
""".trimIndent()

            val methodStart = content.indexOf("\n  def self.next_token(debug = 0)")
            val nextMethod = content.indexOf("\n  def self.parse(debug = 0)", methodStart + 1)
            if (methodStart < 0 || nextMethod < 0) {
                throw IllegalStateException("Could not locate active JSON.next_token method boundaries")
            }

            val patched = content.substring(0, methodStart + 1) +
                iterativeNextToken + "\n\n" +
                content.substring(nextMethod + 1)
            target.writeText(patched, Charsets.UTF_8)
            Log.i(TAG, "Patched 101_jsonEnDecoder.rb with non-recursive next_token.")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to patch 101_jsonEnDecoder.rb: ${e.message}")
        }
    }

    fun patchErrorLogScript(destDir: File) {
        val target = File(destDir, "Data/Scripts/Frames/208_Han_Nama_ErrorLog.rb")
        if (!target.exists()) return

        try {
            val content = target.readText(Charsets.UTF_8)
            val patched = content.replace("open(filename,\"w\")", "open(filename,\"wb\")")
            if (patched != content) {
                target.writeText(patched, Charsets.UTF_8)
                Log.i(TAG, "Patched 208_Han_Nama_ErrorLog.rb to write in binary mode (wb).")
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to patch 208_Han_Nama_ErrorLog.rb: ${e.message}")
        }
    }

    fun installRuntimeFiles(context: Context, destDir: File) {
        patchJsonDecoderScript(destDir)
        patchErrorLogScript(destDir)

        try {
            context.assets.open("runtimepack_bootstrap.rb").use { bootstrapInput ->
                FileOutputStream(File(destDir, "runtimepack_bootstrap.rb")).use { bootstrapOutput ->
                    bootstrapInput.copyTo(bootstrapOutput)
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Could not copy runtimepack_bootstrap.rb from assets: ${e.message}")
        }

        val mkxpConfig = """
            {
                "fixedAspectRatio": true,
                "smoothScaling": true,
                "pathCache": true,
                "subImageFix": true,
                "execName": "Game",
                "customScript": "runtimepack_bootstrap.rb"
            }
        """.trimIndent()
        File(destDir, "mkxp.json").writeText(mkxpConfig)
    }

    fun extract(context: Context, destDir: File, callback: ProgressCallback? = null): Boolean {
        if (destDir.exists()) {
            destDir.deleteRecursively()
        }
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

            // 4. Install/update runtime files
            installRuntimeFiles(context, destDir)

            File(destDir, ".gpack_extracted").writeText(EXTRACTION_VERSION)
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
