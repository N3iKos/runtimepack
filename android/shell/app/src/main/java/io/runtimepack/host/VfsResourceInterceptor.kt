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
                "png" -> "image/png"
                "jpg", "jpeg" -> "image/jpeg"
                "webp" -> "image/webp"
                "ogg" -> "audio/ogg"
                "mp3" -> "audio/mpeg"
                "wav" -> "audio/wav"
                "m4a" -> "audio/mp4"
                "js" -> "application/javascript"
                "json" -> "application/json"
                "css" -> "text/css"
                "html", "htm" -> "text/html"
                else -> "application/octet-stream"
            }
        }

        fun urlToLogicalPath(url: String): String? {
            val clean = url.substringBefore('?').substringBefore('#')
            if (clean.startsWith("https://game.local/")) {
                return clean.removePrefix("https://game.local/")
            }
            if (clean.startsWith("file:///android_asset/game/")) {
                return clean.removePrefix("file:///android_asset/game/")
            }
            val match = LOCALHOST_REGEX.find(clean)
            if (match != null) {
                return clean.substring(match.range.last + 1)
            }
            return null
        }
    }

    fun getMimeType(path: String): String = Companion.getMimeType(path)

    fun urlToLogicalPath(url: String): String? = Companion.urlToLogicalPath(url)

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
                        val mime = getMimeType(logicalPath)
                        return WebResourceResponse(mime, "UTF-8", ByteArrayInputStream(decrypted))
                    }
                }
            }
        }
        if (stream == null && logicalPath.endsWith(".rpgmvm")) {
            val oggEnc = logicalPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            val s = streamProvider(oggEnc)
            if (s != null) {
                return WebResourceResponse("audio/ogg", "UTF-8", s)
            }
        }
        if (stream == null && logicalPath.endsWith(".m4a")) {
            val oggEnc = logicalPath.removeSuffix(".m4a") + ".rpgmvo"
            val s = streamProvider(oggEnc)
            if (s != null) {
                if (rpgMakerKey != null) {
                    val decrypted = decryptRpgMakerAsset(s.readBytes(), rpgMakerKey)
                    if (decrypted != null) {
                        return WebResourceResponse("audio/ogg", "UTF-8", ByteArrayInputStream(decrypted))
                    }
                }
                return WebResourceResponse("audio/ogg", "UTF-8", s)
            }
            val oggPlain = logicalPath.removeSuffix(".m4a") + ".ogg"
            val sPlain = streamProvider(oggPlain)
            if (sPlain != null) {
                return WebResourceResponse("audio/ogg", "UTF-8", sPlain)
            }
        }
        if (stream == null) {
            return null
        }
        val mime = getMimeType(logicalPath)
        return WebResourceResponse(mime, "UTF-8", stream)
    }
}
