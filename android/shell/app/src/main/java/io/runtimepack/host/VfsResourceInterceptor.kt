package io.runtimepack.host

import android.webkit.WebResourceResponse
import java.io.InputStream
import java.net.URLDecoder

class VfsResourceInterceptor(
    val reader: GpackStreamReader?,
    private val streamProvider: (String) -> InputStream?
) {
    constructor(reader: GpackStreamReader) : this(reader, { path -> reader.openStream(path) })

    constructor(streamProvider: (String) -> InputStream?) : this(null, streamProvider)

    companion object {
        private val LOCALHOST_REGEX = Regex("""^http://(?:localhost|127\.0\.0\.1)(?::\d+)?/""")

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
        if (stream == null) {
            return null
        }
        val mime = getMimeType(logicalPath)
        return WebResourceResponse(mime, "UTF-8", stream)
    }
}
