package io.runtimepack.host

import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream
import java.io.InputStream
import java.net.URLDecoder

class VfsResourceInterceptor(
    val reader: GpackStreamReader?,
    val rpgMakerKey: String? = null,
    private val streamProvider: (String) -> InputStream?
) {
    constructor(reader: GpackStreamReader) : this(reader, null, { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?) : this(reader, rpgMakerKey, { path -> reader.openStream(path) })

    constructor(streamProvider: (String) -> InputStream?) : this(null, null, streamProvider)

    companion object {
        private val LOCALHOST_REGEX = Regex("""^http://(?:localhost|127\.0\.0\.1)(?::\d+)?/""")

        val TRANSPARENT_1X1_PNG: ByteArray = byteArrayOf(
            0x89.toByte(), 0x50.toByte(), 0x4E.toByte(), 0x47.toByte(),
            0x0D.toByte(), 0x0A.toByte(), 0x1A.toByte(), 0x0A.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x00.toByte(), 0x0D.toByte(),
            0x49.toByte(), 0x48.toByte(), 0x44.toByte(), 0x52.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x00.toByte(), 0x01.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x00.toByte(), 0x01.toByte(),
            0x08.toByte(), 0x06.toByte(), 0x00.toByte(), 0x00.toByte(),
            0x00.toByte(), 0x1F.toByte(), 0x15.toByte(), 0xC4.toByte(),
            0x89.toByte(), 0x00.toByte(), 0x00.toByte(), 0x00.toByte(),
            0x0D.toByte(), 0x49.toByte(), 0x44.toByte(), 0x41.toByte(),
            0x54.toByte(), 0x78.toByte(), 0x9C.toByte(), 0x63.toByte(),
            0x60.toByte(), 0x60.toByte(), 0x60.toByte(), 0x60.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x00.toByte(), 0x05.toByte(),
            0x00.toByte(), 0x01.toByte(), 0xA5.toByte(), 0xF6.toByte(),
            0x45.toByte(), 0x40.toByte(), 0x00.toByte(), 0x00.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x49.toByte(), 0x45.toByte(),
            0x4E.toByte(), 0x44.toByte(), 0xAE.toByte(), 0x42.toByte(),
            0x60.toByte(), 0x82.toByte()
        )

        val EMPTY_AUDIO: ByteArray = ByteArray(0)

        private val IMAGE_EXTENSIONS = setOf("png", "jpg", "jpeg", "webp", "gif")
        private val AUDIO_EXTENSIONS = setOf("ogg", "m4a", "mp3", "wav")

        private val RPGMV_HEADER = byteArrayOf(
            0x52.toByte(), 0x50.toByte(), 0x47.toByte(), 0x4d.toByte(),
            0x56.toByte(), 0x00, 0x00, 0x00,
            0x00, 0x03, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00
        )

        fun decryptRpgMakerAsset(encryptedBytes: ByteArray, hexKey: String): ByteArray? {
            if (encryptedBytes.size < 16 || hexKey.length < 32) return null
            val isHeaderMatch = (0 until 16).all { encryptedBytes[it] == RPGMV_HEADER[it] }
            if (!isHeaderMatch) return null

            val bodyLen = encryptedBytes.size - 16
            val result = ByteArray(bodyLen)
            val keyBytes = ByteArray(16)
            for (i in 0 until 16) {
                val hexByte = hexKey.substring(i * 2, i * 2 + 2)
                keyBytes[i] = hexByte.toInt(16).toByte()
            }
            val xorCount = if (bodyLen < 16) bodyLen else 16
            for (i in 0 until xorCount) {
                result[i] = (encryptedBytes[16 + i].toInt() xor keyBytes[i].toInt()).toByte()
            }
            if (bodyLen > 16) {
                System.arraycopy(encryptedBytes, 32, result, 16, bodyLen - 16)
            }
            return result
        }

        fun getMimeType(path: String): String {
            val cleanPath = path.substringBefore('?').substringBefore('#')
            val ext = cleanPath.substringAfterLast('.', "").lowercase()
            return when (ext) {
                "png", "png_" -> "image/png"
                "jpg", "jpeg", "jpg_", "jpeg_" -> "image/jpeg"
                "gif", "gif_" -> "image/gif"
                "webp", "webp_" -> "image/webp"
                "svg" -> "image/svg+xml"
                "ogg", "ogg_" -> "audio/ogg"
                "mp3", "mp3_" -> "audio/mpeg"
                "wav", "wav_" -> "audio/wav"
                "m4a", "m4a_" -> "audio/mp4"
                "js" -> "application/javascript"
                "json" -> "application/json"
                "css" -> "text/css"
                "html", "htm" -> "text/html"
                "ks", "tjs", "txt" -> "text/plain"
                "woff" -> "font/woff"
                "woff2" -> "font/woff2"
                "ttf" -> "font/ttf"
                "otf" -> "font/otf"
                else -> "application/octet-stream"
            }
        }

        fun getEncoding(mimeType: String): String? {
            return if (mimeType.startsWith("text/") ||
                mimeType == "application/javascript" ||
                mimeType == "application/json") {
                "UTF-8"
            } else {
                null
            }
        }

        fun urlToLogicalPath(url: String): String? {
            val clean = url.substringBefore('?').substringBefore('#')
            val rawPath = if (clean.startsWith("https://game.local/")) {
                clean.removePrefix("https://game.local/")
            } else if (clean.startsWith("file:///android_asset/game/")) {
                clean.removePrefix("file:///android_asset/game/")
            } else {
                val match = LOCALHOST_REGEX.find(clean)
                if (match != null) {
                    clean.substring(match.range.last + 1)
                } else {
                    return null
                }
            }

            val normalizedParts = mutableListOf<String>()
            for (part in rawPath.replace('\\', '/').split('/')) {
                if (part.isEmpty() || part == ".") {
                    continue
                }
                if (part == "..") {
                    if (normalizedParts.isEmpty()) {
                        return null
                    }
                    normalizedParts.removeAt(normalizedParts.size - 1)
                } else {
                    normalizedParts.add(part)
                }
            }
            return normalizedParts.joinToString("/")
        }
    }

    fun getMimeType(path: String): String = Companion.getMimeType(path)

    fun urlToLogicalPath(url: String): String? = Companion.urlToLogicalPath(url)

    private fun recordResourceServed(path: String) {
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = path,
            outcome = RuntimeAuditOutcome.PASS
        )
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_READABLE,
            assetPath = path,
            outcome = RuntimeAuditOutcome.PASS
        )
    }

    fun intercept(url: String): WebResourceResponse? {
        val logicalPath = urlToLogicalPath(url) ?: return null
        var stream = streamProvider(logicalPath)
        if (stream == null && logicalPath.contains('%')) {
            val decoded = try {
                URLDecoder.decode(logicalPath.replace("+", "%2B"), "UTF-8")
            } catch (_: Exception) {
                null
            }
            if (decoded != null) {
                stream = streamProvider(decoded)
            }
        }
        if (stream == null && (logicalPath.startsWith("js/plugins/") || logicalPath.startsWith("www/js/plugins/")) && logicalPath.endsWith(".js")) {
            val filename = logicalPath.substringAfterLast('/')
            if (reader != null) {
                val matchedEntry = reader.entries.firstOrNull { entry ->
                    val norm = entry.logicalPath.replace('\\', '/').removePrefix("www/")
                    norm.startsWith("js/plugins/") && norm.endsWith("/$filename")
                } ?: reader.entries.firstOrNull { entry ->
                    val norm = entry.logicalPath.replace('\\', '/').removePrefix("www/")
                    norm.startsWith("js/plugins/") && norm.substringAfterLast('/').equals(filename, ignoreCase = true)
                }
                if (matchedEntry != null) {
                    stream = streamProvider(matchedEntry.logicalPath)
                }
            }
        }
        if (stream == null && rpgMakerKey != null) {
            val ext = logicalPath.substringAfterLast('.', "").lowercase()
            val encExt = when (ext) {
                "png" -> "rpgmvp"
                "ogg" -> "rpgmvo"
                "m4a" -> "rpgmvm"
                else -> null
            }
            if (encExt != null) {
                val encPath = logicalPath.substringBeforeLast('.') + "." + encExt
                val encStream = streamProvider(encPath)
                if (encStream != null) {
                    val encBytes = encStream.readBytes()
                    val decrypted = decryptRpgMakerAsset(encBytes, rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(logicalPath)
                        val mime = getMimeType(logicalPath)
                        return WebResourceResponse(mime, getEncoding(mime), ByteArrayInputStream(decrypted))
                    }
                }
            }
        }
        if (stream == null && logicalPath.endsWith(".rpgmvm")) {
            val oggEnc = logicalPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            val s = streamProvider(oggEnc)
            if (s != null) {
                recordResourceServed(logicalPath)
                return WebResourceResponse("audio/ogg", null, s)
            }
        }
        if (stream == null && (logicalPath.endsWith(".m4a") || logicalPath.endsWith(".m4a_"))) {
            val base = if (logicalPath.endsWith(".m4a_")) logicalPath.removeSuffix(".m4a_") else logicalPath.removeSuffix(".m4a")
            val oggEnc = base + ".ogg_"
            val sEnc = streamProvider(oggEnc)
            if (sEnc != null) {
                recordResourceServed(logicalPath)
                return WebResourceResponse("audio/ogg", null, sEnc)
            }
            val oggMvEnc = base + ".rpgmvo"
            val sMvEnc = streamProvider(oggMvEnc)
            if (sMvEnc != null) {
                if (rpgMakerKey != null) {
                    val decrypted = decryptRpgMakerAsset(sMvEnc.readBytes(), rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(logicalPath)
                        return WebResourceResponse("audio/ogg", null, ByteArrayInputStream(decrypted))
                    }
                }
                recordResourceServed(logicalPath)
                return WebResourceResponse("audio/ogg", null, sMvEnc)
            }
            val oggPlain = base + ".ogg"
            val sPlain = streamProvider(oggPlain)
            if (sPlain != null) {
                recordResourceServed(logicalPath)
                return WebResourceResponse("audio/ogg", null, sPlain)
            }
        }
        if (stream == null && (logicalPath.endsWith(".ogg") || logicalPath.endsWith(".png"))) {
            val encPath = logicalPath + "_"
            val s = streamProvider(encPath)
            if (s != null) {
                recordResourceServed(logicalPath)
                val mime = getMimeType(logicalPath)
                return WebResourceResponse(mime, getEncoding(mime), s)
            }
        }
        if (stream == null) {
            val cleanExt = logicalPath.substringAfterLast('.', "").lowercase().removeSuffix("_")
            if (cleanExt in IMAGE_EXTENSIONS) {
                AssetAuditService.recordEvent(
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    assetPath = logicalPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    fallbackAction = "PLACEHOLDER_SUBSTITUTED",
                    errorMessage = "Resource missing in VFS, transparent placeholder substituted"
                )
                return WebResourceResponse("image/png", null, ByteArrayInputStream(TRANSPARENT_1X1_PNG))
            }
            if (cleanExt in AUDIO_EXTENSIONS) {
                val mime = getMimeType(logicalPath)
                AssetAuditService.recordEvent(
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    assetPath = logicalPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    fallbackAction = "PLACEHOLDER_SUBSTITUTED",
                    errorMessage = "Resource missing in VFS, transparent placeholder substituted"
                )
                return WebResourceResponse(mime, null, ByteArrayInputStream(EMPTY_AUDIO))
            }
            return null
        }
        recordResourceServed(logicalPath)
        val mime = getMimeType(logicalPath)
        return WebResourceResponse(mime, getEncoding(mime), stream)
    }
}
