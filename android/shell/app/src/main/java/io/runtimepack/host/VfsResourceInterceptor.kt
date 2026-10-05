package io.runtimepack.host

import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream
import java.io.InputStream
import java.net.URLDecoder

class VfsResourceInterceptor(
    val reader: GpackStreamReader?,
    val rpgMakerKey: String? = null,
    val engineFamily: String? = null,
    val vnmInputGuard: String = "off",
    private val streamProvider: (String) -> InputStream?
) {
    constructor(reader: GpackStreamReader) : this(reader, null, null, "off", { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?) : this(reader, rpgMakerKey, null, "off", { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?, engineFamily: String?) : this(reader, rpgMakerKey, engineFamily, "off", { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?, engineFamily: String?, vnmInputGuard: String) : this(reader, rpgMakerKey, engineFamily, vnmInputGuard, { path -> reader.openStream(path) })

    constructor(streamProvider: (String) -> InputStream?) : this(null, null, null, "off", streamProvider)

    constructor(engineFamily: String?, streamProvider: (String) -> InputStream?) : this(null, null, engineFamily, "off", streamProvider)

    constructor(engineFamily: String?, vnmInputGuard: String, streamProvider: (String) -> InputStream?) : this(null, null, engineFamily, vnmInputGuard, streamProvider)

    fun isVnmProfile(): Boolean = Companion.isVnmProfile(engineFamily)

    fun isAllowlistedShim(path: String): Boolean {
        if (path == "web_shims/runtimepack_web_shims.js") return true
        if (path == "web_shims/runtimepack_vnm_input_guard.js") {
            return isVnmProfile() && vnmInputGuard == "transition-aware"
        }
        return false
    }

    fun transformVnmHtml(rawHtml: String, guardMode: String = this.vnmInputGuard): String {
        if (rawHtml.contains("<!-- RUNTIMEPACK_VNM_SHIMS -->")) {
            return rawHtml
        }

        val scriptsRegex = Regex("""<script[^>]*src\s*=\s*["'](?:\./)?data/SCRIPTS\.js["'][^>]*>\s*</script>""", RegexOption.IGNORE_CASE)
        val scriptsMatch = scriptsRegex.find(rawHtml)
            ?: throw IllegalArgumentException("[RuntimePack] VNM expected script anchor not found in index.html: data/SCRIPTS.js")

        if (scriptsMatch.value.contains("async", ignoreCase = true) || scriptsMatch.value.contains("defer", ignoreCase = true)) {
            throw IllegalArgumentException("[RuntimePack] VNM unsupported async/defer script tag found on critical engine scripts")
        }

        val engineRegex = Regex("""<script[^>]*src\s*=\s*["'](?:\./)?data/ENGINE\.js["'][^>]*>\s*</script>""", RegexOption.IGNORE_CASE)
        val engineMatch = engineRegex.find(rawHtml)
        if (engineMatch != null && (engineMatch.value.contains("async", ignoreCase = true) || engineMatch.value.contains("defer", ignoreCase = true))) {
            throw IllegalArgumentException("[RuntimePack] VNM unsupported async/defer script tag found on critical engine scripts")
        }

        val boundary1Tag = "<!-- RUNTIMEPACK_VNM_SHIMS -->\n    <script type=\"text/javascript\" src=\"web_shims/runtimepack_web_shims.js\"></script>\n"
        val postEngineTag = "\n    <script type=\"text/javascript\">if (window.__runtimepackPollVnmStorage) { window.__runtimepackPollVnmStorage(window.GameStorage || (window.gs && window.gs.GameStorage)); }</script>"
        val boundary2Tag = "\n    <script type=\"text/javascript\">if (window.__runtimepackPollVnmMessageCompat) { window.__runtimepackPollVnmMessageCompat(); } if (window.__runtimepackPollVnmStorage) { window.__runtimepackPollVnmStorage(window.GameStorage || (window.gs && window.gs.GameStorage)); } if (window.__runtimepackPollVnmAudioCompat) { window.__runtimepackPollVnmAudioCompat(); }</script>"
        val guardTag = if (guardMode == "transition-aware") {
            "\n    <script type=\"text/javascript\">window.__runtimepack_vnm_input_guard_mode = \"transition-aware\";</script>\n    <script type=\"text/javascript\" src=\"web_shims/runtimepack_vnm_input_guard.js\"></script>"
        } else {
            ""
        }

        val platformLibsRegex = Regex("""<!--\{PLATFORM_LIBS\}-->""", RegexOption.IGNORE_CASE)
        val htmlWithBoundary1 = if (platformLibsRegex.containsMatchIn(rawHtml)) {
            rawHtml.replace(platformLibsRegex, "$boundary1Tag    <!--{PLATFORM_LIBS}-->")
        } else {
            val firstScriptRegex = Regex("""<script\b""", RegexOption.IGNORE_CASE)
            val firstScriptMatch = firstScriptRegex.find(rawHtml)
            if (firstScriptMatch != null) {
                rawHtml.substring(0, firstScriptMatch.range.first) + boundary1Tag + "    " + rawHtml.substring(firstScriptMatch.range.first)
            } else {
                throw IllegalArgumentException("[RuntimePack] VNM expected script anchor not found in index.html: no script tags")
            }
        }

        val engineMatchAfterB1 = engineRegex.find(htmlWithBoundary1)
        val htmlWithPostEngine = if (engineMatchAfterB1 != null) {
            val engineEnd = engineMatchAfterB1.range.last + 1
            htmlWithBoundary1.substring(0, engineEnd) + postEngineTag + htmlWithBoundary1.substring(engineEnd)
        } else {
            htmlWithBoundary1
        }

        val scriptsMatchAfterPostEngine = scriptsRegex.find(htmlWithPostEngine)
            ?: throw IllegalArgumentException("[RuntimePack] VNM expected script anchor not found in index.html: data/SCRIPTS.js")

        val endIdx = scriptsMatchAfterPostEngine.range.last + 1
        return htmlWithPostEngine.substring(0, endIdx) + boundary2Tag + guardTag + htmlWithPostEngine.substring(endIdx)
    }

    private val pluginFilenameMap: Map<String, String> by lazy {
        val map = HashMap<String, String>()
        reader?.entries?.forEach { entry ->
            val norm = entry.logicalPath.replace('\\', '/').removePrefix("www/")
            if (norm.startsWith("js/plugins/") && norm.endsWith(".js")) {
                val filename = norm.substringAfterLast('/').lowercase()
                map.putIfAbsent(filename, entry.logicalPath)
            }
        }
        map
    }

    private fun createResponse(mime: String, encoding: String?, stream: InputStream?): WebResourceResponse {
        return object : WebResourceResponse(mime, encoding, stream) {
            override fun getMimeType(): String = mime
            override fun getEncoding(): String? = encoding
            override fun getData(): InputStream? = stream
        }
    }

    companion object {
        fun isVnmProfile(engineFamily: String?): Boolean {
            val raw = engineFamily?.trim()?.lowercase() ?: ""
            val norm = raw.replace("-", "").replace("_", "").replace(" ", "")
            return norm == "vnm" ||
                   norm == "visualnovelmaker" ||
                   norm == "vnmaker" ||
                   norm == "vnmweb" ||
                   norm == "vnmakerweb"
        }

        private val LOCALHOST_REGEX = Regex("""^http://(?:localhost|127\.0\.0\.1)(?::\d+)?/""")
        private val ALLOWLISTED_SHIMS = setOf("web_shims/runtimepack_web_shims.js")

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

        private val IMAGE_EXTENSIONS = setOf("png", "jpg", "jpeg", "webp", "gif", "rpgmvp", "rmmzimg")
        private val AUDIO_EXTENSIONS = setOf("ogg", "m4a", "mp3", "wav", "rpgmvo", "rpgmvm", "rmmzaud")

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
        if (logicalPath.startsWith("web_shims/")) {
            if (!isAllowlistedShim(logicalPath)) {
                return null
            }
            val shimStream = streamProvider(logicalPath) ?: return null
            recordResourceServed(logicalPath)
            return createResponse("application/javascript", "UTF-8", shimStream)
        }
        val isHtmlRequest = (logicalPath == "index.html" || logicalPath.isEmpty())
        var stream = streamProvider(logicalPath)
        if (stream == null && isHtmlRequest) {
            stream = streamProvider("index.html")
        }
        if (stream == null) {
            val lower = logicalPath.lowercase()
            if (lower != logicalPath) {
                stream = streamProvider(lower)
            }
        }
        if (stream == null && reader != null) {
            val entry = reader.getEntry(if (logicalPath.isEmpty()) "index.html" else logicalPath)
            if (entry != null) {
                stream = streamProvider(entry.logicalPath)
            }
        }
        if (isHtmlRequest && isVnmProfile()) {
            val rawStream = stream ?: streamProvider("index.html") ?: reader?.openStream("index.html")
            if (rawStream != null) {
                val rawHtml = rawStream.bufferedReader(Charsets.UTF_8).readText()
                val transformed = transformVnmHtml(rawHtml)
                recordResourceServed("index.html")
                return createResponse("text/html", "UTF-8", ByteArrayInputStream(transformed.toByteArray(Charsets.UTF_8)))
            }
        }
        if (stream == null && logicalPath.contains('%')) {
            val decoded = try {
                URLDecoder.decode(logicalPath.replace("+", "%2B"), "UTF-8")
            } catch (_: Exception) {
                null
            }
            if (decoded != null) {
                stream = streamProvider(decoded)
                if (stream == null) {
                    val decodedLower = decoded.lowercase()
                    if (decodedLower != decoded) {
                        stream = streamProvider(decodedLower)
                    }
                }
            }
        }
        if (stream == null && (logicalPath.startsWith("js/plugins/") || logicalPath.startsWith("www/js/plugins/")) && logicalPath.endsWith(".js")) {
            val filename = logicalPath.substringAfterLast('/').lowercase()
            val matchedLogicalPath = pluginFilenameMap[filename]
            if (matchedLogicalPath != null) {
                stream = streamProvider(matchedLogicalPath)
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
                var encStream = streamProvider(encPath)
                if (encStream == null) {
                    val encLower = encPath.lowercase()
                    if (encLower != encPath) {
                        encStream = streamProvider(encLower)
                    }
                }
                if (encStream == null && reader != null) {
                    val entry = reader.getEntry(encPath)
                    if (entry != null) {
                        encStream = streamProvider(entry.logicalPath)
                    }
                }
                if (encStream != null) {
                    val encBytes = encStream.readBytes()
                    val decrypted = decryptRpgMakerAsset(encBytes, rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(logicalPath)
                        val mime = getMimeType(logicalPath)
                        return createResponse(mime, getEncoding(mime), ByteArrayInputStream(decrypted))
                    }
                }
            }
        }
        if (stream == null && logicalPath.endsWith(".rpgmvm")) {
            val oggEnc = logicalPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            val s = streamProvider(oggEnc) ?: if (oggEnc.lowercase() != oggEnc) streamProvider(oggEnc.lowercase()) else null
            if (s != null) {
                recordResourceServed(logicalPath)
                return createResponse("audio/ogg", null, s)
            }
        }
        if (stream == null && (logicalPath.endsWith(".m4a") || logicalPath.endsWith(".m4a_"))) {
            val base = if (logicalPath.endsWith(".m4a_")) logicalPath.removeSuffix(".m4a_") else logicalPath.removeSuffix(".m4a")
            val oggEnc = base + ".ogg_"
            val sEnc = streamProvider(oggEnc) ?: if (oggEnc.lowercase() != oggEnc) streamProvider(oggEnc.lowercase()) else null
            if (sEnc != null) {
                recordResourceServed(logicalPath)
                return createResponse("audio/ogg", null, sEnc)
            }
            val oggMvEnc = base + ".rpgmvo"
            val sMvEnc = streamProvider(oggMvEnc) ?: if (oggMvEnc.lowercase() != oggMvEnc) streamProvider(oggMvEnc.lowercase()) else null
            if (sMvEnc != null) {
                if (rpgMakerKey != null) {
                    val decrypted = decryptRpgMakerAsset(sMvEnc.readBytes(), rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(logicalPath)
                        return createResponse("audio/ogg", null, ByteArrayInputStream(decrypted))
                    }
                }
                recordResourceServed(logicalPath)
                return createResponse("audio/ogg", null, sMvEnc)
            }
            val oggPlain = base + ".ogg"
            val sPlain = streamProvider(oggPlain) ?: if (oggPlain.lowercase() != oggPlain) streamProvider(oggPlain.lowercase()) else null
            if (sPlain != null) {
                recordResourceServed(logicalPath)
                return createResponse("audio/ogg", null, sPlain)
            }
        }
        if (stream == null && (logicalPath.endsWith(".ogg") || logicalPath.endsWith(".png"))) {
            val encPath = logicalPath + "_"
            val s = streamProvider(encPath) ?: if (encPath.lowercase() != encPath) streamProvider(encPath.lowercase()) else null
            if (s != null) {
                recordResourceServed(logicalPath)
                val mime = getMimeType(logicalPath)
                return createResponse(mime, getEncoding(mime), s)
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
                return createResponse("image/png", null, ByteArrayInputStream(TRANSPARENT_1X1_PNG))
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
                return createResponse(mime, null, ByteArrayInputStream(EMPTY_AUDIO))
            }
            return null
        }
        val effectivePath = if (logicalPath.isEmpty()) "index.html" else logicalPath
        recordResourceServed(effectivePath)
        val mime = getMimeType(effectivePath)
        return createResponse(mime, getEncoding(mime), stream)
    }
}
