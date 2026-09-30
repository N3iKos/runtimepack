package io.runtimepack.host

import android.annotation.SuppressLint
import android.app.AlertDialog
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.system.Os
import android.util.Log
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.RelativeLayout
import io.runtimepack.host.WebActivity.Companion.ControlHitbox
import org.libsdl.app.SDLActivity
import org.renpy.android.PythonSDLActivity
import java.io.File

/**
 * SDL-backed runtime activity for Ren'Py games.
 *
 * Inherits from [PythonSDLActivity] so that PyJNIus lookups
 * (`autoclass("org.renpy.android.PythonSDLActivity")`) succeed and can access
 * [PythonSDLActivity.mActivity] and its bridge methods (vibrate, getDPI, openUrl,
 * setWakeLock, etc.).
 *
 * This activity must NOT mount content itself: on Android 15+ `onCreate` is
 * required to call through to `super.onCreate` synchronously, while mounting an
 * ~850 MB `game.pack` cannot run on the UI thread. Content mounting is therefore
 * owned by [RenpyBootstrapActivity], which runs first and only starts this
 * activity once the engine layout is present. Here, SDL initializes against
 * already-mounted content and the SDL thread is started from [onStart].
 */
open class RenpyActivity : PythonSDLActivity() {

    var overlayWebView: WebView? = null
    var isGamepadHidden: Boolean = false
    var isOverlayModalOpen: Boolean = false
    var isTouchCapturedByGame: Boolean = false
    var activeControlHitboxes: List<ControlHitbox> = emptyList()

