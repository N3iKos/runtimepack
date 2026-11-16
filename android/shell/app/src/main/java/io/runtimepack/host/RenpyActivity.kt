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
import java.nio.file.Files

/**
 * How the shipped engine builds `config.searchpath` on Android.
 *
 * [LEGACY] engines (`android_searchpath()` in `renpy/main.py`, Ren'Py 7.x/8.0.x)
 * reset the path and drop `config.commondir`; [MODERN] engines build it from
 * `predefined_searchpath(commondir)` and keep the common directory.
 */
enum class CommonDirWiring { LEGACY, MODERN, UNKNOWN }

/**
 * Filesystem primitives for the app-managed asset-pack link.
 *
 * Injectable because `android.system.Os` is a stub in host unit tests; the
 * default implementation is the only production path.
 */
interface CommonLinkOps {
    fun isSymlink(link: File): Boolean
    fun readLink(link: File): String?
    fun move(from: File, to: File): Boolean
    fun symlink(target: String, link: String)
}

/** Production [CommonLinkOps]: `java.nio.file` for inspection, `Os.symlink` to create. */
object DefaultCommonLinkOps : CommonLinkOps {
    override fun isSymlink(link: File): Boolean = try {
        Files.isSymbolicLink(link.toPath())
    } catch (e: Exception) {
        false
    }

    override fun readLink(link: File): String? = try {
        Files.readSymbolicLink(link.toPath()).toString()
    } catch (e: Exception) {
        null
    }

    override fun move(from: File, to: File): Boolean = try {
        Files.move(from.toPath(), to.toPath())
        true
    } catch (e: Exception) {
        false
    }

