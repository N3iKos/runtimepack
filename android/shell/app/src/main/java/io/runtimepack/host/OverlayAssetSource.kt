package io.runtimepack.host

import android.content.Context
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.File
import java.io.InputStream
import java.net.URLDecoder

object OverlayAssetSource {
    @Volatile
    var gpackReader: GpackStreamReader? = null

    fun ensureInitialized(context: Context) {
        if (gpackReader != null) return
        val candidates = listOf(
            File(File(context.filesDir, "game"), "game.pack"),
            File(context.filesDir, "game.pack")
        )
        for (f in candidates) {
            if (f.exists()) {
                try {
                    gpackReader = GpackStreamReader.open(f)
                    return
                } catch (_: Exception) {}
            }
        }
        try {
            val afd = context.assets.openFd("game.pack")
            gpackReader = GpackStreamReader.open(afd)
            return
        } catch (_: Exception) {}
        try {
            val stream = context.assets.open("game.pack")
            gpackReader = GpackStreamReader.open(stream)
            return
        } catch (_: Exception) {}
    }

    fun exists(path: String, context: Context? = null): Boolean {
        val clean = path.replace('\\', '/').trimStart('/')
        if (gpackReader?.hasFile(clean) == true) return true
        if (context != null) {
            try {
                context.assets.open(clean).close()
                return true
            } catch (_: Exception) {}
        }
        return false
    }

    fun open(path: String, context: Context? = null): InputStream? {
        val clean = path.replace('\\', '/').trimStart('/')
        val gpackStream = gpackReader?.openStream(clean)
        if (gpackStream != null) return gpackStream
        if (context != null) {
            try {
                return context.assets.open(clean)
            } catch (_: Exception) {}
        }
        return null
    }

    fun read(path: String, context: Context? = null): ByteArray? {
        return open(path, context)?.use { it.readBytes() }
    }

    fun guessMimeType(path: String): String {
        val lower = path.substringBefore('?').substringBefore('#').lowercase()
        return when {
            lower.endsWith(".html") || lower.endsWith(".htm") -> "text/html"
            lower.endsWith(".js") || lower.endsWith(".mjs") -> "application/javascript"
            lower.endsWith(".css") -> "text/css"
            lower.endsWith(".json") -> "application/json"
            lower.endsWith(".png") -> "image/png"
            lower.endsWith(".jpg") || lower.endsWith(".jpeg") -> "image/jpeg"
            lower.endsWith(".svg") -> "image/svg+xml"
            lower.endsWith(".ttf") -> "font/ttf"
            lower.endsWith(".woff") -> "font/woff"
            lower.endsWith(".woff2") -> "font/woff2"
            else -> "application/octet-stream"
        }
    }

    fun shouldIntercept(url: String?, context: Context? = null): WebResourceResponse? {
        if (url == null) return null
        val cleanUrl = url.substringBefore('?').substringBefore('#')
        val prefix = "file:///android_asset/"
        if (!cleanUrl.startsWith(prefix)) return null
        val rawPath = cleanUrl.removePrefix(prefix)
        val decodedPath = try {
            URLDecoder.decode(rawPath, "UTF-8")
        } catch (_: Exception) {
            rawPath
        }
        val cleanPath = decodedPath.replace('\\', '/').trimStart('/')
        if (!cleanPath.startsWith("overlay/") && !cleanPath.startsWith("fonts/")) return null

        val stream = open(cleanPath, context)
            ?: open(rawPath.replace('\\', '/').trimStart('/'), context)
            ?: return null

        val mime = guessMimeType(cleanPath)
        val encoding = if (mime.startsWith("text/") || mime == "application/javascript" || mime == "application/json") "utf-8" else null
        return createResponse(mime, encoding, stream)
    }

    fun createOverlayWebViewClient(
        context: Context,
        onPageFinishedCallback: ((view: WebView?, url: String?) -> Unit)? = null
    ): WebViewClient {
        return object : WebViewClient() {
            override fun shouldInterceptRequest(
                view: WebView?,
                request: WebResourceRequest?
            ): WebResourceResponse? {
                val url = request?.url?.toString()
                return shouldIntercept(url, context) ?: super.shouldInterceptRequest(view, request)
            }

            @Deprecated("Deprecated in Java")
            override fun shouldInterceptRequest(
                view: WebView?,
                url: String?
            ): WebResourceResponse? {
                return shouldIntercept(url, context) ?: @Suppress("DEPRECATION") super.shouldInterceptRequest(view, url)
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                onPageFinishedCallback?.invoke(view, url)
            }
        }
    }

    private fun createResponse(mime: String, encoding: String?, stream: InputStream): WebResourceResponse {
        return object : WebResourceResponse(mime, encoding, stream) {
            override fun getData(): InputStream = stream
            override fun getMimeType(): String = mime
            override fun getEncoding(): String? = encoding
        }
    }
}
