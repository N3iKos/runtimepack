package io.runtimepack.host

import java.net.URI
import java.util.Locale

object WebExternalUrlPolicy {
    const val DEFAULT_MAX_URL_LENGTH = 2048

    /**
     * Validates and normalizes an external URL.
     *
     * Invariants:
     * 1. Rejects null, blank, or raw whitespace entirely (no silent trim of whitespace).
     * 2. Rejects control characters (code < 32, 127) and Unicode whitespace.
     * 3. Rejects URLs exceeding maxLength.
     * 4. Rejects backslashes anywhere in the URL.
     * 5. Rejects raw or encoded userinfo / credentials (@, %40, %5c) anywhere in authority/raw URL.
     * 6. Requires absolute, non-opaque URI with scheme strictly http or https.
     * 7. Requires non-empty host and authority.
     * 8. Rejects unencoded IDN domains; supports punycode and IPv6 literals.
     * 9. Validates port in range 1..65535 (or -1 for default).
     * 10. Normalizes scheme and host to lowercase ASCII; preserves legitimate escaped path/query/fragment.
     */
    fun validateAndNormalize(rawUrl: String?, maxLength: Int = DEFAULT_MAX_URL_LENGTH): String? {
        if (rawUrl.isNullOrEmpty()) return null
        if (rawUrl.length > maxLength) return null

        // 1. Reject ANY control character or Unicode whitespace (including NBSP, Zs, Zl, Zp)
        for (c in rawUrl) {
            if (c.code < 32 || c.code == 127 || c.isWhitespace() || Character.isSpaceChar(c) || c == '\uFEFF') {
                return null
            }
        }

        // 2. Reject backslashes and credential delimiters anywhere in raw URL
        if (rawUrl.contains('\\')) return null
        if (rawUrl.contains('@')) return null
        if (rawUrl.contains("%40", ignoreCase = true)) return null
        if (rawUrl.contains("%5c", ignoreCase = true)) return null

        return try {
            val uri = URI(rawUrl)

            // 3. Must be absolute and non-opaque (hierarchical)
            if (!uri.isAbsolute || uri.isOpaque) return null

            // 4. Strict scheme allowlist: http or https only
            val rawScheme = uri.scheme ?: return null
            val scheme = rawScheme.lowercase(Locale.ROOT)
            if (scheme != "http" && scheme != "https") return null

            // 5. Host and raw authority validation
            val rawHost = uri.host ?: return null
            if (rawHost.isBlank()) return null
            val rawAuth = uri.rawAuthority ?: return null
            if (rawAuth.isBlank()) return null

            // 6. Userinfo rejection
            if (uri.rawUserInfo != null) return null

            // 7. Reject unencoded IDN in host (punycode and IPv6 literals are ASCII)
            if (rawHost.any { it.code > 127 }) return null

            // 8. Port validation (-1 or 1..65535)
            val port = uri.port
            if (port != -1 && (port < 1 || port > 65535)) return null

            // 9. Safe ASCII string representation
            val ascii = uri.toASCIIString()
            val schemeEnd = ascii.indexOf("://")
            if (schemeEnd == -1) return null

            val afterScheme = ascii.substring(schemeEnd + 3)
            val authEnd = afterScheme.indexOfAny(charArrayOf('/', '?', '#')).let {
                if (it == -1) afterScheme.length else it
            }
            val authStr = afterScheme.substring(0, authEnd)
            val pathQueryFrag = afterScheme.substring(authEnd)

            "$scheme://${authStr.lowercase(Locale.ROOT)}$pathQueryFrag"
        } catch (_: Exception) {
            null
        }
    }

    /**
     * Differential parser validation comparing java.net.URI with android.net.Uri.
     */
    fun verifyAndroidUriParity(
        normalizedUrl: String,
        androidScheme: String?,
        androidHost: String?,
        androidPort: Int = -1
    ): Boolean {
        return try {
            val javaUri = URI(normalizedUrl)
            if (!javaUri.scheme.equals(androidScheme, ignoreCase = true)) {
                return false
            }

            val jHost = javaUri.host?.trim('[', ']') ?: return false
            val aHost = androidHost?.trim('[', ']') ?: return false
            if (!jHost.equals(aHost, ignoreCase = true)) {
                return false
            }

            val jPort = if (javaUri.port == -1) -1 else javaUri.port
            val aPort = if (androidPort <= 0) -1 else androidPort
            if (aPort != -1 && jPort != aPort) {
                return false
            }

            true
        } catch (_: Exception) {
            false
        }
    }

    /**
     * Sanitizes a URL for diagnostic logging: outputs scheme and host only.
     * Never logs query tokens, parameters, or full paths which could leak secrets.
     */
    fun sanitizeForLog(rawUrl: String?): String {
        if (rawUrl == null) return "[null-url]"
        val normalized = validateAndNormalize(rawUrl) ?: return "[invalid-url]"
        return try {
            val uri = URI(normalized)
            val scheme = uri.scheme
            val host = uri.host
            val port = uri.port
            if (port != -1 && port != 80 && port != 443) {
                "$scheme://$host:$port"
            } else {
                "$scheme://$host"
            }
        } catch (_: Exception) {
            "[invalid-url]"
        }
    }
}