    override fun symlink(target: String, link: String) {
        Os.symlink(target, link)
    }
}

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

    /**
     * Written on the UI thread during dispatch, read on the JS bridge thread to
     * decide whether a cancel must be enqueued. `@Volatile` keeps the bridge from
     * acting on a stale value and skipping the cancel entirely.
     */
    @Volatile
    var isTouchCapturedByGame: Boolean = false

    /**
     * Bumped on every gesture start. Mirrors [WebActivity.gestureGeneration]:
     * queued UI work is bound to the generation it was scheduled for, so a cancel
     * meant for an ended gesture can never land on a later one.
     */
    @Volatile
    var gestureGeneration: Long = 0L

    /** Generation already cancelled, so repeated requests cannot double-cancel. */
    @Volatile
    var activeCancelledGestureGen: Long = 0L

    /** Written on the JS bridge thread, read on the UI thread. */
    @Volatile
    var activeToolbarHitboxes: List<ControlHitbox> = emptyList()

    @Volatile
    var lastMoveFeedbackDispatchTimeMs: Long = 0L
    @Volatile
    var pendingMoveCssSample: Pair<Int, Int>? = null
    @Volatile
    var pendingMoveFeedbackRunnable: Runnable? = null
    @Volatile
    var moveFeedbackDispatchCount: Int = 0

    /**
     * Written by the JS bridge thread, read by the UI thread during dispatch.
     * Same visibility contract as [WebActivity.activeToolbarHitboxes].
     */
    @Volatile
    var activeControlHitboxes: List<ControlHitbox> = emptyList()

    /** Forwards game-owned touch samples to the shared overlay effects. */
    private val feedbackHost = object : TouchFeedbackHost {
        override fun evaluateFeedbackScript(script: String) {
            this@RenpyActivity.evaluateFeedbackScript(script)
        }

        override fun postFeedbackRunnable(action: Runnable, delayMs: Long) {
            this@RenpyActivity.postFeedbackRunnable(action, delayMs)
        }

        override fun removeFeedbackRunnable(action: Runnable) {
            this@RenpyActivity.removeFeedbackRunnable(action)
        }

        override fun currentUptimeMs(): Long = this@RenpyActivity.currentUptimeMs()

        override var lastMoveFeedbackDispatchTimeMs: Long
            get() = this@RenpyActivity.lastMoveFeedbackDispatchTimeMs
            set(value) { this@RenpyActivity.lastMoveFeedbackDispatchTimeMs = value }

        override var pendingMoveCssSample: Pair<Int, Int>?
            get() = this@RenpyActivity.pendingMoveCssSample
            set(value) { this@RenpyActivity.pendingMoveCssSample = value }

        override var pendingMoveFeedbackRunnable: Runnable?
            get() = this@RenpyActivity.pendingMoveFeedbackRunnable
            set(value) { this@RenpyActivity.pendingMoveFeedbackRunnable = value }

        override var moveFeedbackDispatchCount: Int
            get() = this@RenpyActivity.moveFeedbackDispatchCount
            set(value) { this@RenpyActivity.moveFeedbackDispatchCount = value }
    }

    companion object {
        private const val TAG = "RenpyActivity"

        private const val LEGACY_ENGINE_MARKER = "android_searchpath("
        private const val MODERN_ENGINE_MARKER = "predefined_searchpath("
        private const val LEGACY_ENTRYPOINT_MARKER = "path_to_common"
        private const val MODERN_ENTRYPOINT_MARKER = "predefined_searchpath"
        private const val STALE_ENTRY_SUFFIX = ".stale"

        /** Cap for reading shipped Python sources while probing; larger files are not guessed at. */
        const val MAX_COMMON_PROBE_BYTES = 256 * 1024

        const val RENPY_LIBRARY = "renpython"
        const val GAME_DIR_NAME = "game"
        const val PUBLIC_DIR_NAME = "public"
        const val SAVES_DIR_NAME = "saves"
        const val LEGACY_PUBLIC_DIR_NAME = "legacy_public"
        const val MAIN_SCRIPT_NAME = "main.py"
        const val ENGINE_DIR_NAME = "renpy"
        const val ENGINE_COMMON_DIR_NAME = "common"
        const val PYTHON_LIB_DIR_NAME = "lib"

        const val ENV_ANDROID_PRIVATE = "ANDROID_PRIVATE"
        const val ENV_ANDROID_PUBLIC = "ANDROID_PUBLIC"
        const val ENV_ANDROID_OLD_PUBLIC = "ANDROID_OLD_PUBLIC"
        const val ENV_ANDROID_APK = "ANDROID_APK"
        const val ENV_ANDROID_ARGUMENT = "ANDROID_ARGUMENT"
        const val ENV_RENPY_PATH_TO_SAVES = "RENPY_PATH_TO_SAVES"
        const val ENV_ANDROID_PACK_FF1 = "ANDROID_PACK_FF1"
        const val RENPY_ASSET_PACK_DIR_NAME = "renpy_assetpack"

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
        fun assetPackRoot(filesDir: File): File = File(filesDir, RENPY_ASSET_PACK_DIR_NAME)

        /**
         * Ren'Py 7.x and 8.0.x rebuild `config.searchpath` on Android inside
         * `renpy/main.py android_searchpath()`, which also sets
         * `config.commondir = None`. The extracted `gameRoot/renpy/common` is
         * then never searched, so `load_module("_errorhandling")` fails.
         * Ren'Py 8.1+ instead builds the path from the entrypoint's
         * `predefined_searchpath(commondir)`, which appends the common
         * directory.
         *
         * `<gameRoot>/renpy/main.py` is the authoritative signal because it is
         * the module that assigns `config.searchpath`. The distributor
         * entrypoint (`<gameRoot>/main.py`) is only a fallback for engines whose
         * module is absent. Anything ambiguous, oversized or unreadable yields
         * [CommonDirWiring.UNKNOWN] so the host changes nothing and logs why.
         */
        @JvmStatic
        fun engineCommonWiring(gameRoot: File): CommonDirWiring {
            val engineModule = File(File(gameRoot, ENGINE_DIR_NAME), MAIN_SCRIPT_NAME)
            val engineSource = if (engineModule.isFile) readProbeHead(engineModule) else null
            val entrypointSource = readProbeHead(File(gameRoot, MAIN_SCRIPT_NAME))

            val wiring = when {
                // Present but unreadable/oversized: do not guess from the entrypoint.
                engineModule.isFile && engineSource == null -> CommonDirWiring.UNKNOWN
                engineSource != null -> classifyEngineModule(engineSource)
                    ?: entrypointWiring(entrypointSource)
                else -> entrypointWiring(entrypointSource)
            }

            Log.i(
                TAG,
                "Common search path wiring: $wiring " +
                    "(engine_module=${engineSource != null}, entrypoint=${entrypointSource != null})"
            )
            return wiring
        }

        private fun classifyEngineModule(source: String): CommonDirWiring? {
            val legacy = source.contains(LEGACY_ENGINE_MARKER)
            val modern = source.contains(MODERN_ENGINE_MARKER)
            return when {
                legacy && modern -> CommonDirWiring.UNKNOWN
                legacy -> CommonDirWiring.LEGACY
                modern -> CommonDirWiring.MODERN
                else -> null
            }
        }

        private fun entrypointWiring(source: String?): CommonDirWiring = when {
            source == null -> CommonDirWiring.UNKNOWN
            source.contains(MODERN_ENTRYPOINT_MARKER) -> CommonDirWiring.MODERN
            source.contains(LEGACY_ENTRYPOINT_MARKER) -> CommonDirWiring.LEGACY
            else -> CommonDirWiring.UNKNOWN
        }

        /** Reads a shipped Python file, refusing to load more than [MAX_COMMON_PROBE_BYTES]. */
        private fun readProbeHead(file: File): String? {
            if (!file.isFile) return null
            val length = file.length()
            if (length > MAX_COMMON_PROBE_BYTES) {
                Log.w(TAG, "Not probing ${file.absolutePath}: $length bytes exceeds $MAX_COMMON_PROBE_BYTES")
                return null
            }
            return try {
                file.readText()
            } catch (e: Exception) {
                Log.w(TAG, "Could not read ${file.absolutePath}: ${e.message}")
                null
            }
        }

        /**
         * Extra environment for engines that drop `config.commondir` on
         * Android. `ANDROID_PACK_FF1` is the engine's own asset-pack hook: it
         * appends `<value>/renpy/common` (and `<value>/game`, which is absent
         * here) after the game directory. Pointing it at a dedicated pack root
         * keeps the game directory in the search path exactly once.
         */
        @JvmStatic
        fun commonSearchPathEnv(
            filesDir: File,
            gameRoot: File,
            wiring: CommonDirWiring = engineCommonWiring(gameRoot)
        ): Map<String, String> =
            if (wiring == CommonDirWiring.LEGACY)
                mapOf(ENV_ANDROID_PACK_FF1 to assetPackRoot(filesDir).absolutePath)
            else
                emptyMap()

        /** The exact environment handed to Python; the only merge point for the legacy pack. */
        @JvmStatic
        fun pythonEnv(
            filesDir: File,
            apkPath: String,
            gameRoot: File,
            wiring: CommonDirWiring = engineCommonWiring(gameRoot)
        ): Map<String, String> = runtimeEnv(filesDir, apkPath) + commonSearchPathEnv(filesDir, gameRoot, wiring)

        /**
         * Materializes `<filesDir>/renpy_assetpack/renpy` pointing at
         * `<gameRoot>/renpy` so the engine can resolve
         * `<pack>/renpy/common`.
         *
         * Only that exact app-managed child is ever touched. A pre-existing
         * entry is inspected without following it: a link to a different target
         * (including a dangling one) or a foreign tree/copy is preserved by a
         * unique sibling rename and never deleted, so a stale layout can be
         * repaired on the next launch instead of bricking the app.
         */
        @JvmStatic
        fun prepareCommonSearchPath(
            filesDir: File,
            gameRoot: File,
            ops: CommonLinkOps = DefaultCommonLinkOps
        ): Boolean {
            val root = assetPackRoot(filesDir)
            val link = File(root, ENGINE_DIR_NAME)
            val expectedTarget = File(gameRoot, ENGINE_DIR_NAME).absolutePath
            val common = File(link, ENGINE_COMMON_DIR_NAME)

            if (link.parentFile?.absoluteFile != root.absoluteFile) {
                Log.e(TAG, "Refusing to manage unexpected asset-pack path ${link.absolutePath}")
                return false
            }

            if (ops.isSymlink(link) && samePath(ops.readLink(link), expectedTarget) && common.isDirectory) {
                return true
            }

            root.mkdirs()

            if (ops.isSymlink(link)) {
                // Includes dangling links: compare the link text, never follow it.
                if (!samePath(ops.readLink(link), expectedTarget) && !preserveEntry(link, ops)) {
                    return false
                }
            } else if (link.exists() && !preserveEntry(link, ops)) {
                // A foreign tree or copy in our slot: keep it as a sibling, never delete it.
                return false
            }

            if (!common.isDirectory) {
                try {
                    ops.symlink(expectedTarget, link.absolutePath)
                } catch (e: Exception) {
                    Log.e(TAG, "Could not link ${link.absolutePath} -> $expectedTarget: ${e.message}")
                }
            }

            return common.isDirectory
        }

        private fun preserveEntry(entry: File, ops: CommonLinkOps): Boolean {
            val target = uniqueStaleSibling(entry, ops)
            if (!ops.move(entry, target)) {
                Log.e(TAG, "Could not preserve ${entry.absolutePath}; leaving it untouched")
                return false
            }
            Log.w(TAG, "Preserved ${entry.absolutePath} as ${target.absolutePath}")
            return true
        }

        private fun uniqueStaleSibling(entry: File, ops: CommonLinkOps): File {
            val parent = entry.parentFile
            var candidate = File(parent, entry.name + STALE_ENTRY_SUFFIX)
            var index = 2
            while (candidate.exists() || ops.isSymlink(candidate)) {
                candidate = File(parent, entry.name + STALE_ENTRY_SUFFIX + "-" + index)
                index += 1
            }
            return candidate
        }

        private fun samePath(left: String?, right: String): Boolean =
            left != null && File(left).absolutePath == File(right).absolutePath

        @JvmStatic
        fun shouldForwardTouchToGame(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList(),
            toolbarHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            if (isOverlayModalOpen) {
                return false
            }
            return !WebActivity.isTouchOnActiveControls(
                x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen,
                controlHitboxes, toolbarHitboxes
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
            controlHitboxes: List<ControlHitbox> = emptyList(),
            toolbarHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            return if (actionMasked == android.view.MotionEvent.ACTION_DOWN) {
                shouldForwardTouchToGame(
                    x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen,
                    controlHitboxes, toolbarHitboxes
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
        if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return

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
                fun updateToolbarHitboxes(json: String) {
                    activeToolbarHitboxes = WebActivity.parseControlHitboxes(json)
                }

                @JavascriptInterface
                fun setGamepadHidden(hidden: Boolean) {
                    onGamepadHiddenChanged(hidden)
                    if (hidden) {
                        runOnUiThread { mSurface?.requestFocus() }
                    }
                }

                @JavascriptInterface
                fun setOverlayModalOpen(open: Boolean) {
                    onOverlayModalOpenChanged(open)
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

        val handled = handleTouchDispatch(
            action = event.actionMasked,
            x = event.x,
            y = event.y,
            screenWidth = screenWidth,
            screenHeight = screenHeight,
            density = density
        ) {
            try {
                mSurface?.onTouch(mSurface, event)
                mSurface?.dispatchTouchEvent(event)
            } catch (ignored: Exception) {
            }
        }
        if (handled) return true

        return super.dispatchTouchEvent(event)
    }

    /**
     * Routes one touch sample: game-owned gestures reach the SDL surface and are
     * mirrored to the overlay effects; overlay-owned gestures do neither, so a
     * single tap never produces a duplicate effect or a duplicate SDL dispatch.
     */
    open fun handleTouchDispatch(
        action: Int,
        x: Float,
        y: Float,
        screenWidth: Float,
        screenHeight: Float,
        density: Float,
        dispatchToSurface: () -> Unit
    ): Boolean {
        if (action == MotionEvent.ACTION_DOWN) {
            // New gesture: invalidate any cancel queued for the previous one.
            ++gestureGeneration
        }

        isTouchCapturedByGame = resolveGestureCapturedByGame(
            action,
            x,
            y,
            screenWidth,
            screenHeight,
            density,
            isGamepadHidden,
            isOverlayModalOpen,
            isTouchCapturedByGame,
            activeControlHitboxes,
            activeToolbarHitboxes
        )

        if (!isTouchCapturedByGame) {
            return false
        }

        dispatchToSurface()
        forwardFeedbackSample(action, x, y, density)
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            isTouchCapturedByGame = false
        }
        return true
    }

    /** Runs a feedback script on the overlay WebView that owns the ripple/trail effects. */
    open fun evaluateFeedbackScript(script: String) {
        runOnUiThread {
            overlayWebView?.evaluateJavascript(script, null)
        }
    }

    open fun postFeedbackRunnable(action: Runnable, delayMs: Long) {
        val wv = overlayWebView
        if (wv != null) {
            wv.postDelayed(action, delayMs)
        } else {
            runOnUiThread(action)
        }
    }

    open fun removeFeedbackRunnable(action: Runnable) {
        overlayWebView?.removeCallbacks(action)
    }

    open fun currentUptimeMs(): Long = try {
        android.os.SystemClock.uptimeMillis()
    } catch (_: Exception) {
        System.currentTimeMillis()
    }

    /** Applies a gamepad visibility change and drops feedback queued against the old state. */
    open fun onGamepadHiddenChanged(hidden: Boolean) {
        isGamepadHidden = hidden
        TouchFeedbackForwarding.reset(feedbackHost)
    }

    /** Applies a modal ownership change and drops feedback queued behind the modal. */
    open fun onOverlayModalOpenChanged(open: Boolean) {
        isOverlayModalOpen = open
        if (open) {
            // Queued samples and a live trail would otherwise survive behind the
            // modal, and a gesture the game already owns would keep streaming
            // MOVE/UP into SDL. Cancel it exactly once and release ownership,
            // mirroring WebActivity.onOverlayModalOpenChanged semantics.
            TouchFeedbackForwarding.reset(feedbackHost)
            cancelCapturedGameGesture()
        }
    }

    /**
     * Queues UI work. The JS bridge calls into this activity from the WebView
     * thread, while [android.view.View.dispatchTouchEvent] and the captured-gesture
     * state belong to the UI thread.
     */
    open fun dispatchUiRunnable(action: Runnable) {
        runOnUiThread(action)
    }

    /**
     * Cancels the gesture that is currently owned by the game. The bridge thread
     * only snapshots the generation and schedules UI work; ownership is checked on
     * the UI thread against that same generation, so:
     * - a gesture that ended before the task runs is not cancelled afterwards, and
     * - a gesture started afterwards is never cancelled by this stale request.
     */
    open fun cancelCapturedGameGesture() {
        if (!isTouchCapturedByGame) return
        val targetGesture = gestureGeneration
        if (targetGesture == activeCancelledGestureGen) return
        dispatchUiRunnable {
            if (targetGesture != gestureGeneration || !isTouchCapturedByGame) return@dispatchUiRunnable
            activeCancelledGestureGen = targetGesture
            isTouchCapturedByGame = false
            TouchFeedbackForwarding.clearPending(feedbackHost)
            dispatchCancelToSurface()
        }
    }

    /** Delivers one ACTION_CANCEL to the SDL surface. Must run on the UI thread. */
    open fun dispatchCancelToSurface() {
        val now = android.os.SystemClock.uptimeMillis()
        val cancelEvent = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0f, 0f, 0)
        try {
            mSurface?.dispatchTouchEvent(cancelEvent)
        } catch (ignored: Exception) {
        } finally {
            cancelEvent.recycle()
        }
    }

    /**
     * Forwards a game-captured touch sample to the shared overlay effects using
     * the same semantics as [WebActivity.forwardFeedbackSample].
     */
    fun forwardFeedbackSample(
        action: Int,
        x: Float,
        y: Float,
        density: Float = try { resources?.displayMetrics?.density ?: 1.0f } catch (_: Exception) { 1.0f }
    ) {
        TouchFeedbackForwarding.forward(feedbackHost, action, x, y, density)
    }

    override fun onPause() {
        TouchFeedbackForwarding.reset(feedbackHost)
        super.onPause()
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        overlayWebView?.onResume()
    }

    override fun onDestroy() {
        TouchFeedbackForwarding.reset(feedbackHost)
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

        val wiring = engineCommonWiring(root)
        if (wiring == CommonDirWiring.LEGACY && !prepareCommonSearchPath(filesDir, root)) {
            // Honest diagnostic: the legacy engine cannot load its common scripts
            // without this, and the failure would otherwise surface as an opaque
            // "Module _errorhandling could not be loaded" traceback.
            Log.e(
                TAG,
                "Ren'Py 7.x/8.0.x drops config.commondir on Android but " +
                    "${root.absolutePath}/$ENGINE_DIR_NAME/$ENGINE_COMMON_DIR_NAME is not reachable via " +
                    "$ENV_ANDROID_PACK_FF1=${assetPackRoot(filesDir).absolutePath}; " +
                    "the engine will fail to load _errorhandling"
            )
        }

        pythonEnv(filesDir, applicationInfo.sourceDir, root, wiring).forEach { (name, value) ->
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
