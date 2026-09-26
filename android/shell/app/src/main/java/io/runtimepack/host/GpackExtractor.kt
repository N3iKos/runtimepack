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

    fun isEncryptedStreamingMode(context: Context): Boolean {
        return try {
            val manifestJson = context.assets.open("build-manifest.json").bufferedReader().use { it.readText() }
            manifestJson.contains("\"mode\": \"encrypted-streaming\"") || manifestJson.contains("\"mode\":\"encrypted-streaming\"")
        } catch (e: Exception) {
            false
        }
    }

    fun prepareStreamingContainer(context: Context, destDir: File): Boolean {
        return try {
            destDir.mkdirs()
            val targetPack = File(destDir, "game.pack")
            if (!targetPack.exists()) {
                context.assets.open("game.pack").use { input ->
                    FileOutputStream(targetPack).use { output ->
                        input.copyTo(output)
                    }
                }
            }
            installRuntimeFiles(context, destDir)
            File(destDir, ".gpack_extracted").writeText(EXTRACTION_VERSION)
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to prepare streaming container: ${e.message}", e)
            false
        }
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

        try {
            context.assets.open("runtimepack_asset_audit.rb").use { auditInput ->
                FileOutputStream(File(destDir, "runtimepack_asset_audit.rb")).use { auditOutput ->
                    auditInput.copyTo(auditOutput)
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Could not copy runtimepack_asset_audit.rb from assets: ${e.message}")
        }

        val fontsDir = File(destDir, "Fonts")
        val fontFiles = if (fontsDir.exists()) {
            fontsDir.listFiles { _, name ->
                val l = name.lowercase()
                l.endsWith(".ttf") || l.endsWith(".otf") || l.endsWith(".ttc")
            } ?: emptyArray()
        } else {
            emptyArray()
        }

        if (!fontsDir.exists() || fontFiles.isEmpty()) {
            if (!fontsDir.exists()) {
                fontsDir.mkdirs()
            }
            try {
                context.assets.open("fonts/VL-Gothic-Regular.ttf").use { fontInput ->
                    FileOutputStream(File(fontsDir, "VL-Gothic-Regular.ttf")).use { fontOutput ->
                        fontInput.copyTo(fontOutput)
                    }
                }
                Log.i(TAG, "Provisioned fallback CJK font VL-Gothic-Regular.ttf into Fonts/")
            } catch (e: Exception) {
                Log.w(TAG, "Could not copy fallback font from assets: ${e.message}")
            }
        }

        var detectedExecName: String? = null
        val rtpList = mutableListOf<String>()
        val extraFontSubs = mutableListOf<String>()

        val mkxpConf = File(destDir, "mkxp.conf")
        if (mkxpConf.exists()) {
            try {
                mkxpConf.forEachLine { line ->
                    val trimmed = line.trim()
                    if (trimmed.startsWith("#") || !trimmed.contains("=")) return@forEachLine
                    val eqIdx = trimmed.indexOf('=')
                    val key = trimmed.substring(0, eqIdx).trim()
                    val value = trimmed.substring(eqIdx + 1).trim()
                    when (key) {
                        "execName" -> if (value.isNotEmpty()) detectedExecName = value
                        "RTP" -> if (value.isNotEmpty()) rtpList.add(value)
                        "fontSub" -> if (value.isNotEmpty()) extraFontSubs.add(value)
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Error reading mkxp.conf: ${e.message}")
            }
        }

        val archives = destDir.listFiles { _, name ->
            val l = name.lowercase()
            l.endsWith(".rgssad") || l.endsWith(".rgss2a") || l.endsWith(".rgss3a")
        } ?: emptyArray()

        if (detectedExecName == null && archives.isNotEmpty()) {
            val baseName = archives[0].name.substringBeforeLast('.')
            detectedExecName = baseName
        }

        val finalExecName = detectedExecName ?: "Game"

        extractArchiveSymlinks(destDir, finalExecName)

        // Ensure game / Game directory aliases exist for case-sensitive filesystems
        val uppercaseGameDir = File(destDir, "Game")
        val lowercaseGameDir = File(destDir, "game")
        if (lowercaseGameDir.exists() && !uppercaseGameDir.exists()) {
            try {
                android.system.Os.symlink(lowercaseGameDir.absolutePath, uppercaseGameDir.absolutePath)
                Log.i(TAG, "Symlinked game -> Game")
            } catch (e: Throwable) {
                Log.w(TAG, "Could not symlink game -> Game: ${e.message}")
            }
        } else if (uppercaseGameDir.exists() && !lowercaseGameDir.exists()) {
            try {
                android.system.Os.symlink(uppercaseGameDir.absolutePath, lowercaseGameDir.absolutePath)
                Log.i(TAG, "Symlinked Game -> game")
            } catch (e: Throwable) {
                Log.w(TAG, "Could not symlink Game -> game: ${e.message}")
            }
        }

        // Ensure Game.ini exists if custom ini present
        val gameIni = File(destDir, "Game.ini")
        if (!gameIni.exists() && detectedExecName != null) {
            val customIni = File(destDir, "$detectedExecName.ini")
            if (customIni.exists()) {
                try {
                    customIni.copyTo(gameIni, overwrite = false)
                } catch (e: Exception) {
                    Log.w(TAG, "Could not copy $customIni to Game.ini: ${e.message}")
                }
            }
        }

        val defaultFontSubs = mutableListOf(
            "UmePlus Gothic>VL Gothic",
            "VL PGothic>VL Gothic",
            "MS Gothic>VL Gothic",
            "MS Mincho>VL Gothic",
            "Noto Sans CJK TC Black>Noto Sans CJK TC",
            "Noto Sans CJK TC Medium>Noto Sans CJK TC",
            "Source Han Sans Bold>Source Han Sans",
            "Noto Sans LRPG Black>Noto Sans LRPG"
        )
        for (fs in extraFontSubs) {
            if (!defaultFontSubs.contains(fs)) {
                defaultFontSubs.add(fs)
            }
        }

        val rtpJson = if (rtpList.isNotEmpty()) {
            val items = rtpList.joinToString(",\n                    ") { "\"$it\"" }
            ",\n                \"RTP\": [\n                    $items\n                ]"
        } else {
            ""
        }

        val fontSubJson = defaultFontSubs.joinToString(",\n                    ") { "\"$it\"" }

        val mkxpConfig = """
            {
                "fixedAspectRatio": true,
                "smoothScaling": true,
                "pathCache": false,
                "subImageFix": true,
                "execName": "$finalExecName",
                "customScript": "runtimepack_bootstrap.rb"$rtpJson,
                "fontSub": [
                    $fontSubJson
                ]
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

    fun extractArchiveSymlinks(destDir: File, finalExecName: String = "Game") {
        val archives = destDir.listFiles { _, name ->
            val l = name.lowercase()
            l.endsWith(".rgssad") || l.endsWith(".rgss2a") || l.endsWith(".rgss3a")
        } ?: emptyArray()

        // Ensure Game.* and finalExecName.* archive aliases exist for mkxp-z fallback
        for (archive in archives) {
            val ext = archive.name.substring(archive.name.lastIndexOf('.'))
            val targetGame = File(destDir, "Game$ext")
            if (!targetGame.exists() && archive.name != targetGame.name) {
                try {
                    android.system.Os.symlink(archive.absolutePath, targetGame.absolutePath)
                    Log.i(TAG, "Symlinked ${archive.name} -> ${targetGame.name}")
                } catch (e: Throwable) {
                    // Symlink failed or not supported
                }
                if (!targetGame.exists()) {
                    try {
                        archive.copyTo(targetGame, overwrite = false)
                        Log.i(TAG, "Copied ${archive.name} -> ${targetGame.name}")
                    } catch (e2: Exception) {
                        Log.w(TAG, "Could not alias archive: ${e2.message}")
                    }
                }
            }

            if (finalExecName.isNotBlank() && finalExecName != "Game") {
                val targetExec = File(destDir, "$finalExecName$ext")
                if (!targetExec.exists() && archive.name != targetExec.name) {
                    try {
                        android.system.Os.symlink(archive.absolutePath, targetExec.absolutePath)
                        Log.i(TAG, "Symlinked ${archive.name} -> ${targetExec.name}")
                    } catch (e: Throwable) {
                        // Symlink failed or not supported
                    }
                    if (!targetExec.exists()) {
                        try {
                            archive.copyTo(targetExec, overwrite = false)
                            Log.i(TAG, "Copied ${archive.name} -> ${targetExec.name}")
                        } catch (e2: Exception) {
                            Log.w(TAG, "Could not alias archive: ${e2.message}")
                        }
                    }
                }
            }
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
