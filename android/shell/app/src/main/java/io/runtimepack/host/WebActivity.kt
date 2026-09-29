package io.runtimepack.host

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.webkit.ConsoleMessage
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.RelativeLayout
import java.io.File

open class WebActivity : Activity() {

    companion object {
        private const val TAG = "WebActivity"

        private val HITBOX_FIELD_RE = Regex("\"(left|top|right|bottom)\"\\s*:\\s*(-?[0-9.]+(?:[eE][-+]?[0-9]+)?)")
        private val HITBOX_OBJECT_RE = Regex("\\{[^{}]*\\}")

        data class ControlHitbox(val left: Float, val top: Float, val right: Float, val bottom: Float, val id: String = "") {
            constructor(id: String, left: Float, top: Float, right: Float, bottom: Float) : this(left, top, right, bottom, id)

            fun containsWithMargin(x: Float, y: Float, margin: Float, density: Float = 1.0f): Boolean {
                // 1. Direct physical coordinate match (if hitboxes were reported scaled by dpr)
                if (x in (left - margin)..(right + margin) && y in (top - margin)..(bottom + margin)) {
                    return true
                }
                // 2. CSS-to-physical scaled match fallback (if hitboxes were reported in raw CSS pixels)
                if (density > 1.05f) {
                    val dLeft = left * density - margin
                    val dRight = right * density + margin
                    val dTop = top * density - margin
                    val dBottom = bottom * density + margin
                    if (x in dLeft..dRight && y in dTop..dBottom) {
                        return true
                    }
                }
                return false
            }
        }

        @JvmStatic
        fun parseControlHitboxes(json: String?): List<ControlHitbox> {
            if (json.isNullOrBlank()) return emptyList()
            val trimmed = json.trim()
            if (!trimmed.startsWith("[") || !trimmed.endsWith("]")) return emptyList()
            val hitboxes = mutableListOf<ControlHitbox>()
            HITBOX_OBJECT_RE.findAll(trimmed).forEach { objMatch ->
                val fields = HITBOX_FIELD_RE.findAll(objMatch.value)
                    .associate { it.groupValues[1] to it.groupValues[2].toFloatOrNull() }
                val left = fields["left"]
                val top = fields["top"]
                val right = fields["right"]
                val bottom = fields["bottom"]
                if (left != null && top != null && right != null && bottom != null && right > left && bottom > top) {
                    hitboxes.add(ControlHitbox(left, top, right, bottom))
                }
            }
            return hitboxes
        }

        @JvmStatic
        fun androidKeyToJsKeyCode(keyCode: Int): Int {
            return when (keyCode) {
                in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> keyCode - KeyEvent.KEYCODE_A + 65
                in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> keyCode - KeyEvent.KEYCODE_0 + 48
                in KeyEvent.KEYCODE_NUMPAD_0..KeyEvent.KEYCODE_NUMPAD_9 -> keyCode - KeyEvent.KEYCODE_NUMPAD_0 + 96
                in KeyEvent.KEYCODE_F1..KeyEvent.KEYCODE_F12 -> keyCode - KeyEvent.KEYCODE_F1 + 112
                KeyEvent.KEYCODE_DPAD_UP -> 38
                KeyEvent.KEYCODE_DPAD_DOWN -> 40
                KeyEvent.KEYCODE_DPAD_LEFT -> 37
                KeyEvent.KEYCODE_DPAD_RIGHT -> 39
                KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> 13
                KeyEvent.KEYCODE_ESCAPE, KeyEvent.KEYCODE_BACK -> 27
                KeyEvent.KEYCODE_SPACE -> 32
                KeyEvent.KEYCODE_DEL -> 8
                KeyEvent.KEYCODE_TAB -> 9
                KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.KEYCODE_SHIFT_RIGHT -> 16
                KeyEvent.KEYCODE_CTRL_LEFT, KeyEvent.KEYCODE_CTRL_RIGHT -> 17
                KeyEvent.KEYCODE_ALT_LEFT, KeyEvent.KEYCODE_ALT_RIGHT -> 18
                KeyEvent.KEYCODE_PAGE_UP -> 33
                KeyEvent.KEYCODE_PAGE_DOWN -> 34
                KeyEvent.KEYCODE_MOVE_HOME -> 36
                KeyEvent.KEYCODE_MOVE_END -> 35
                KeyEvent.KEYCODE_INSERT -> 45
                KeyEvent.KEYCODE_FORWARD_DEL -> 46
                else -> keyCode
            }
        }

        @JvmStatic
        fun isTouchInsideToolbar(x: Float, y: Float, density: Float): Boolean {
            val toolbarMaxX = 70f * density
            val toolbarMaxY = 260f * density
            return x in 0f..toolbarMaxX && y in 0f..toolbarMaxY
        }

        @JvmStatic
        fun isTouchInsideModal(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float
        ): Boolean {
            val modalWidth = minOf(screenWidth * 0.92f, 520f * density)
            val modalHeight = screenHeight * 0.86f
            val left = (screenWidth - modalWidth) / 2f
            val right = (screenWidth + modalWidth) / 2f
            val top = (screenHeight - modalHeight) / 2f
            val bottom = (screenHeight + modalHeight) / 2f
            return x in left..right && y in top..bottom
        }

        @JvmStatic
        fun isTouchInsideVirtualGamepad(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            // 0. Precise per-control hitboxes reported by the overlay JS layer.
            //    Only touches landing on (or ~12dp around) a real control are captured;
            //    everything else falls through to the game canvas.
            if (controlHitboxes.isNotEmpty()) {
                val margin = 12f * density
                return controlHitboxes.any { it.containsWithMargin(x, y, margin, density) }
            }

            // 1. D-Pad / Joystick zone (bottom-left) — legacy fallback before JS reports hitboxes.
            val dpadMaxX = maxOf(screenWidth * 0.35f, 220f * density)
            val dpadMinY = minOf(screenHeight * 0.50f, screenHeight - 240f * density)
            if (x in 0f..dpadMaxX && y in dpadMinY..screenHeight) {
                return true
            }

            // 2. Action buttons zone (bottom-right)
            val actionMinX = minOf(screenWidth * 0.65f, screenWidth - 240f * density)
            val actionMinY = minOf(screenHeight * 0.50f, screenHeight - 240f * density)
            if (x in actionMinX..screenWidth && y in actionMinY..screenHeight) {
                return true
            }

            // 3. Top utility buttons zone (top-center)
            val utilMinX = screenWidth * 0.35f
            val utilMaxX = screenWidth * 0.65f
            val utilMaxY = maxOf(screenHeight * 0.15f, 60f * density)
            if (x in utilMinX..utilMaxX && y in 0f..utilMaxY) {
                return true
            }

            return false
        }

        @JvmStatic
        fun isTouchOnActiveControls(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            if (isOverlayModalOpen) {
                return true
            }
            if (isTouchInsideToolbar(x, y, density)) {
                return true
            }
            if (!isGamepadHidden && isTouchInsideVirtualGamepad(x, y, screenWidth, screenHeight, density, controlHitboxes)) {
                return true
            }
            return false
        }

        @JvmStatic
        fun shouldForwardTouchToGame(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            if (isOverlayModalOpen) {
                return false
            }
            return !isTouchOnActiveControls(
                x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen, controlHitboxes
            )
        }

        @JvmStatic
        fun resolveGestureCapturedByGame(
            actionMasked: Int,
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            wasCapturedByGame: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            return if (actionMasked == MotionEvent.ACTION_DOWN) {
                shouldForwardTouchToGame(
                    x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen, controlHitboxes
                )
            } else {
                wasCapturedByGame
            }
        }

        @Volatile
        var currentInstance: WebActivity? = null
    }

