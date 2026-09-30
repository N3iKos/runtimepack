package io.runtimepack.host

import android.annotation.SuppressLint
import android.content.Context
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream
import java.io.File
import java.io.InputStream
import org.json.JSONArray

class WebBridge(private val saveService: SaveService, private val saveRepository: NativeSaveRepository? = null) {
    private val repository: NativeSaveRepository by lazy {
        saveRepository ?: run {
            val baseDir = WebActivity.currentInstance?.filesDir
                ?: WebActivity.currentInstance?.gameDirectory?.parentFile
                ?: File(".")
            NativeSaveRepository(File(baseDir, "saves"))
        }
    }

    @JavascriptInterface
    fun saveSlot(namespace: String, slot: String, payload: String): Boolean {
        return repository.put(namespace, slot, payload)
    }

    @JavascriptInterface
    fun loadSlot(namespace: String, slot: String): String? {
        return repository.get(namespace, slot)
    }

    @JavascriptInterface
    fun slotExists(namespace: String, slot: String): Boolean {
        return repository.contains(namespace, slot)
    }

    @JavascriptInterface
    fun deleteSlot(namespace: String, slot: String): Boolean {
        return repository.remove(namespace, slot)
    }

    @JavascriptInterface
    fun listSlots(namespace: String): String {
        val keys = repository.keys(namespace)
        val json = try {
            org.json.JSONArray(keys).toString()
        } catch (_: Throwable) {
            null
        }
        return if (!json.isNullOrEmpty()) json else keys.joinToString(prefix = "[", postfix = "]", separator = ",") {
            "\"" + it.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
        }
    }

    @JavascriptInterface
    fun saveItem(key: String, value: String) {
        saveService.openWrite(key).use {
            it.write(value.toByteArray(Charsets.UTF_8))
        }
    }

    @JavascriptInterface
    fun loadItem(key: String): String? {
        val stream = saveService.openRead(key) ?: return null
        return stream.use {
            it.bufferedReader(Charsets.UTF_8).readText()
        }
    }

    @JavascriptInterface
    fun onFirstFrame() {
        HealthMarker.FIRST_FRAME.emit()
    }

    @JavascriptInterface
    fun fileExists(relPath: String): Boolean {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.hasFile(clean) ?: File(gameDir, clean).exists()
    }

    @JavascriptInterface
    fun fileSize(relPath: String): Long {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.getEntry(clean)?.size ?: File(gameDir, clean).length()
    }

    @JavascriptInterface
    fun readdir(relPath: String): String {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val prefix = if (clean.isEmpty()) "" else "$clean/"
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory

        val packEntries = reader?.fileIndexMap?.keys
            ?.filter { it.startsWith(prefix) }
            ?.map { it.removePrefix(prefix).substringBefore('/') }
            ?.distinct()

        val diskEntries = gameDir?.let {
            val dir = if (clean.isEmpty()) it else File(it, clean)
            if (dir.exists() && dir.isDirectory) dir.list()?.toList() else null
        }

        val result = (packEntries ?: diskEntries ?: emptyList()).distinct()
        val json = try {
            org.json.JSONArray(result).toString()
        } catch (_: Throwable) {
            null
        }
        return if (!json.isNullOrEmpty()) json else result.joinToString(prefix = "[", postfix = "]", separator = ",") {
            "\"" + it.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
        }
    }
}

class WebBackend : RuntimeBackend {
    private var context: RuntimeContext? = null
    private var webView: WebView? = null

    override fun create(context: RuntimeContext) {
        this.context = context
        HealthMarker.HOST_CREATED.emit()
        HealthMarker.RUNTIME_CREATED.emit()
    }

    override fun mountContent(contentService: ContentService) {
        HealthMarker.CONTENT_MOUNTED.emit()
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun attachSurface(surfaceView: View) {
        if (surfaceView is WebView) {
            this.webView = surfaceView
            configureWebView(surfaceView)
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun configureWebView(wv: WebView) {
        val ctx = context ?: return
        wv.isVerticalScrollBarEnabled = false
        wv.isHorizontalScrollBarEnabled = false
        wv.overScrollMode = View.OVER_SCROLL_NEVER

        val settings = wv.settings
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        // Strict security: disable direct file access; serve via controlled local origin
        settings.allowFileAccess = false
        settings.allowContentAccess = false

        wv.addJavascriptInterface(WebBridge(ctx.saveService), "RuntimePackNative")

        wv.webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(
                view: WebView?,
                request: WebResourceRequest?
            ): WebResourceResponse? {
                val url = request?.url ?: return null
                // Controlled local origin: https://game.local/
                if (url.scheme == "https" && url.host == "game.local") {
                    val path = url.path?.trimStart('/') ?: "index.html"
                    return try {
                        val stream = ctx.contentService.open(path)
                        val mime = getMimeType(path)
                        WebResourceResponse(mime, "utf-8", stream)
                    } catch (_: Exception) {
                        WebResourceResponse("text/plain", "utf-8", 404, "Not Found", null, ByteArrayInputStream("Asset Not Found".toByteArray()))
                    }
                }
                return super.shouldInterceptRequest(view, request)
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                injectJsShims(view)
                HealthMarker.GAME_STARTED.emit()
            }
        }
    }

    private fun injectJsShims(view: WebView?) {
        val shimScript = """
            (function() {
                // Compatibility Shim: node.path & nw.Window
                window.require = function(mod) {
                    if (mod === 'path') {
                        return {
                            join: function() {
                                return Array.prototype.slice.call(arguments).join('/').replace(/\/+/g, '/');
                            },
                            dirname: function(p) {
                                var parts = p.split('/');
                                parts.pop();
                                return parts.join('/') || '.';
                            },
                            basename: function(p) { return p.split('/').pop(); },
                            extname: function(p) {
                                var base = p.split('/').pop();
                                var idx = base.lastIndexOf('.');
                                return idx >= 0 ? base.substring(idx) : '';
                            }
                        };
                    }
                    if (mod === 'nw.gui' || mod === 'nw') {
                        return { Window: { get: function() { return { showDevTools: function(){}, close: function(){} }; } } };
                    }
                    console.warn('[RuntimePack] Unhandled require module:', mod);
                    return {};
                };
                window.nw = window.nw || { Window: { get: function() { return { showDevTools: function(){}, close: function(){} }; } } };
                // Report first frame
                setTimeout(function() {
                    if (window.RuntimePackNative) { window.RuntimePackNative.onFirstFrame(); }
                }, 100);
            })();
        """.trimIndent()
        view?.evaluateJavascript(shimScript, null)
    }

    private fun getMimeType(path: String): String {
        return when {
            path.endsWith(".html") -> "text/html"
            path.endsWith(".js") -> "application/javascript"
            path.endsWith(".json") -> "application/json"
            path.endsWith(".css") -> "text/css"
            path.endsWith(".png") -> "image/png"
            path.endsWith(".jpg") || path.endsWith(".jpeg") -> "image/jpeg"
            path.endsWith(".ogg") -> "audio/ogg"
            path.endsWith(".m4a") -> "audio/mp4"
            path.endsWith(".webm") -> "video/webm"
            else -> "application/octet-stream"
        }
    }

    override fun start(entryPoint: String) {
        webView?.loadUrl("https://game.local/index.html")
    }

    override fun onInput(event: RuntimeInputEvent) {}
    override fun pause() { webView?.onPause() }
    override fun resume() { webView?.onResume() }
    override fun stop() { webView?.destroy() }
}
