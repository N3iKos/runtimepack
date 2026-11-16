package io.runtimepack.host

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import java.io.ByteArrayInputStream

object SharedNavigationPolicy {
    const val CANONICAL_GAME_HOST = "game.local"
    const val CANONICAL_GAME_ORIGIN = "https://game.local"
    private const val ASSET_GAME_PREFIX = "file:///android_asset/game/"

    fun isCanonicalGameUri(scheme: String?, host: String?, port: Int, rawUrl: String? = null): Boolean {
        if (!"https".equals(scheme, ignoreCase = true)) return false
        if (!CANONICAL_GAME_HOST.equals(host, ignoreCase = true)) return false
        if (port != -1 && port != 443) return false
        if (rawUrl != null) {
            val parsed = try {
                java.net.URI(rawUrl)
            } catch (_: Exception) {
                return false
            }
            if (!parsed.isAbsolute || parsed.isOpaque) return false
            val pScheme = parsed.scheme ?: return false
            val pHost = parsed.host ?: return false
            val pPort = parsed.port
            val pRawAuth = parsed.rawAuthority ?: return false

            if (!"https".equals(pScheme, ignoreCase = true)) return false
            if (!CANONICAL_GAME_HOST.equals(pHost, ignoreCase = true)) return false
            if (pPort != -1 && pPort != 443) return false

            if (scheme != null && !pScheme.equals(scheme, ignoreCase = true)) return false
            if (host != null && !pHost.equals(host, ignoreCase = true)) return false
            if (port != -1 && port != 443 && port != pPort) return false

            if (parsed.rawUserInfo != null) return false
            if (pRawAuth.contains('@') || pRawAuth.contains("%40", ignoreCase = true) ||
                pRawAuth.contains("%2540", ignoreCase = true)
            ) {
                return false
            }
            if (pRawAuth.contains('\\') || pRawAuth.contains("%5c", ignoreCase = true)) {
                return false
            }

            if (rawUrl.contains('\\') || rawUrl.contains("%5c", ignoreCase = true)) {
                return false
            }
        }
        return true
    }

    fun isAllowedAssetUri(rawUrl: String?): Boolean {
        if (rawUrl.isNullOrBlank()) return false
        if (!rawUrl.startsWith(ASSET_GAME_PREFIX)) return false
        val subPath = rawUrl.removePrefix(ASSET_GAME_PREFIX)
        if (subPath.contains("..") || subPath.contains('\\') || subPath.contains("%2e", ignoreCase = true)) {
            return false
        }
        return true
    }

    /**
     * Shared navigation override policy for game WebViews.
     *
     * Invariants:
     * 1. Allows internal navigation within the canonical game origin (https://game.local/).
     * 2. Delegates main-frame user-gesture HTTP/HTTPS links to the injected external URL dispatcher
     *    and returns true, blocking in-place navigation of external domains in privileged WebViews.
     * 3. Blocks and logs all unauthorized in-place navigations (non-gesture, subframe, non-HTTP schemes).
     * 4. Guarantees gameWebView NEVER navigates away from https://game.local/.
     */
    @Suppress("UNUSED_PARAMETER")
    fun shouldOverrideUrlLoading(
        view: WebView?,
        request: WebResourceRequest?,
        urlDispatcher: (String) -> Boolean
    ): Boolean {
        val uri = request?.url ?: return true
        val scheme = uri.scheme?.lowercase() ?: return true
        val host = uri.host?.lowercase() ?: ""
        val port = uri.port
        val rawUrl = uri.toString()

        // 1. Internal canonical origin navigation: allow WebView to handle in-place
        if (isCanonicalGameUri(scheme, host, port, rawUrl)) {
            return false
        }

        // 2. Allowed legacy asset navigation without path traversal
        if (isAllowedAssetUri(rawUrl)) {
            return false
        }

        // 3. Mainframe external HTTP/HTTPS with user gesture: hand off to external browser
        val hasGesture = request.hasGesture()
        val isMainFrame = request.isForMainFrame
        if (isMainFrame && hasGesture && (scheme == "http" || scheme == "https")) {
            urlDispatcher(rawUrl)
            return true
        }

        // 4. Block all other unauthorized navigations fail-closed
        val sanitized = WebExternalUrlPolicy.sanitizeForLog(rawUrl)
        DiagnosticLogStore.recordLog(
            level = "WARN",
            message = "[AND-SEC-001] Blocked unauthorized in-place navigation to $sanitized (mainFrame=$isMainFrame, gesture=$hasGesture)",
            source = "WEB_SECURITY"
        )
        return true
    }

    /**
     * Legacy string-based overload for backward compatibility.
     */
    fun shouldOverrideUrlLoadingLegacy(
        url: String?,
        urlDispatcher: (String) -> Boolean
    ): Boolean {
        if (url.isNullOrBlank()) return true
        val norm = url.lowercase()
        if (norm.startsWith("https://game.local/") || norm == "https://game.local") {
            return false
        }
        if (isAllowedAssetUri(url)) {
            return false
        }
        val sanitized = WebExternalUrlPolicy.sanitizeForLog(url)
        DiagnosticLogStore.recordLog(
            level = "WARN",
            message = "[AND-SEC-001] Blocked legacy in-place navigation to $sanitized",
            source = "WEB_SECURITY"
        )
        if (norm.startsWith("http://") || norm.startsWith("https://")) {
            urlDispatcher(url)
        }
        return true
    }

    /**
     * Checks whether an intercepted request is an untrusted remote origin request
     * attempting to enter the privileged JavaScript bridge context.
     *
     * Fail-closed: All remote origin requests (documents, iframes, scripts, XHR)
     * are blocked with HTTP 403 Forbidden unless targeting canonical local game origin.
     *
     * @return 403 Forbidden WebResourceResponse if blocked, null if allowed to proceed to local VFS.
     */
    fun interceptRemoteRequest(request: WebResourceRequest?): WebResourceResponse? {
        val uri = request?.url ?: return null
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: ""
        val port = uri.port
        val rawUrl = uri.toString()

        if (isCanonicalGameUri(scheme, host, port, rawUrl)) {
            return null
        }

        if (isAllowedAssetUri(rawUrl)) {
            return null
        }

        // Block all untrusted remote requests into privileged WebView
        val sanitized = WebExternalUrlPolicy.sanitizeForLog(rawUrl)
        DiagnosticLogStore.recordLog(
            level = "WARN",
            message = "[AND-SEC-003] Blocked untrusted remote origin request in privileged WebView: $sanitized (mainFrame=${request.isForMainFrame})",
            source = "WEB_SECURITY"
        )
        return WebResourceResponse(
            "text/plain",
            "UTF-8",
            403,
            "Forbidden",
            null,
            ByteArrayInputStream("Remote request blocked by RuntimePack security policy [AND-SEC-003]".toByteArray(Charsets.UTF_8))
        )
    }

    /**
     * Backward-compatible alias for interceptRemoteRequest.
     */
    fun interceptRemoteDocument(request: WebResourceRequest?): WebResourceResponse? {
        return interceptRemoteRequest(request)
    }
}