    var gameWebView: WebView? = null
    var overlayWebView: WebView? = null
    var vfsInterceptor: VfsResourceInterceptor? = null
    var gpackReader: GpackStreamReader? = null
    var gameDirectory: File? = null
    var webShimsScript: String = ""

    @Volatile
    var overlayTextFocused: Boolean = false

    @Volatile
    var isGamepadHidden: Boolean = false

    @Volatile
    var isOverlayModalOpen: Boolean = false

    @Volatile
    var activeControlHitboxes: List<ControlHitbox> = emptyList()

    @Volatile
    var cachedCheatState: String = "{}"

    @Volatile
    var cachedEvalResult: String = ""

    private var isTouchCapturedByGame: Boolean = false

    @SuppressLint("SetJavaScriptEnabled")
    @Suppress("DEPRECATION")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        requestWindowFeature(Window.FEATURE_NO_TITLE)
        window.setFlags(
            WindowManager.LayoutParams.FLAG_FULLSCREEN,
            WindowManager.LayoutParams.FLAG_FULLSCREEN
        )
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        window.setFlags(
            WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
            WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED
        )

        val gameDir = File(filesDir, "game")
        gameDirectory = gameDir
        currentInstance = this
        AssetAuditService.init(filesDir, "rpgmaker_web", this)
        prepareGameContent(gameDir)
        setupVfs(gameDir)
        loadWebShims()