    companion object {
        private const val TAG = "RenpyActivity"

        const val RENPY_LIBRARY = "renpython"
        const val GAME_DIR_NAME = "game"
        const val PUBLIC_DIR_NAME = "public"
        const val SAVES_DIR_NAME = "saves"
        const val LEGACY_PUBLIC_DIR_NAME = "legacy_public"
        const val MAIN_SCRIPT_NAME = "main.py"
        const val ENGINE_DIR_NAME = "renpy"
        const val PYTHON_LIB_DIR_NAME = "lib"

        const val ENV_ANDROID_PRIVATE = "ANDROID_PRIVATE"
        const val ENV_ANDROID_PUBLIC = "ANDROID_PUBLIC"
        const val ENV_ANDROID_OLD_PUBLIC = "ANDROID_OLD_PUBLIC"
        const val ENV_ANDROID_APK = "ANDROID_APK"
        const val ENV_ANDROID_ARGUMENT = "ANDROID_ARGUMENT"
        const val ENV_RENPY_PATH_TO_SAVES = "RENPY_PATH_TO_SAVES"

        @JvmStatic
        fun gameRoot(filesDir: File): File = File(filesDir, GAME_DIR_NAME)

        @JvmStatic
        fun publicDir(filesDir: File): File = File(filesDir, PUBLIC_DIR_NAME)

        @JvmStatic
        fun saveDir(filesDir: File): File = File(filesDir, SAVES_DIR_NAME)

        @JvmStatic
        fun legacyPublicDir(filesDir: File): File = File(filesDir, LEGACY_PUBLIC_DIR_NAME)

        @JvmStatic
        fun extractionMarker(root: File): File = File(root, GpackExtractor.EXTRACTION_MARKER)

        @JvmStatic
        fun expectedLayout(): List<String> =
            listOf(MAIN_SCRIPT_NAME, ENGINE_DIR_NAME, PYTHON_LIB_DIR_NAME)

        @JvmStatic
        fun missingLayoutEntries(root: File): List<String> =
            expectedLayout().filterNot { File(root, it).exists() }

        @JvmStatic
        fun isGameMounted(root: File): Boolean = missingLayoutEntries(root).isEmpty()

        /**
         * Content is considered mounted only when the extractor's own version
         * marker is current *and* the Ren'Py engine layout is present. A bare
         * marker-existence check would silently skip re-extraction after an app
         * update shipped a newer pack.
         */
        @JvmStatic
        fun isContentMounted(root: File): Boolean =
            GpackExtractor.isMarkerCurrent(root) && isGameMounted(root)

        @JvmStatic
        fun archivesIn(dir: File): List<File> =
            dir.listFiles { f -> f.isFile && f.name.endsWith(".rpa", ignoreCase = true) }
                ?.sortedBy { it.name } ?: emptyList()

        @JvmStatic
        fun shouldLinkSaveDirectory(legacySaves: File): Boolean = !legacySaves.exists()

        @JvmStatic
        fun runtimeEnv(filesDir: File, apkPath: String): Map<String, String> = linkedMapOf(
            ENV_ANDROID_PRIVATE to gameRoot(filesDir).absolutePath,
            ENV_ANDROID_PUBLIC to publicDir(filesDir).absolutePath,
            ENV_ANDROID_OLD_PUBLIC to legacyPublicDir(filesDir).absolutePath,
            ENV_ANDROID_APK to apkPath,
            ENV_ANDROID_ARGUMENT to gameRoot(filesDir).absolutePath,
            ENV_RENPY_PATH_TO_SAVES to saveDir(filesDir).absolutePath
        )

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
            return !WebActivity.isTouchOnActiveControls(
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
            return if (actionMasked == android.view.MotionEvent.ACTION_DOWN) {
                shouldForwardTouchToGame(
                    x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen, controlHitboxes
                )
            } else {
                wasCapturedByGame
            }
        }

        @JvmStatic
        fun mapJsKeyCodeToAndroidKeyCode(jsKeyCode: Int): Int {
            return when (jsKeyCode) {
                38 -> KeyEvent.KEYCODE_DPAD_UP
                40 -> KeyEvent.KEYCODE_DPAD_DOWN
                37 -> KeyEvent.KEYCODE_DPAD_LEFT
                39 -> KeyEvent.KEYCODE_DPAD_RIGHT
                90 -> KeyEvent.KEYCODE_BUTTON_A
                13 -> KeyEvent.KEYCODE_ENTER
                32 -> KeyEvent.KEYCODE_SPACE
                88 -> KeyEvent.KEYCODE_BUTTON_B
                27 -> KeyEvent.KEYCODE_MENU
                16 -> KeyEvent.KEYCODE_SHIFT_LEFT
                17 -> KeyEvent.KEYCODE_CTRL_LEFT
                81 -> KeyEvent.KEYCODE_PAGE_UP
                69 -> KeyEvent.KEYCODE_PAGE_DOWN
                else -> KeyEvent.KEYCODE_UNKNOWN
            }
        }

        @JvmStatic
        fun mapVPadButtonToKeyCode(button: String): Int {
            return when (button.lowercase()) {
                "up" -> KeyEvent.KEYCODE_DPAD_UP
                "down" -> KeyEvent.KEYCODE_DPAD_DOWN
                "left" -> KeyEvent.KEYCODE_DPAD_LEFT
                "right" -> KeyEvent.KEYCODE_DPAD_RIGHT
                "a", "ok" -> KeyEvent.KEYCODE_BUTTON_A
                "b", "cancel" -> KeyEvent.KEYCODE_BUTTON_B
                "menu" -> KeyEvent.KEYCODE_MENU
                "dash", "shift" -> KeyEvent.KEYCODE_SHIFT_LEFT
                else -> KeyEvent.KEYCODE_UNKNOWN
            }
        }
    }

    override fun getLibraries(): Array<String> = arrayOf(RENPY_LIBRARY)

    override fun onCreate(savedInstanceState: Bundle?) {
        // Android 15 enforces a synchronous call-through to super.onCreate();
        // deferring it to a background thread aborts the activity with
        // SuperNotCalledException. Content is mounted beforehand by
        // RenpyBootstrapActivity, so SDL can initialize immediately here.
        super.onCreate(savedInstanceState)

        val root = gameRoot(filesDir)
        if (!isContentMounted(root)) {
            // Defensive: the bootstrap normally guarantees mounted content.
            // onStart() will not start the SDL thread while this is false.
            Log.e(TAG, "RenpyActivity started without mounted content at ${root.absolutePath}")
            showMountError(root, extractionSucceeded = false)
        }
        initOverlayAssetSource()
    }

