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
                        return WebResourceResponse(mime, getEncoding(mime), ByteArrayInputStream(decrypted))
                    }
                }
            }
        }
        if (stream == null && logicalPath.endsWith(".rpgmvm")) {
            val oggEnc = logicalPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            val s = streamProvider(oggEnc)
            if (s != null) {
                return WebResourceResponse("audio/ogg", null, s)
            }
        }
        if (stream == null && (logicalPath.endsWith(".m4a") || logicalPath.endsWith(".m4a_"))) {
            val base = if (logicalPath.endsWith(".m4a_")) logicalPath.removeSuffix(".m4a_") else logicalPath.removeSuffix(".m4a")
            val oggEnc = base + ".ogg_"
            val sEnc = streamProvider(oggEnc)
            if (sEnc != null) {
                return WebResourceResponse("audio/ogg", null, sEnc)
            }
            val oggMvEnc = base + ".rpgmvo"
            val sMvEnc = streamProvider(oggMvEnc)
            if (sMvEnc != null) {
                if (rpgMakerKey != null) {
                    val decrypted = decryptRpgMakerAsset(sMvEnc.readBytes(), rpgMakerKey)
                    if (decrypted != null) {
                        return WebResourceResponse("audio/ogg", null, ByteArrayInputStream(decrypted))
                    }
                }
                return WebResourceResponse("audio/ogg", null, sMvEnc)
            }
            val oggPlain = base + ".ogg"
            val sPlain = streamProvider(oggPlain)
            if (sPlain != null) {
                return WebResourceResponse("audio/ogg", null, sPlain)
            }
        }
        if (stream == null && (logicalPath.endsWith(".ogg") || logicalPath.endsWith(".png"))) {
            val encPath = logicalPath + "_"
            val s = streamProvider(encPath)
            if (s != null) {
                val mime = getMimeType(logicalPath)
                return WebResourceResponse(mime, getEncoding(mime), s)
            }
        }
        if (stream == null) {
            return null
        }
        val mime = getMimeType(logicalPath)
        return WebResourceResponse(mime, getEncoding(mime), stream)
    }
}