        setupGameWebView()
        setupOverlayWebView()

        val rootLayout = RelativeLayout(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }

        gameWebView?.let { rootLayout.addView(it) }
        overlayWebView?.let { rootLayout.addView(it) }
        setContentView(rootLayout)

        hideSystemUi()

        Log.i(TAG, "Starting game loading: https://game.local/index.html")
        gameWebView?.loadUrl("https://game.local/index.html")
    }

    private fun hideSystemUi() {
        try {
            val decor = window.peekDecorView() ?: return
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.setDecorFitsSystemWindows(false)
                val controller = decor.windowInsetsController ?: window.insetsController
                if (controller != null) {
                    controller.hide(
                        android.view.WindowInsets.Type.statusBars() or
                            android.view.WindowInsets.Type.navigationBars()
                    )
                    controller.systemBarsBehavior =
                        android.view.WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                }
            } else {
                @Suppress("DEPRECATION")
                decor.systemUiVisibility = (
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        or View.SYSTEM_UI_FLAG_FULLSCREEN
                )
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to hide system UI: ${e.message}")
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            hideSystemUi()
        }
    }

    private fun prepareGameContent(gameDir: File) {
        val isStreaming = GpackExtractor.isEncryptedStreamingMode(this)
        try {
            if (isStreaming) {
                val marker = File(gameDir, ".gpack_extracted")
                if (!marker.exists()) {
                    GpackExtractor.prepareStreamingContainer(this, gameDir)
                }
            } else {
                val hasPackAsset = try {
                    val afd = assets.openFd("game.pack")
                    afd.close()
                    true
                } catch (_: Exception) {
                    false
                }
                if (!hasPackAsset) {
                    val marker = File(gameDir, ".gpack_extracted")
                    if (!marker.exists() || !File(gameDir, "index.html").exists()) {
                        GpackExtractor.extract(this, gameDir)
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed during game preparation: ${e.message}", e)
        }
    }

    private fun setupVfs(gameDir: File) {
        val packFile = File(gameDir, "game.pack")
        if (packFile.exists()) {
            try {
                gpackReader = GpackStreamReader.open(packFile)
                Log.i(TAG, "Opened game.pack from gameDir: ${packFile.absolutePath}")
            } catch (e: Exception) {
                Log.w(TAG, "Could not open packFile: ${e.message}")
            }
        }
        if (gpackReader == null) {
            try {
                val afd = assets.openFd("game.pack")
                gpackReader = GpackStreamReader.open(afd)
                Log.i(TAG, "Opened game.pack from APK assets (${afd.length} bytes, totalFiles=${gpackReader?.totalFiles})")
            } catch (e: Exception) {
                Log.w(TAG, "Could not open game.pack from assets: ${e.message}")
            }
        }

        var rpgMakerKey: String? = null
        try {
            val sysStream = gpackReader?.openStream("data/System.json") ?: File(gameDir, "data/System.json").let { if (it.exists()) it.inputStream() else null }
            sysStream?.bufferedReader()?.use { reader ->
                val content = reader.readText()
                val match = Regex(""""encryptionKey"\s*:\s*"([a-fA-F0-9]{32})"""").find(content)
                if (match != null) {
                    rpgMakerKey = match.groupValues[1]
                    Log.i(TAG, "Detected RPG Maker encryption key: $rpgMakerKey")
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Could not check System.json for encryption key: ${e.message}")
        }

        vfsInterceptor = VfsResourceInterceptor(gpackReader, rpgMakerKey) { path ->
            val resolvedPath = if (path.isEmpty()) "index.html" else path
            val localFile = File(gameDir, resolvedPath)
            if (localFile.exists() && localFile.isFile) {
                localFile.inputStream()
            } else {
                gpackReader?.openStream(resolvedPath)
            }
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupGameWebView() {
        val wv = WebView(this).apply {
            layoutParams = RelativeLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.BLACK)
            setLayerType(View.LAYER_TYPE_HARDWARE, null)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            overScrollMode = View.OVER_SCROLL_NEVER
        }

        with(wv.settings) {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            mediaPlaybackRequiresUserGesture = false
            allowFileAccess = true
            allowContentAccess = true
            loadsImagesAutomatically = true
            useWideViewPort = true
            loadWithOverviewMode = true
            mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
        }

        wv.addJavascriptInterface(WebBridge(InternalStorageSaveService(this)), "RuntimePackNative")
        wv.addJavascriptInterface(object : Any() {
            @JavascriptInterface
            fun setCheatState(stateJson: String) {
                this@WebActivity.setCheatState(stateJson)
            }

            @JavascriptInterface
            fun setEvalResult(resStr: String) {
                this@WebActivity.setEvalResult(resStr)
            }

            @JavascriptInterface
            fun postAuditEvent(eventJson: String): Boolean {
                return this@WebActivity.postAuditEvent(eventJson)
            }

            @JavascriptInterface
            fun getAuditEvents(afterSeq: Long): String {
                return this@WebActivity.getAuditEvents(afterSeq)
            }

            @JavascriptInterface
            fun getAuditReport(): String {
                return this@WebActivity.getAuditReport()
            }

            @JavascriptInterface
            fun cancelAudit(): Boolean {
                return this@WebActivity.cancelAudit()
            }

            @JavascriptInterface
            fun startAudit(requestJson: String): String {
                return this@WebActivity.startAudit(requestJson)
            }

            @JavascriptInterface
            fun getCombinedSummary(): String {
                return this@WebActivity.getCombinedSummary()
            }
        }, "RuntimePackWeb")

        wv.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(consoleMessage: ConsoleMessage?): Boolean {
                if (consoleMessage != null) {
                    Log.i(TAG, "[GameConsole] ${consoleMessage.message()} (${consoleMessage.sourceId()}:${consoleMessage.lineNumber()})")
                }
                return super.onConsoleMessage(consoleMessage)
            }
        }

        wv.webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView?, url: String?, favicon: Bitmap?) {
                super.onPageStarted(view, url, favicon)
                injectJsShims(view)
            }

            override fun shouldInterceptRequest(
                view: WebView?,
                request: WebResourceRequest?
            ): WebResourceResponse? {
                val url = request?.url?.toString() ?: return null
                val response = vfsInterceptor?.intercept(url)
                if (response == null) {
                    Log.w(TAG, "[VFS Unhandled] $url")
                } else if (!url.endsWith(".js") && !url.endsWith(".css")) {
                    Log.i(TAG, "[VFS Handled] $url (${response.mimeType}, enc=${response.encoding})")
                }
                return response ?: super.shouldInterceptRequest(view, request)
            }

            @Deprecated("Deprecated in Java")
            override fun shouldInterceptRequest(
                view: WebView?,
                url: String?
            ): WebResourceResponse? {
                if (url == null) return null
                val response = vfsInterceptor?.intercept(url)
                if (response == null) {
                    Log.w(TAG, "[VFS Unhandled] $url")
                } else if (!url.endsWith(".js") && !url.endsWith(".css")) {
                    Log.i(TAG, "[VFS Handled] $url (${response.mimeType}, enc=${response.encoding})")
                }
                return response ?: @Suppress("DEPRECATION") super.shouldInterceptRequest(view, url)
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                injectJsShims(view)
                val firstFrameScript = """
                    (function() {
                        setTimeout(function() {
                            if (window.RuntimePackNative) { window.RuntimePackNative.onFirstFrame(); }
                        }, 100);
                    })();
                """.trimIndent()
                view?.evaluateJavascript(firstFrameScript, null)

                val diagScript = """
                    (function() {
                        setTimeout(function() {
                            console.log('[RP_DIAG] URL: ' + window.location.href);
                            console.log('[RP_DIAG] Title: ' + document.title);
                            console.log('[RP_DIAG] TYRANO defined: ' + (typeof window.TYRANO));
                            if (window.TYRANO && window.TYRANO.kag) {
                                console.log('[RP_DIAG] current_scenario: ' + (window.TYRANO.kag.stat ? window.TYRANO.kag.stat.current_scenario : 'no stat'));
                            }
                            var base = document.getElementById('tyrano_base');
                            console.log('[RP_DIAG] tyrano_base: ' + (base ? base.innerHTML.length + ' chars, children: ' + base.children.length : 'null'));
                            var imgs = document.querySelectorAll('img');
                            console.log('[RP_DIAG] Total imgs: ' + imgs.length);
                            for (var i = 0; i < imgs.length; i++) {
                                var img = imgs[i];
                                console.log('[RP_DIAG] img[' + i + ']: src=' + img.src + ' w=' + img.naturalWidth + ' h=' + img.naturalHeight + ' comp=' + img.complete + ' op=' + img.style.opacity);
                            }
                            var layers = document.querySelectorAll('.layer');
                            console.log('[RP_DIAG] Total layers: ' + layers.length);
                            for (var j = 0; j < Math.min(layers.length, 10); j++) {
                                var l = layers[j];
                                console.log('[RP_DIAG] layer[' + j + ']: ' + l.className + ' disp=' + l.style.display + ' vis=' + l.style.visibility + ' op=' + l.style.opacity);
                            }
                        }, 2500);
                    })();
                """.trimIndent()
                view?.evaluateJavascript(diagScript, null)
                HealthMarker.GAME_STARTED.emit()
            }
        }

        gameWebView = wv
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupOverlayWebView() {
        val wv = WebView(this).apply {
            layoutParams = RelativeLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.TRANSPARENT)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            overScrollMode = View.OVER_SCROLL_NEVER
        }

        with(wv.settings) {
            javaScriptEnabled = true
            domStorageEnabled = true
            allowFileAccess = true
            allowContentAccess = true
        }

        wv.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(consoleMessage: ConsoleMessage?): Boolean {
                if (consoleMessage != null) {
                    Log.i(TAG, "[OverlayConsole] ${consoleMessage.message()} (${consoleMessage.sourceId()}:${consoleMessage.lineNumber()})")
                }
                return super.onConsoleMessage(consoleMessage)
            }
        }

        wv.addJavascriptInterface(createVPadBridge(), "RuntimePackVPad")

        wv.loadUrl("file:///android_asset/overlay/index.html")
        overlayWebView = wv
    }

    inner class VPadBridge {
        @JavascriptInterface
        fun onKeyDown(jsKeyCode: Int) {
            this@WebActivity.dispatchKeyToGame(jsKeyCode, "keydown")
        }

        @JavascriptInterface
        fun onKeyUp(jsKeyCode: Int) {
            this@WebActivity.dispatchKeyToGame(jsKeyCode, "keyup")
        }

        @JavascriptInterface
        fun openUrl(url: String) {
            this@WebActivity.openUrl(url)
        }

        @JavascriptInterface
        fun openCommunityLink() {
            this@WebActivity.openCommunityLink()
        }

        @JavascriptInterface
        fun postCheatCmd(jsonCmd: String): Boolean {
            return this@WebActivity.postCheatCmd(jsonCmd)
        }

        @JavascriptInterface
        fun getCheatState(): String {
            return this@WebActivity.getCheatState()
        }

        @JavascriptInterface
        fun getEvalResult(): String {
            return this@WebActivity.getEvalResult()
        }

        @JavascriptInterface
        fun setOverlayTextFocused(focused: Boolean) {
            this@WebActivity.overlayTextFocused = focused
        }

        @JavascriptInterface
        fun setGamepadHidden(hidden: Boolean) {
            this@WebActivity.isGamepadHidden = hidden
            if (hidden) {
                runOnUiThread { gameWebView?.requestFocus() }
            }
        }

        @JavascriptInterface
        fun updateControlHitboxes(json: String) {
            this@WebActivity.activeControlHitboxes = parseControlHitboxes(json)
        }

        @JavascriptInterface
        fun setOverlayModalOpen(open: Boolean) {
            this@WebActivity.isOverlayModalOpen = open
            if (open) {
                isTouchCapturedByGame = false
            }
            if (!open) {
                runOnUiThread { gameWebView?.requestFocus() }
            }
        }

        @JavascriptInterface
        fun getAuditProgress(): String {
            return this@WebActivity.getAuditProgress()
        }

        @JavascriptInterface
        fun getAuditEvents(afterSeq: Long): String {
            return this@WebActivity.getAuditEvents(afterSeq)
        }

        @JavascriptInterface
        fun getAuditReport(): String {
            return this@WebActivity.getAuditReport()
        }

        @JavascriptInterface
        fun postAuditEvent(eventJson: String): Boolean {
            return this@WebActivity.postAuditEvent(eventJson)
        }

        @JavascriptInterface
        fun cancelAudit(): Boolean {
            return this@WebActivity.cancelAudit()
        }

        @JavascriptInterface
        fun startAudit(requestJson: String): String {
            return this@WebActivity.startAudit(requestJson)
        }

        @JavascriptInterface
        fun getCombinedSummary(): String {
            return this@WebActivity.getCombinedSummary()
        }

        @JavascriptInterface
        fun shareBugReport(jsonContent: String) {
            this@WebActivity.shareBugReport(jsonContent)
        }
    }

    fun createVPadBridge(): VPadBridge = VPadBridge()

    fun loadWebShims() {
        if (webShimsScript.isNotEmpty()) return
        webShimsScript = try {
            val shims = assets.open("web_shims/runtimepack_web_shims.js").bufferedReader().use { it.readText() }
            val audit = try {
                assets.open("web_shims/runtimepack_asset_audit.js").bufferedReader().use { it.readText() }
            } catch (_: Exception) { "" }
            if (audit.isNotEmpty()) "$shims\n\n$audit" else shims
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load web shims from assets: ${e.message}", e)
            ""
        }
    }

    private fun injectJsShims(view: WebView?) {
        if (webShimsScript.isEmpty()) {
            loadWebShims()
        }
        if (webShimsScript.isNotEmpty()) {
            view?.evaluateJavascript(webShimsScript, null)
        }
    }

    open fun dispatchKeyToGame(jsKeyCode: Int, type: String) {
        runOnUiThread {
            val script = """
                (function() {
                    var codeMap = {
                        8: "Backspace", 9: "Tab", 13: "Enter", 16: "ShiftLeft", 17: "ControlLeft",
                        18: "AltLeft", 19: "Pause", 27: "Escape", 32: "Space", 33: "PageUp",
                        34: "PageDown", 35: "End", 36: "Home", 37: "ArrowLeft", 38: "ArrowUp",
                        39: "ArrowRight", 40: "ArrowDown", 45: "Insert", 46: "Delete",
                        48: "Digit0", 49: "Digit1", 50: "Digit2", 51: "Digit3", 52: "Digit4",
                        53: "Digit5", 54: "Digit6", 55: "Digit7", 56: "Digit8", 57: "Digit9",
                        65: "KeyA", 66: "KeyB", 67: "KeyC", 68: "KeyD", 69: "KeyE", 70: "KeyF",
                        71: "KeyG", 72: "KeyH", 73: "KeyI", 74: "KeyJ", 75: "KeyK", 76: "KeyL",
                        77: "KeyM", 78: "KeyN", 79: "KeyO", 80: "KeyP", 81: "KeyQ", 82: "KeyR",
                        83: "KeyS", 84: "KeyT", 85: "KeyU", 86: "KeyV", 87: "KeyW", 88: "KeyX",
                        89: "KeyY", 90: "KeyZ", 96: "Numpad0", 97: "Numpad1", 98: "Numpad2",
                        99: "Numpad3", 100: "Numpad4", 101: "Numpad5", 102: "Numpad6",
                        103: "Numpad7", 104: "Numpad8", 105: "Numpad9", 108: "NumpadEnter",
                        112: "F1", 113: "F2", 114: "F3", 115: "F4", 116: "F5", 117: "F6",
                        118: "F7", 119: "F8", 120: "F9", 121: "F10", 122: "F11", 123: "F12"
                    };
                    var evt;
                    try {
                        evt = new KeyboardEvent('$type', {
                            bubbles: true,
                            cancelable: true
                        });
                        Object.defineProperty(evt, 'keyCode', {
                            get: function() { return $jsKeyCode; },
                            configurable: true
                        });
                        Object.defineProperty(evt, 'which', {
                            get: function() { return $jsKeyCode; },
                            configurable: true
                        });
                        var evtCode = codeMap[$jsKeyCode] || "";
                        try {
                            Object.defineProperty(evt, 'code', {
                                get: function() { return evtCode; },
                                configurable: true
                            });
                        } catch (e) {}
                    } catch (e) {
                        evt = null;
                    }
                    if (evt) {
                        window.dispatchEvent(evt);
                        document.dispatchEvent(evt);
                    }

                    // Jalur utama: injeksi langsung ke state engine RPG Maker.
                    // (Chromium modern membuang keyCode dari KeyboardEventInit,
                    //  sehingga dispatch DOM saja tidak pernah sampai ke Input.keyMapper.)
                    if (window.Input) {
                        if ('$type' === 'keydown' && typeof window.Input._onKeyDown === 'function' && evt) {
                            window.Input._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.Input._onKeyUp === 'function' && evt) {
                            window.Input._onKeyUp(evt);
                        }
                    }
                    if (window.TouchInput) {
                        if ('$type' === 'keydown' && typeof window.TouchInput._onKeyDown === 'function' && evt) {
                            window.TouchInput._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.TouchInput._onKeyUp === 'function' && evt) {
                            window.TouchInput._onKeyUp(evt);
                        }
                    }

                    // Safety net: jika event tetap tidak diproses engine (keyCode hilang),
                    // set state input secara langsung via RuntimePackInput (Task 1 shim).
                    if (window.RuntimePackInput && window.RuntimePackInput.isAvailable()) {
                        var action = null;
                        if (window.Input && window.Input.keyMapper) {
                            action = window.Input.keyMapper[$jsKeyCode] || null;
                        }
                        if (!action && evt && evt.code && window.Input && window.Input.codeMapper) {
                            action = window.Input.codeMapper[evt.code] || null;
                        }
                        if (action) {
                            if ('$type' === 'keydown') {
                                window.RuntimePackInput.press(action);
                            } else {
                                window.RuntimePackInput.release(action);
                            }
                        }
                    }
                })();
            """.trimIndent()
            gameWebView?.evaluateJavascript(script, null)
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (overlayTextFocused) {
            return super.dispatchKeyEvent(event)
        }

        val keyCode = event.keyCode
        if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN ||
            keyCode == KeyEvent.KEYCODE_VOLUME_UP ||
            keyCode == KeyEvent.KEYCODE_CAMERA ||
            keyCode == KeyEvent.KEYCODE_ZOOM_IN ||
            keyCode == KeyEvent.KEYCODE_ZOOM_OUT
        ) {
            return super.dispatchKeyEvent(event)
        }

        val action = event.action
        if (action == KeyEvent.ACTION_DOWN || action == KeyEvent.ACTION_UP) {
            val type = if (action == KeyEvent.ACTION_DOWN) "keydown" else "keyup"
            val jsKeyCode = androidKeyToJsKeyCode(keyCode)
            dispatchKeyToGame(jsKeyCode, type)
            if (keyCode == KeyEvent.KEYCODE_BACK) {
                return true
            }
            return true
        }

        return super.dispatchKeyEvent(event)
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        val density = resources.displayMetrics.density
        val screenWidth = resources.displayMetrics.widthPixels.toFloat()
        val screenHeight = resources.displayMetrics.heightPixels.toFloat()

        val action = event.actionMasked
        isTouchCapturedByGame = resolveGestureCapturedByGame(
            action,
            event.x,
            event.y,
            screenWidth,
            screenHeight,
            density,
            isGamepadHidden,
            isOverlayModalOpen,
            isTouchCapturedByGame,
            activeControlHitboxes
        )

        if (isTouchCapturedByGame) {
            val copy = MotionEvent.obtain(event)
            try {
                gameWebView?.dispatchTouchEvent(event)
            } finally {
                try {
                    overlayWebView?.dispatchTouchEvent(copy)
                } finally {
                    copy.recycle()
                }
            }
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                isTouchCapturedByGame = false
            }
            return true
        }

        return super.dispatchTouchEvent(event)
    }

    override fun onPause() {
        super.onPause()
        gameWebView?.onPause()
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        gameWebView?.onResume()
        overlayWebView?.onResume()
        hideSystemUi()
    }

    override fun onDestroy() {
        if (currentInstance === this) {
            currentInstance = null
        }
        try {
            gpackReader?.close()
        } catch (_: Exception) {}
        gameWebView?.destroy()
        overlayWebView?.destroy()
        super.onDestroy()
    }

    open fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            startActivity(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open URL $url: ${e.message}", e)
        }
    }

    @JavascriptInterface
    open fun openCommunityLink() {
        openUrl("https://linktr.ee/Veldra.Works")
    }

    open fun postCheatCmd(jsonCmd: String): Boolean {
        runOnUiThread {
            val escaped = jsonCmd.replace("\\", "\\\\").replace("'", "\\'")
            val script = """
                (function() {
                    try {
                        var cmd = JSON.parse('$escaped');
                        if (window.CheatEngine && typeof window.CheatEngine.executeCommand === 'function') {
                            window.CheatEngine.executeCommand(cmd);
                        } else if (window.executeCheatCmd && typeof window.executeCheatCmd === 'function') {
                            window.executeCheatCmd(cmd);
                        }
                    } catch(e) {
                        console.error('[WebActivity] postCheatCmd error:', e);
                    }
                })();
            """.trimIndent()
            gameWebView?.evaluateJavascript(script, null)
        }
        return true
    }

    fun setCheatState(stateJson: String) {
        cachedCheatState = stateJson
    }

    @Synchronized
    fun setEvalResult(resStr: String) {
        cachedEvalResult = resStr
    }

    open fun getCheatState(): String = cachedCheatState

    @Synchronized
    open fun getEvalResult(): String {
        val result = cachedEvalResult
        cachedEvalResult = ""
        return result
    }

    private var cachedAuditProgress: String = ""

    @Synchronized
    fun setAuditProgress(progressText: String) {
        cachedAuditProgress = progressText
    }

    @Synchronized
    open fun getAuditProgress(): String {
        val res = cachedAuditProgress
        cachedAuditProgress = ""
        return res
    }

    open fun getAuditEvents(afterSeq: Long): String {
        return AssetAuditService.getAuditEvents(afterSeq)
    }

    open fun getAuditReport(): String {
        return AssetAuditService.getAuditReport()
    }

    open fun getCombinedSummary(): String {
        return AssetAuditService.getCombinedSummary(filesDir)
    }

    open fun postAuditEvent(eventJson: String): Boolean {
        return try {
            val ev = RuntimeAuditEvent.fromJson(eventJson)
            if (ev != null) {
                AssetAuditService.recordEvent(ev)
                true
            } else {
                false
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse and record audit event: ${e.message}", e)
            false
        }
    }

    open fun cancelAudit(): Boolean {
        return AssetAuditService.cancelAudit()
    }

    open fun startAudit(requestJson: String = ""): String {
        return AssetAuditService.startAudit(requestJson)
    }

    open fun shareBugReport(jsonContent: String) {
        runOnUiThread {
            try {
                val appLabel = try {
                    packageManager.getApplicationLabel(applicationInfo).toString()
                } catch (_: Exception) {
                    "Game"
                }
                val sendIntent = Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_SUBJECT, "Bug Report - $appLabel")
                    putExtra(Intent.EXTRA_TEXT, jsonContent)
                }
                val chooser = Intent.createChooser(sendIntent, "Share Bug Report").apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                startActivity(chooser)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to share bug report: ${e.message}", e)
            }
        }
    }
}