    override fun onStart() {
        super.onStart()
        // SDL only starts its native thread from onStart on modern Android
        // (see RgssActivity); guard it so an unmounted game never boots.
        if (isContentMounted(gameRoot(filesDir))) {
            resumeNativeThread()
            if (overlayWebView == null) {
                setupOverlayWebView()
            }
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    fun setupOverlayWebView() {
        if (mLayout == null) return
        try {
            val wv = WebView(this)
            wv.layoutParams = RelativeLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            wv.setBackgroundColor(Color.TRANSPARENT)
            wv.isVerticalScrollBarEnabled = false
            wv.isHorizontalScrollBarEnabled = false
            wv.overScrollMode = View.OVER_SCROLL_NEVER
            wv.isFocusable = false
            wv.isFocusableInTouchMode = false

            val settings = wv.settings
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = true
            settings.allowContentAccess = true

            wv.addJavascriptInterface(object : Any() {
                @JavascriptInterface
                fun openCommunityLink() {
                    this@RenpyActivity.openCommunityLink()
                }

                @JavascriptInterface
                fun openUrl(url: String) {
                    try {
                        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        startActivity(intent)
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to open url: ${e.message}", e)
                    }
                }

                @JavascriptInterface
                fun updateControlHitboxes(json: String) {
                    activeControlHitboxes = WebActivity.parseControlHitboxes(json)
                }

                @JavascriptInterface
                fun setGamepadHidden(hidden: Boolean) {
                    isGamepadHidden = hidden
                    if (hidden) {
                        runOnUiThread { mSurface?.requestFocus() }
                    }
                }

                @JavascriptInterface
                fun setOverlayModalOpen(open: Boolean) {
                    isOverlayModalOpen = open
                    runOnUiThread {
                        if (open) {
                            wv.isFocusable = true
                            wv.isFocusableInTouchMode = true
                            wv.requestFocus()
                        } else {
                            wv.isFocusable = false
                            wv.isFocusableInTouchMode = false
                            mSurface?.requestFocus()
                        }
                    }
                }

                @JavascriptInterface
                fun onKeyDown(jsKeyCode: Int) {
                    val keyCode = mapJsKeyCodeToAndroidKeyCode(jsKeyCode)
                    if (keyCode != KeyEvent.KEYCODE_UNKNOWN) {
                        try {
                            onNativeKeyDown(keyCode)
                        } catch (ignored: Throwable) {}
                    }
                }

                @JavascriptInterface
                fun onKeyUp(jsKeyCode: Int) {
                    val keyCode = mapJsKeyCodeToAndroidKeyCode(jsKeyCode)
                    if (keyCode != KeyEvent.KEYCODE_UNKNOWN) {
                        try {
                            onNativeKeyUp(keyCode)
                        } catch (ignored: Throwable) {}
                    }
                }

                @JavascriptInterface
                fun sendKeyEvent(button: String, isDown: Boolean) {
                    val keyCode = mapVPadButtonToKeyCode(button)
                    if (keyCode != KeyEvent.KEYCODE_UNKNOWN) {
                        try {
                            if (isDown) {
                                onNativeKeyDown(keyCode)
                            } else {
                                onNativeKeyUp(keyCode)
                            }
                        } catch (ignored: Throwable) {
                            // In host unit tests where native library is not loaded
                        }
                    }
                }
            }, "RuntimePackVPad")

            wv.webViewClient = OverlayAssetSource.createOverlayWebViewClient(this)
            wv.loadUrl("file:///android_asset/overlay/index.html")
            mLayout.addView(wv)
            overlayWebView = wv
            mSurface?.requestFocus()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to attach overlay: ${e.message}", e)
        }
    }

    private fun initOverlayAssetSource() {
        if (OverlayAssetSource.gpackReader != null) return
        val root = gameRoot(filesDir)
        val packFile = File(root, "game.pack")
        if (packFile.exists()) {
            try {
                OverlayAssetSource.gpackReader = GpackStreamReader.open(packFile)
                Log.i(TAG, "Opened game.pack from gameDir for OverlayAssetSource: ${packFile.absolutePath}")
                return
            } catch (e: Exception) {
                Log.w(TAG, "Could not open packFile for OverlayAssetSource: ${e.message}")
            }
        }
        val packFilesDir = File(filesDir, "game.pack")
        if (packFilesDir.exists()) {
            try {
                OverlayAssetSource.gpackReader = GpackStreamReader.open(packFilesDir)
                Log.i(TAG, "Opened game.pack from filesDir for OverlayAssetSource: ${packFilesDir.absolutePath}")
                return
            } catch (e: Exception) {
                Log.w(TAG, "Could not open packFilesDir for OverlayAssetSource: ${e.message}")
            }
        }
        try {
            val afd = assets.openFd("game.pack")
            OverlayAssetSource.gpackReader = GpackStreamReader.open(afd)
            Log.i(TAG, "Opened game.pack from APK assets for OverlayAssetSource")
            return
        } catch (_: Exception) {}
        try {
            val stream = assets.open("game.pack")
            OverlayAssetSource.gpackReader = GpackStreamReader.open(stream)
            Log.i(TAG, "Opened game.pack stream from APK assets for OverlayAssetSource")
        } catch (e: Exception) {
            Log.w(TAG, "Could not open game.pack from assets for OverlayAssetSource: ${e.message}")
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (mSurface != null) {
            val handled = SDLActivity.handleKeyEvent(mSurface, event.keyCode, event, null)
            if (handled) return true
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
            try {
                mSurface?.onTouch(mSurface, event)
                mSurface?.dispatchTouchEvent(event)
            } catch (ignored: Exception) {}
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                isTouchCapturedByGame = false
            }
            return true
        }

        return super.dispatchTouchEvent(event)
    }

    override fun onPause() {
        super.onPause()
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        overlayWebView?.onResume()
    }

    override fun onDestroy() {
        super.onDestroy()
        overlayWebView?.destroy()
        overlayWebView = null
        try {
            OverlayAssetSource.gpackReader?.close()
            OverlayAssetSource.gpackReader = null
        } catch (_: Exception) {}
    }

    @JavascriptInterface
    open fun openCommunityLink() {
        openUrl("https://linktr.ee/Veldra.Works")
    }

    fun preparePython() {
        PythonSDLActivity.mActivity = this

        val root = gameRoot(filesDir)
        root.mkdirs()

        val saves = saveDir(filesDir)
        saves.mkdirs()

        publicDir(filesDir).mkdirs()
        legacyPublicDir(filesDir).mkdirs()
        linkSaveDirectory(root, saves)

        runtimeEnv(filesDir, applicationInfo.sourceDir).forEach { (name, value) ->
            setEnv(name, value)
            try {
                nativeSetEnv(name, value)
            } catch (ignored: Throwable) {
                // In host unit tests where native library is not loaded
            }
        }

        Log.i(TAG, "preparePython complete: ANDROID_PRIVATE=${root.absolutePath}")
    }

    private fun linkSaveDirectory(root: File, saves: File) {
        val legacySaves = File(root, SAVES_DIR_NAME)
        if (!shouldLinkSaveDirectory(legacySaves)) {
            return
        }
        try {
            Os.symlink(saves.absolutePath, legacySaves.absolutePath)
        } catch (e: Exception) {
            Log.w(TAG, "Could not link ${legacySaves.absolutePath} -> ${saves.absolutePath}: ${e.message}")
        }
    }

    private fun setEnv(name: String, value: String) {
        try {
            Os.setenv(name, value, true)
        } catch (e: Exception) {
            Log.w(TAG, "Could not set $name: ${e.message}")
        }
    }

    private fun showMountError(root: File, extractionSucceeded: Boolean) {
        val missing = missingLayoutEntries(root)
        val reason = buildString {
            append("Game content could not be prepared.\n\n")
            if (!extractionSucceeded) {
                append("\u2022 Failed to extract ${GpackExtractor.GAME_PACK_ASSET}.\n")
            }
            if (missing.isNotEmpty()) {
                append("\u2022 Missing expected layout: ${missing.joinToString(", ")}\n")
            }
            append("\u2022 Game root: ${root.absolutePath}")
        }
        Log.e(TAG, reason)

        try {
            AlertDialog.Builder(this)
                .setTitle("Cannot start game")
                .setMessage(reason)
                .setCancelable(false)
                .setPositiveButton("Exit") { _, _ -> finish() }
                .show()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to show mount error dialog: ${e.message}", e)
        }
    }
}
