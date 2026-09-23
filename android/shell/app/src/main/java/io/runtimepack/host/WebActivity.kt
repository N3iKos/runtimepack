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
            density: Float
        ): Boolean {
            // 1. D-Pad / Joystick zone (bottom-left)
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
            isOverlayModalOpen: Boolean
        ): Boolean {
            if (isTouchInsideToolbar(x, y, density)) {
                return true
            }
            if (isOverlayModalOpen && isTouchInsideModal(x, y, screenWidth, screenHeight, density)) {
                return true
            }
            if (!isGamepadHidden && isTouchInsideVirtualGamepad(x, y, screenWidth, screenHeight, density)) {
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
            isOverlayModalOpen: Boolean
        ): Boolean {
            return !isTouchOnActiveControls(
                x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen
            )
        }
    }

    var gameWebView: WebView? = null
    var overlayWebView: WebView? = null
    var vfsInterceptor: VfsResourceInterceptor? = null
    var gpackReader: GpackStreamReader? = null
    var webShimsScript: String = ""

    @Volatile
    var overlayTextFocused: Boolean = false

    @Volatile
    var isGamepadHidden: Boolean = false

    @Volatile
    var isOverlayModalOpen: Boolean = false

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

        wv.addJavascriptInterface(object : Any() {
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
            fun setOverlayModalOpen(open: Boolean) {
                this@WebActivity.isOverlayModalOpen = open
                if (!open) {
                    runOnUiThread { gameWebView?.requestFocus() }
                }
            }
        }, "RuntimePackVPad")

        wv.loadUrl("file:///android_asset/overlay/index.html")
        overlayWebView = wv
    }

    fun loadWebShims() {
        if (webShimsScript.isNotEmpty()) return
        webShimsScript = try {
            assets.open("web_shims/runtimepack_web_shims.js").bufferedReader().use { it.readText() }
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
                    var evt = new KeyboardEvent('$type', {
                        keyCode: $jsKeyCode,
                        which: $jsKeyCode,
                        bubbles: true,
                        cancelable: true
                    });
                    window.dispatchEvent(evt);
                    document.dispatchEvent(evt);
                    if (window.Input) {
                        if ('$type' === 'keydown' && typeof window.Input._onKeyDown === 'function') {
                            window.Input._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.Input._onKeyUp === 'function') {
                            window.Input._onKeyUp(evt);
                        }
                    }
                    if (window.TouchInput) {
                        if ('$type' === 'keydown' && typeof window.TouchInput._onKeyDown === 'function') {
                            window.TouchInput._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.TouchInput._onKeyUp === 'function') {
                            window.TouchInput._onKeyUp(evt);
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
        if (action == MotionEvent.ACTION_DOWN) {
            isTouchCapturedByGame = shouldForwardTouchToGame(
                event.x,
                event.y,
                screenWidth,
                screenHeight,
                density,
                isGamepadHidden,
                isOverlayModalOpen
            )
        }

        if (isTouchCapturedByGame) {
            try {
                gameWebView?.dispatchTouchEvent(event)
            } catch (ignored: Exception) {
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
}
