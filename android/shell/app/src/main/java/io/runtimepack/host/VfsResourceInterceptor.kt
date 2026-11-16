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
    val scriptCompletionHooks: List<String> = RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS,
    private val streamProvider: (String) -> InputStream?
) {
    constructor(reader: GpackStreamReader) : this(reader, null, null, "off", RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?) : this(reader, rpgMakerKey, null, "off", RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?, engineFamily: String?) : this(reader, rpgMakerKey, engineFamily, "off", RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?, engineFamily: String?, vnmInputGuard: String) : this(reader, rpgMakerKey, engineFamily, vnmInputGuard, RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, { path -> reader.openStream(path) })

    constructor(reader: GpackStreamReader, rpgMakerKey: String?, engineFamily: String?, vnmInputGuard: String, scriptCompletionHooks: List<String>) : this(reader, rpgMakerKey, engineFamily, vnmInputGuard, scriptCompletionHooks, { path -> reader.openStream(path) })

    constructor(streamProvider: (String) -> InputStream?) : this(null, null, null, "off", RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, streamProvider)

    constructor(engineFamily: String?, streamProvider: (String) -> InputStream?) : this(null, null, engineFamily, "off", RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, streamProvider)

    constructor(engineFamily: String?, vnmInputGuard: String, streamProvider: (String) -> InputStream?) : this(null, null, engineFamily, vnmInputGuard, RuntimeBuildInfo.SCRIPT_COMPLETION_HOOKS, streamProvider)

    constructor(scriptCompletionHooks: List<String>, streamProvider: (String) -> InputStream?) : this(null, null, null, "off", scriptCompletionHooks, streamProvider)

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

    fun transformWebHtml(rawHtml: String): String {
        if (rawHtml.contains("web_shims/runtimepack_web_shims.js")) {
            return rawHtml
        }
        val shimTag = "    <script type=\"text/javascript\" src=\"web_shims/runtimepack_web_shims.js\"></script>\n"
        val mainScriptRegex = Regex("""<script[^>]*src\s*=\s*["'](?:\./)?js/main\.js["'][^>]*>\s*</script>""", RegexOption.IGNORE_CASE)
        val mainMatch = mainScriptRegex.find(rawHtml)
        return if (mainMatch != null) {
            rawHtml.substring(0, mainMatch.range.first) + shimTag + rawHtml.substring(mainMatch.range.first)
        } else {
            val firstScriptRegex = Regex("""<script\b""", RegexOption.IGNORE_CASE)
            val firstMatch = firstScriptRegex.find(rawHtml)
            if (firstMatch != null) {
                rawHtml.substring(0, firstMatch.range.first) + shimTag + rawHtml.substring(firstMatch.range.first)
            } else {
                val headCloseRegex = Regex("""</head>""", RegexOption.IGNORE_CASE)
                val headMatch = headCloseRegex.find(rawHtml)
                if (headMatch != null) {
                    rawHtml.substring(0, headMatch.range.first) + shimTag + rawHtml.substring(headMatch.range.first)
                } else {
                    shimTag + rawHtml
                }
            }
        }
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

        val SCRIPT_COMPLETION_TRAILER: String = "\n;\n(function() {\n" +
            "    var u = (typeof Utils !== \"undefined\") ? Utils : (typeof window !== \"undefined\" ? window.Utils : undefined);\n" +
            "    var hasTarget = (typeof u === \"function\" || (typeof u === \"object\" && u !== null));\n" +
            "    var applied = false;\n" +
            "    if (typeof window !== \"undefined\" && window.RuntimePack && typeof window.RuntimePack.applyPlatformIdentity === \"function\") {\n" +
            "        if (hasTarget) {\n" +
            "            applied = (window.RuntimePack.applyPlatformIdentity(u) !== false);\n" +
            "        }\n" +
            "    }\n" +
            "    if (hasTarget && !applied) {\n" +
            "        var falseFn = function() { return false; };\n" +
            "        falseFn.__rp_patched = true;\n" +
            "        try {\n" +
            "            Object.defineProperty(u, \"isNwjs\", {\n" +
            "                configurable: true,\n" +
            "                enumerable: true,\n" +
            "                get: function() { return falseFn; },\n" +
            "                set: function(_fn) {}\n" +
            "            });\n" +
            "        } catch (_) {\n" +
            "            try { u.isNwjs = falseFn; } catch (_) {}\n" +
            "        }\n" +
            "        try {\n" +
            "            if (typeof window !== \"undefined\") {\n" +
            "                window.__runtimepack_pendingIdentity = u;\n" +
            "            }\n" +
            "        } catch (_) {}\n" +
            "        var pinned = false;\n" +
            "        try { pinned = (typeof u.isNwjs === \"function\" && u.isNwjs() === false); } catch (_) { pinned = false; }\n" +
            "        if (!pinned) {\n" +
            "            if (typeof console !== \"undefined\" && typeof console.error === \"function\") {\n" +
            "                console.error(\"[RuntimePack] WEB-029: Failed to pin Utils.isNwjs in core script completion hook\");\n" +
            "            }\n" +
            "        } else if (typeof console !== \"undefined\" && typeof console.warn === \"function\") {\n" +
            "            console.warn(\"[RuntimePack] WEB-029: platform identity pending shim handoff in core script completion hook\");\n" +
            "        }\n" +
            "    } else if (!hasTarget) {\n" +
            "        if (typeof console !== \"undefined\" && typeof console.error === \"function\") {\n" +
            "            console.error(\"[RuntimePack] WEB-029: Utils is undefined in core script completion hook\");\n" +
            "        }\n" +
            "    }\n" +
            "})();\n"

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

        private fun isHex(c: Char): Boolean =
            (c in '0'..'9') || (c in 'a'..'f') || (c in 'A'..'F')

        fun safePercentDecode(path: String): String? {
            if (!path.contains('%')) return path
            var i = 0
            while (i < path.length) {
                if (path[i] == '%') {
                    if (i + 2 >= path.length) return null
                    val h1 = path[i + 1]
                    val h2 = path[i + 2]
                    if (!isHex(h1) || !isHex(h2)) return null
                    val hex = "" + h1 + h2
                    if (hex.equals("00", ignoreCase = true)) return null // NUL
                    if (hex.equals("5c", ignoreCase = true)) return null // backslash
                    if (hex.equals("2f", ignoreCase = true)) return null // slash
                    i += 3
                } else {
                    i++
                }
            }
            val safeForDecoder = path.replace("+", "%2B")
            return try {
                URLDecoder.decode(safeForDecoder, "UTF-8")
            } catch (_: Exception) {
                null
            }
        }

        fun urlToLogicalPath(url: String): String? {
            val clean = url.substringBefore('?').substringBefore('#')
            if (clean.contains('\u0000')) return null

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

            if (rawPath.contains('\\') || rawPath.contains("%5c", ignoreCase = true) ||
                rawPath.contains("%00", ignoreCase = true) || rawPath.contains("%2f", ignoreCase = true)
            ) {
                return null
            }

            val normalizedParts = mutableListOf<String>()
            for (part in rawPath.split('/')) {
                if (part.isEmpty() || part == ".") {
                    continue
                }
                val decodedPart = safePercentDecode(part) ?: return null
                if (decodedPart == "..") {
                    if (normalizedParts.isEmpty()) {
                        return null
                    }
                    normalizedParts.removeAt(normalizedParts.size - 1)
                } else if (decodedPart == ".") {
                    continue
                } else {
                    if (decodedPart.contains(':')) {
                        return null
                    }
                    normalizedParts.add(part)
                }
            }
            return normalizedParts.joinToString("/")
        }
    }

    fun getMimeType(path: String): String = Companion.getMimeType(path)

    fun urlToLogicalPath(url: String): String? = Companion.urlToLogicalPath(url)

    private fun findStream(path: String): Pair<InputStream, String>? {
        val s = streamProvider(path)
        if (s != null) return Pair(s, path)

        val lower = path.lowercase()
        if (lower != path) {
            val sLower = streamProvider(lower)
            if (sLower != null) return Pair(sLower, lower)
        }

        if (reader != null) {
            val entry = reader.getEntry(path)
            if (entry != null) {
                val sEntry = streamProvider(entry.logicalPath)
                if (sEntry != null) return Pair(sEntry, entry.logicalPath)
            }
            if (lower != path) {
                val lowerEntry = reader.getEntry(lower)
                if (lowerEntry != null) {
                    val sLowerEntry = streamProvider(lowerEntry.logicalPath)
                    if (sLowerEntry != null) return Pair(sLowerEntry, lowerEntry.logicalPath)
                }
            }
        }
        return null
    }

    private fun resolveStream(rawPath: String, decodedPath: String): Pair<InputStream, String>? {
        val directRaw = findStream(rawPath)
        if (directRaw != null) return directRaw

        if (decodedPath != rawPath) {
            val directDecoded = findStream(decodedPath)
            if (directDecoded != null) return directDecoded
        }
        return null
    }

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
        val decodedPath = safePercentDecode(logicalPath) ?: return null

        if (logicalPath.startsWith("web_shims/") || decodedPath.startsWith("web_shims/")) {
            val targetShim = if (logicalPath.startsWith("web_shims/")) logicalPath else decodedPath
            if (!isAllowlistedShim(targetShim)) {
                return null
            }
            val shimStream = streamProvider(targetShim) ?: return null
            recordResourceServed(targetShim)
            return createResponse("application/javascript", "UTF-8", shimStream)
        }

        val isHtmlRequest = (logicalPath.equals("index.html", ignoreCase = true) || logicalPath.isEmpty() ||
                             decodedPath.equals("index.html", ignoreCase = true) || decodedPath.isEmpty())

        var resolved = resolveStream(
            if (logicalPath.isEmpty()) "index.html" else logicalPath,
            if (decodedPath.isEmpty()) "index.html" else decodedPath
        )

        if (resolved == null && isHtmlRequest) {
            resolved = resolveStream("index.html", "index.html")
        }

        if (isHtmlRequest && isVnmProfile()) {
            val rawStream = resolved?.first ?: streamProvider("index.html") ?: reader?.openStream("index.html")
            if (rawStream != null) {
                val rawHtml = rawStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                val transformed = transformVnmHtml(rawHtml)
                recordResourceServed("index.html")
                return createResponse("text/html", "UTF-8", ByteArrayInputStream(transformed.toByteArray(Charsets.UTF_8)))
            }
        }

        if (isHtmlRequest && !isVnmProfile() && scriptCompletionHooks.isNotEmpty()) {
            val rawStream = resolved?.first ?: streamProvider("index.html") ?: reader?.openStream("index.html")
            if (rawStream != null) {
                val rawHtml = rawStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                val transformed = transformWebHtml(rawHtml)
                recordResourceServed("index.html")
                return createResponse("text/html", "UTF-8", ByteArrayInputStream(transformed.toByteArray(Charsets.UTF_8)))
            }
        }

        val canonicalHooks = scriptCompletionHooks.map { it.replace('\\', '/').trimStart('/') }
        val isHookMatch = canonicalHooks.any { hook ->
            logicalPath.equals(hook, ignoreCase = true) || decodedPath.equals(hook, ignoreCase = true)
        }
        if (isHookMatch) {
            val targetStream = resolved?.first
                ?: streamProvider(logicalPath)
                ?: streamProvider(decodedPath)
                ?: streamProvider(logicalPath.lowercase())
                ?: reader?.openStream(logicalPath)
                ?: reader?.openStream(decodedPath)
                ?: reader?.openStream(logicalPath.lowercase())
            if (targetStream != null) {
                val effectiveRecordPath = if (canonicalHooks.any { logicalPath.equals(it, ignoreCase = true) }) logicalPath else decodedPath
                recordResourceServed(effectiveRecordPath)
                val trailerBytes = SCRIPT_COMPLETION_TRAILER.toByteArray(Charsets.UTF_8)
                val compositeStream = java.io.SequenceInputStream(targetStream, ByteArrayInputStream(trailerBytes))
                return createResponse("application/javascript", "UTF-8", compositeStream)
            }
        }

        if (resolved == null && ((logicalPath.startsWith("js/plugins/") || decodedPath.startsWith("js/plugins/") ||
                                  logicalPath.startsWith("www/js/plugins/") || decodedPath.startsWith("www/js/plugins/")) &&
                                 (logicalPath.endsWith(".js") || decodedPath.endsWith(".js")))) {
            val filename = (if (logicalPath.endsWith(".js")) logicalPath else decodedPath).substringAfterLast('/').lowercase()
            val matchedLogicalPath = pluginFilenameMap[filename]
            if (matchedLogicalPath != null) {
                resolved = findStream(matchedLogicalPath)
            }
        }

        if (resolved == null && rpgMakerKey != null) {
            val ext = decodedPath.substringAfterLast('.', "").lowercase()
            val encExt = when (ext) {
                "png" -> "rpgmvp"
                "ogg" -> "rpgmvo"
                "m4a" -> "rpgmvm"
                else -> null
            }
            if (encExt != null) {
                val rawEncPath = logicalPath.substringBeforeLast('.') + "." + encExt
                val decEncPath = decodedPath.substringBeforeLast('.') + "." + encExt
                val encResult = resolveStream(rawEncPath, decEncPath)
                if (encResult != null) {
                    val encBytes = encResult.first.use { it.readBytes() }
                    val decrypted = decryptRpgMakerAsset(encBytes, rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(encResult.second)
                        val mime = getMimeType(decodedPath)
                        return createResponse(mime, getEncoding(mime), ByteArrayInputStream(decrypted))
                    }
                }
            }
        }

        if (resolved == null && (logicalPath.endsWith(".rpgmvm") || decodedPath.endsWith(".rpgmvm"))) {
            val rawOgg = logicalPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            val decOgg = decodedPath.removeSuffix(".rpgmvm") + ".rpgmvo"
            resolved = resolveStream(rawOgg, decOgg)
            if (resolved != null) {
                recordResourceServed(resolved.second)
                return createResponse("audio/ogg", null, resolved.first)
            }
        }

        if (resolved == null && (logicalPath.endsWith(".m4a") || logicalPath.endsWith(".m4a_") ||
                                 decodedPath.endsWith(".m4a") || decodedPath.endsWith(".m4a_"))) {
            val rawBase = if (logicalPath.endsWith(".m4a_")) logicalPath.removeSuffix(".m4a_") else logicalPath.removeSuffix(".m4a")
            val decBase = if (decodedPath.endsWith(".m4a_")) decodedPath.removeSuffix(".m4a_") else decodedPath.removeSuffix(".m4a")

            var sAudio = resolveStream(rawBase + ".ogg_", decBase + ".ogg_")
            if (sAudio != null) {
                recordResourceServed(sAudio.second)
                return createResponse("audio/ogg", null, sAudio.first)
            }

            sAudio = resolveStream(rawBase + ".rpgmvo", decBase + ".rpgmvo")
            if (sAudio != null) {
                if (rpgMakerKey != null) {
                    val decrypted = decryptRpgMakerAsset(sAudio.first.readBytes(), rpgMakerKey)
                    if (decrypted != null) {
                        recordResourceServed(sAudio.second)
                        return createResponse("audio/ogg", null, ByteArrayInputStream(decrypted))
                    }
                }
                recordResourceServed(sAudio.second)
                return createResponse("audio/ogg", null, sAudio.first)
            }

            sAudio = resolveStream(rawBase + ".ogg", decBase + ".ogg")
            if (sAudio != null) {
                recordResourceServed(sAudio.second)
                return createResponse("audio/ogg", null, sAudio.first)
            }
        }

        if (resolved == null && (logicalPath.endsWith(".ogg") || logicalPath.endsWith(".png") ||
                                 decodedPath.endsWith(".ogg") || decodedPath.endsWith(".png"))) {
            val rawEnc = logicalPath + "_"
            val decEnc = decodedPath + "_"
            val encResult = resolveStream(rawEnc, decEnc)
            if (encResult != null) {
                recordResourceServed(encResult.second)
                val mime = getMimeType(decodedPath)
                return createResponse(mime, getEncoding(mime), encResult.first)
            }
        }

        if (resolved == null) {
            val cleanExt = decodedPath.substringAfterLast('.', "").lowercase().removeSuffix("_")
            if (cleanExt in IMAGE_EXTENSIONS) {
                AssetAuditService.recordEvent(
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    assetPath = decodedPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    fallbackAction = "PLACEHOLDER_SUBSTITUTED",
                    errorMessage = "Resource missing in VFS, transparent placeholder substituted"
                )
                return createResponse("image/png", null, ByteArrayInputStream(TRANSPARENT_1X1_PNG))
            }
            if (cleanExt in AUDIO_EXTENSIONS) {
                val mime = getMimeType(decodedPath)
                AssetAuditService.recordEvent(
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    assetPath = decodedPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    fallbackAction = "PLACEHOLDER_SUBSTITUTED",
                    errorMessage = "Resource missing in VFS, transparent placeholder substituted"
                )
                return createResponse(mime, null, ByteArrayInputStream(EMPTY_AUDIO))
            }
            return null
        }

        val effectivePath = resolved.second
        recordResourceServed(effectivePath)
        val mime = getMimeType(effectivePath)
        return createResponse(mime, getEncoding(mime), resolved.first)
    }
}
