package io.runtimepack.host

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.VibrationEffect
import android.os.Vibrator
import android.util.Log
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.RelativeLayout
import org.libsdl.app.SDLActivity
import java.io.File
import java.util.concurrent.ConcurrentHashMap

open class RgssActivity : SDLActivity() {

    companion object {
        private const val TAG = "RgssActivity"
        const val INPUT_STATE_FILE = "rp_input_state.bin"
        const val CHEAT_CMD_FILE = "rp_cheat_cmd.json"
        const val CHEAT_STATE_FILE = "rp_cheat_state.json"
        const val CHEAT_EVAL_FILE = "rp_cheat_eval_res.txt"

        @JvmField
        var GAME_PATH: String = ""

        @JvmStatic
        fun hasVibrator(): Boolean {
            val vib = mSingleton?.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
            return vib?.hasVibrator() ?: false
        }

        @JvmStatic
        fun vibrate(duration: Int) {
            val vib = mSingleton?.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator ?: return
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                vib.vibrate(VibrationEffect.createOneShot(duration.toLong(), VibrationEffect.DEFAULT_AMPLITUDE))
            } else {
                @Suppress("DEPRECATION")
                vib.vibrate(duration.toLong())
            }
        }

        @JvmStatic
        fun vibrateStop() {
            val vib = mSingleton?.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
            vib?.cancel()
        }

        @JvmStatic
        fun inMultiWindow(activity: Activity): Boolean {
            return Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && activity.isInMultiWindowMode
        }

        @JvmStatic
        fun mapJsKey(jsKey: Int): Pair<List<Int>, List<Int>> {
            return when (jsKey) {
                90 -> Pair(
                    listOf(KeyEvent.KEYCODE_Z, KeyEvent.KEYCODE_ENTER),
                    listOf(0x5A, 0x0D)
                )
                88 -> Pair(
                    listOf(KeyEvent.KEYCODE_X, KeyEvent.KEYCODE_ESCAPE),
                    listOf(0x58, 0x1B)
                )
                38 -> Pair(listOf(KeyEvent.KEYCODE_DPAD_UP), listOf(0x26))
                40 -> Pair(listOf(KeyEvent.KEYCODE_DPAD_DOWN), listOf(0x28))
                37 -> Pair(listOf(KeyEvent.KEYCODE_DPAD_LEFT), listOf(0x25))
                39 -> Pair(listOf(KeyEvent.KEYCODE_DPAD_RIGHT), listOf(0x27))
                16 -> Pair(listOf(KeyEvent.KEYCODE_SHIFT_LEFT), listOf(0x10))
                17 -> Pair(listOf(KeyEvent.KEYCODE_CTRL_LEFT), listOf(0x11))
                18 -> Pair(listOf(KeyEvent.KEYCODE_ALT_LEFT), listOf(0x12))
                19 -> Pair(listOf(KeyEvent.KEYCODE_BREAK), listOf(0x13))
                20 -> Pair(listOf(KeyEvent.KEYCODE_CAPS_LOCK), listOf(0x14))
                27 -> Pair(listOf(KeyEvent.KEYCODE_ESCAPE), listOf(0x1B))
                13 -> Pair(listOf(KeyEvent.KEYCODE_ENTER), listOf(0x0D))
                32 -> Pair(listOf(KeyEvent.KEYCODE_SPACE), listOf(0x20))
                33 -> Pair(listOf(KeyEvent.KEYCODE_PAGE_UP), listOf(0x21))
                34 -> Pair(listOf(KeyEvent.KEYCODE_PAGE_DOWN), listOf(0x22))
                35 -> Pair(listOf(KeyEvent.KEYCODE_MOVE_END), listOf(0x23))
                36 -> Pair(listOf(KeyEvent.KEYCODE_MOVE_HOME), listOf(0x24))
                45 -> Pair(listOf(KeyEvent.KEYCODE_INSERT), listOf(0x2D))
                46 -> Pair(listOf(KeyEvent.KEYCODE_FORWARD_DEL), listOf(0x2E))
                81 -> Pair(listOf(KeyEvent.KEYCODE_Q), listOf(0x51))
                69 -> Pair(listOf(KeyEvent.KEYCODE_E), listOf(0x45))
                in 48..57 -> Pair(
                    listOf(KeyEvent.KEYCODE_0 + (jsKey - 48)),
                    listOf(jsKey)
                )
                in 65..90 -> Pair(
                    listOf(KeyEvent.KEYCODE_A + (jsKey - 65)),
                    listOf(jsKey)
                )
                in 96..105 -> Pair(
                    listOf(KeyEvent.KEYCODE_NUMPAD_0 + (jsKey - 96)),
                    listOf(0x60 + (jsKey - 96))
                )
                106 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_MULTIPLY), listOf(0x6A))
                107 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_ADD), listOf(0x6B))
                108 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_COMMA), listOf(0x6C))
                109 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_SUBTRACT), listOf(0x6D))
                110 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_DOT), listOf(0x6E))
                111 -> Pair(listOf(KeyEvent.KEYCODE_NUMPAD_DIVIDE), listOf(0x6F))
                9 -> Pair(listOf(KeyEvent.KEYCODE_TAB), listOf(0x09))
                8 -> Pair(listOf(KeyEvent.KEYCODE_DEL), listOf(0x08))
                in 112..123 -> Pair(
                    listOf(KeyEvent.KEYCODE_F1 + (jsKey - 112)),
                    listOf(0x70 + (jsKey - 112))
                )
                186 -> Pair(listOf(KeyEvent.KEYCODE_SEMICOLON), listOf(0xBA))
                187 -> Pair(listOf(KeyEvent.KEYCODE_EQUALS), listOf(0xBB))
                188 -> Pair(listOf(KeyEvent.KEYCODE_COMMA), listOf(0xBC))
                189 -> Pair(listOf(KeyEvent.KEYCODE_MINUS), listOf(0xBD))
                190 -> Pair(listOf(KeyEvent.KEYCODE_PERIOD), listOf(0xBE))
                191 -> Pair(listOf(KeyEvent.KEYCODE_SLASH), listOf(0xBF))
                192 -> Pair(listOf(KeyEvent.KEYCODE_GRAVE), listOf(0xC0))
                219 -> Pair(listOf(KeyEvent.KEYCODE_LEFT_BRACKET), listOf(0xDB))
                220 -> Pair(listOf(KeyEvent.KEYCODE_BACKSLASH), listOf(0xDC))
                221 -> Pair(listOf(KeyEvent.KEYCODE_RIGHT_BRACKET), listOf(0xDD))
                222 -> Pair(listOf(KeyEvent.KEYCODE_APOSTROPHE), listOf(0xDE))
                else -> Pair(emptyList(), emptyList())
            }
        }

        @JvmStatic
        @JvmOverloads
        fun mapAndroidKeyToVk(keyCode: Int, isNumLockOn: Boolean = true): Int {
            if (!isNumLockOn) {
                when (keyCode) {
                    KeyEvent.KEYCODE_NUMPAD_8 -> return 0x26 // VK_UP
                    KeyEvent.KEYCODE_NUMPAD_2 -> return 0x28 // VK_DOWN
                    KeyEvent.KEYCODE_NUMPAD_4 -> return 0x25 // VK_LEFT
                    KeyEvent.KEYCODE_NUMPAD_6 -> return 0x27 // VK_RIGHT
                    KeyEvent.KEYCODE_NUMPAD_7 -> return 0x24 // VK_HOME
                    KeyEvent.KEYCODE_NUMPAD_1 -> return 0x23 // VK_END
                    KeyEvent.KEYCODE_NUMPAD_9 -> return 0x21 // VK_PRIOR
                    KeyEvent.KEYCODE_NUMPAD_3 -> return 0x22 // VK_NEXT
                    KeyEvent.KEYCODE_NUMPAD_5 -> return 0x0C // VK_CLEAR
                }
            }
            return when (keyCode) {
                in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> 0x41 + (keyCode - KeyEvent.KEYCODE_A)
                in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> 0x30 + (keyCode - KeyEvent.KEYCODE_0)
                in KeyEvent.KEYCODE_NUMPAD_0..KeyEvent.KEYCODE_NUMPAD_9 -> 0x60 + (keyCode - KeyEvent.KEYCODE_NUMPAD_0)
                KeyEvent.KEYCODE_NUMPAD_MULTIPLY -> 0x6A
                KeyEvent.KEYCODE_NUMPAD_ADD -> 0x6B
                KeyEvent.KEYCODE_NUMPAD_COMMA -> 0x6C
                KeyEvent.KEYCODE_NUMPAD_SUBTRACT -> 0x6D
                KeyEvent.KEYCODE_NUMPAD_DOT -> 0x6E
                KeyEvent.KEYCODE_NUMPAD_DIVIDE -> 0x6F
                KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_DPAD_CENTER -> 0x0D
                KeyEvent.KEYCODE_SPACE -> 0x20
                KeyEvent.KEYCODE_ESCAPE, KeyEvent.KEYCODE_BACK -> 0x1B
                KeyEvent.KEYCODE_DPAD_UP -> 0x26
                KeyEvent.KEYCODE_DPAD_DOWN -> 0x28
                KeyEvent.KEYCODE_DPAD_LEFT -> 0x25
                KeyEvent.KEYCODE_DPAD_RIGHT -> 0x27
                KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.KEYCODE_SHIFT_RIGHT -> 0x10
                KeyEvent.KEYCODE_CTRL_LEFT, KeyEvent.KEYCODE_CTRL_RIGHT -> 0x11
                KeyEvent.KEYCODE_ALT_LEFT, KeyEvent.KEYCODE_ALT_RIGHT -> 0x12
                KeyEvent.KEYCODE_TAB -> 0x09
                KeyEvent.KEYCODE_DEL -> 0x08
                KeyEvent.KEYCODE_FORWARD_DEL -> 0x2E
                KeyEvent.KEYCODE_INSERT -> 0x2D
                KeyEvent.KEYCODE_PAGE_UP -> 0x21
                KeyEvent.KEYCODE_PAGE_DOWN -> 0x22
                KeyEvent.KEYCODE_MOVE_HOME -> 0x24
                KeyEvent.KEYCODE_MOVE_END -> 0x23
                KeyEvent.KEYCODE_CAPS_LOCK -> 0x14
                KeyEvent.KEYCODE_BREAK -> 0x13
                in KeyEvent.KEYCODE_F1..KeyEvent.KEYCODE_F12 -> 0x70 + (keyCode - KeyEvent.KEYCODE_F1)
                KeyEvent.KEYCODE_SEMICOLON -> 0xBA
                KeyEvent.KEYCODE_EQUALS -> 0xBB
                KeyEvent.KEYCODE_COMMA -> 0xBC
                KeyEvent.KEYCODE_MINUS -> 0xBD
                KeyEvent.KEYCODE_PERIOD -> 0xBE
                KeyEvent.KEYCODE_SLASH -> 0xBF
                KeyEvent.KEYCODE_GRAVE -> 0xC0
                KeyEvent.KEYCODE_LEFT_BRACKET -> 0xDB
                KeyEvent.KEYCODE_BACKSLASH -> 0xDC
                KeyEvent.KEYCODE_RIGHT_BRACKET -> 0xDD
                KeyEvent.KEYCODE_APOSTROPHE -> 0xDE
                else -> -1
            }
        }

        @JvmStatic
        fun writeVirtualKeyBufferAtomic(buffer: ByteArray, targetDir: File): Boolean {
            return try {
                if (!targetDir.exists()) {
                    targetDir.mkdirs()
                }
                val targetFile = File(targetDir, INPUT_STATE_FILE)
                val tempFile = File(targetDir, "$INPUT_STATE_FILE.tmp")
                val snapshot = synchronized(buffer) { buffer.clone() }
                tempFile.writeBytes(snapshot)
                try {
                    java.nio.file.Files.move(
                        tempFile.toPath(),
                        targetFile.toPath(),
                        java.nio.file.StandardCopyOption.ATOMIC_MOVE,
                        java.nio.file.StandardCopyOption.REPLACE_EXISTING
                    )
                } catch (e: java.nio.file.AtomicMoveNotSupportedException) {
                    java.nio.file.Files.move(
                        tempFile.toPath(),
                        targetFile.toPath(),
                        java.nio.file.StandardCopyOption.REPLACE_EXISTING
                    )
                } catch (e: Exception) {
                    if (!tempFile.renameTo(targetFile)) {
                        tempFile.copyTo(targetFile, overwrite = true)
                        tempFile.delete()
                    }
                }
                true
            } catch (e: Exception) {
                Log.e(TAG, "Failed to write virtual key buffer: ${e.message}", e)
                false
            }
        }

        @JvmStatic
        fun writeAtomic(targetFile: File, content: String): Boolean {
            return try {
                val targetDir = targetFile.parentFile ?: File(".")
                if (!targetDir.exists()) {
                    targetDir.mkdirs()
                }
                val tempFile = File(targetDir, "${targetFile.name}.tmp")
                tempFile.writeText(content, Charsets.UTF_8)
                try {
                    java.nio.file.Files.move(
                        tempFile.toPath(),
                        targetFile.toPath(),
                        java.nio.file.StandardCopyOption.ATOMIC_MOVE,
                        java.nio.file.StandardCopyOption.REPLACE_EXISTING
                    )
                } catch (e: java.nio.file.AtomicMoveNotSupportedException) {
                    java.nio.file.Files.move(
                        tempFile.toPath(),
                        targetFile.toPath(),
                        java.nio.file.StandardCopyOption.REPLACE_EXISTING
                    )
                } catch (e: Exception) {
                    if (!tempFile.renameTo(targetFile)) {
                        tempFile.copyTo(targetFile, overwrite = true)
                        tempFile.delete()
                    }
                }
                true
            } catch (e: Exception) {
                Log.e(TAG, "Failed to write atomic file ${targetFile.name}: ${e.message}", e)
                false
            }
        }
    }

    val virtualKeyBuffer = ByteArray(256)
    val pressedJsKeys: MutableSet<Int> = ConcurrentHashMap.newKeySet()
    private var overlayWebView: WebView? = null

    @Volatile
    private var overlayTextFocused = false

    @Volatile
    private var isGamepadHidden = false

    @Volatile
    private var isOverlayModalOpen = false

    fun flushVirtualKeyBuffer() {
        val dir = getInputStateDirectory()
        writeVirtualKeyBufferAtomic(virtualKeyBuffer, dir)
        val parent = dir.parentFile
        if (parent != null && parent.exists() && parent != dir) {
            writeVirtualKeyBufferAtomic(virtualKeyBuffer, parent)
        }
    }

    override fun getLibraries(): Array<String> {
        return arrayOf(
            "c++_shared",
            "SDL2",
            "SDL2_image",
            "SDL2_ttf",
            "SDL2_sound",
            "openal",
            "ruby",
            "mkxp-z"
        )
    }

    override fun getArguments(): Array<String> {
        return arrayOf()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        // When no overlay text field is focused, route hardware keys straight to SDL and virtualKeyBuffer.
        if (!overlayTextFocused) {
            val keyCode = event.keyCode
            // Preserve SDL's special-key handling: let the system handle volume/camera/zoom keys.
            if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN ||
                keyCode == KeyEvent.KEYCODE_VOLUME_UP ||
                keyCode == KeyEvent.KEYCODE_CAMERA ||
                keyCode == KeyEvent.KEYCODE_ZOOM_IN ||
                keyCode == KeyEvent.KEYCODE_ZOOM_OUT) {
                return super.dispatchKeyEvent(event)
            }

            val isAlt = event.isAltPressed
            val isNumLock = event.isNumLockOn
            val vk = mapAndroidKeyToVk(keyCode, isNumLock && isAlt)
            if (vk in 0..255) {
                val isDown = (event.action != KeyEvent.ACTION_UP)
                synchronized(virtualKeyBuffer) {
                    virtualKeyBuffer[vk] = if (isDown) 0x80.toByte() else 0x00.toByte()
                    if (vk == 0x5A) {
                        virtualKeyBuffer[0x0D] = if (isDown) 0x80.toByte() else 0x00.toByte()
                    } else if (vk == 0x58) {
                        virtualKeyBuffer[0x1B] = if (isDown) 0x80.toByte() else 0x00.toByte()
                    }
                }
                flushVirtualKeyBuffer()
            }

            if (!mBrokenLibraries) {
                try {
                    if (SDLActivity.handleKeyEvent(mSurface, keyCode, event, null)) {
                        return true
                    }
                    if (event.action == KeyEvent.ACTION_DOWN) {
                        SDLActivity.onNativeKeyDown(keyCode)
                        return true
                    } else if (event.action == KeyEvent.ACTION_UP) {
                        SDLActivity.onNativeKeyUp(keyCode)
                        return true
                    }
                } catch (e: Throwable) {
                    // Native SDL not loaded (e.g. host unit tests); fall back to default dispatch.
                }
            }
        }
        // Overlay text is focused (or SDL did not consume the key): let the WebView/IME handle it.
        return super.dispatchKeyEvent(event)
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        val buttonState = event.buttonState
        val isDown = (event.actionMasked != MotionEvent.ACTION_UP && event.actionMasked != MotionEvent.ACTION_CANCEL)
        val lButton = isDown && ((buttonState and MotionEvent.BUTTON_PRIMARY) != 0 || (event.isFromSource(InputDevice.SOURCE_MOUSE) && event.actionMasked == MotionEvent.ACTION_DOWN))
        val rButton = isDown && ((buttonState and MotionEvent.BUTTON_SECONDARY) != 0)
        val mButton = isDown && ((buttonState and MotionEvent.BUTTON_TERTIARY) != 0)

        if (event.isFromSource(InputDevice.SOURCE_MOUSE) || event.isFromSource(InputDevice.SOURCE_TOUCHSCREEN)) {
            var changed = false
            synchronized(virtualKeyBuffer) {
                val oldL = virtualKeyBuffer[0x01]
                val oldR = virtualKeyBuffer[0x02]
                val oldM = virtualKeyBuffer[0x04]
                val newL = if (lButton) 0x80.toByte() else 0x00.toByte()
                val newR = if (rButton) 0x80.toByte() else 0x00.toByte()
                val newM = if (mButton) 0x80.toByte() else 0x00.toByte()
                if (oldL != newL || oldR != newR || oldM != newM) {
                    virtualKeyBuffer[0x01] = newL
                    virtualKeyBuffer[0x02] = newR
                    virtualKeyBuffer[0x04] = newM
                    changed = true
                }
            }
            if (changed) {
                flushVirtualKeyBuffer()
            }
        }

        // When gamepad is hidden and no overlay modal is open:
        // Forward touch/mouse events directly to mSurface so the user can interact directly!
        if (isGamepadHidden && !isOverlayModalOpen) {
            val density = resources.displayMetrics.density
            val toolbarMaxX = 70f * density
            val toolbarMaxY = 260f * density
            val isInsideToolbar = (event.x <= toolbarMaxX && event.y <= toolbarMaxY)
            if (!isInsideToolbar) {
                try {
                    mSurface?.dispatchTouchEvent(event)
                    return true
                } catch (ignored: Exception) {
                }
            }
        }

        return super.dispatchTouchEvent(event)
    }

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.isFromSource(InputDevice.SOURCE_CLASS_POINTER) || event.isFromSource(InputDevice.SOURCE_MOUSE)) {
            try {
                if (mSurface != null && mSurface.dispatchGenericMotionEvent(event)) {
                    return true
                }
            } catch (ignored: Exception) {
            }
        }
        return super.dispatchGenericMotionEvent(event)
    }

    private fun updateDebugHud(msg: String) {
        Log.i(TAG, "[HUD] $msg")
        runOnUiThread {
            val escaped = msg.replace("'", "\\'").replace("\n", " ")
            overlayWebView?.evaluateJavascript("window.updateDebugStatus && window.updateDebugStatus('$escaped');", null)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Set transparent window background so SurfaceView beneath is visible
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))

        val gameDir = File(filesDir, "game")
        val alreadyExtracted = GpackExtractor.isExtracted(gameDir)

        if (!alreadyExtracted) {
            Log.i(TAG, "Extracting game assets to internal storage...")
            val success = GpackExtractor.extract(this, gameDir)
            if (!success) {
                Log.e(TAG, "Extraction failed!")
            }
        }
        GpackExtractor.installRuntimeFiles(this, gameDir)

        GAME_PATH = gameDir.absolutePath
        Log.i(TAG, "Initialized RGSS GAME_PATH: $GAME_PATH")

        // Initialize SDLActivity and OpenGL ES Native Surface
        super.onCreate(savedInstanceState)

        // Make mLayout transparent
        mLayout?.setBackgroundColor(Color.TRANSPARENT)

        // Attach In-Game Virtual Gamepad (vpad.js) & Cheat Overlay on top of SDL Surface
        attachVirtualGamepadOverlay()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun attachVirtualGamepadOverlay() {
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

            val settings = wv.settings
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = true
            settings.allowContentAccess = true

            // VPad Bridge
            wv.addJavascriptInterface(object : Any() {
                @JavascriptInterface
                fun onKeyDown(jsKeyCode: Int) {
                    this@RgssActivity.onKeyDown(jsKeyCode)
                }

                @JavascriptInterface
                fun onKeyUp(jsKeyCode: Int) {
                    this@RgssActivity.onKeyUp(jsKeyCode)
                }

                @JavascriptInterface
                fun openUrl(url: String) {
                    this@RgssActivity.openUrl(url)
                }

                @JavascriptInterface
                fun postCheatCmd(jsonCmd: String): Boolean {
                    return this@RgssActivity.postCheatCmd(jsonCmd)
                }

                @JavascriptInterface
                fun getCheatState(): String {
                    return this@RgssActivity.getCheatState()
                }

                @JavascriptInterface
                fun getEvalResult(): String {
                    return this@RgssActivity.getEvalResult()
                }

                @JavascriptInterface
                fun setOverlayTextFocused(focused: Boolean) {
                    this@RgssActivity.overlayTextFocused = focused
                }

                @JavascriptInterface
                fun setGamepadHidden(hidden: Boolean) {
                    this@RgssActivity.isGamepadHidden = hidden
                    if (hidden) {
                        runOnUiThread { mSurface?.requestFocus() }
                    }
                }

                @JavascriptInterface
                fun setOverlayModalOpen(open: Boolean) {
                    this@RgssActivity.isOverlayModalOpen = open
                    if (!open) {
                        runOnUiThread { mSurface?.requestFocus() }
                    }
                }
            }, "RuntimePackVPad")

            wv.webViewClient = object : android.webkit.WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                }
            }

            wv.loadUrl("file:///android_asset/overlay/index.html")
            mLayout.addView(wv)
            overlayWebView = wv

            // Restore focus to SDL so physical keyboard input is dispatched to the game.
            mSurface?.requestFocus()

            Log.i(TAG, "Virtual Gamepad & Cheat Engine overlay attached successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to attach overlay: ${e.message}", e)
        }
    }

    protected open fun getInputStateDirectory(): File {
        return try {
            val game = File(filesDir, "game")
            if (game.exists() && game.isDirectory) game else (filesDir ?: File("."))
        } catch (e: Exception) {
            File(".")
        }
    }

    open fun postCheatCmd(jsonCmd: String): Boolean {
        return try {
            val dir = getInputStateDirectory()
            val targetFile = File(dir, CHEAT_CMD_FILE)
            val ok = writeAtomic(targetFile, jsonCmd)
            val gameSub = File(dir, "game")
            if (gameSub.exists() && gameSub.isDirectory) {
                writeAtomic(File(gameSub, CHEAT_CMD_FILE), jsonCmd)
            }
            ok
        } catch (e: Exception) {
            Log.e(TAG, "Failed to post cheat command: ${e.message}", e)
            false
        }
    }

    open fun getCheatState(): String {
        return try {
            val dir = getInputStateDirectory()
            val stateFile = File(dir, CHEAT_STATE_FILE)
            if (stateFile.exists()) {
                stateFile.readText(Charsets.UTF_8)
            } else {
                val gameSubFile = File(File(dir, "game"), CHEAT_STATE_FILE)
                if (gameSubFile.exists()) {
                    gameSubFile.readText(Charsets.UTF_8)
                } else {
                    "{}"
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read cheat state: ${e.message}", e)
            "{}"
        }
    }

    open fun getEvalResult(): String {
        return try {
            val dir = getInputStateDirectory()
            val evalFile = File(dir, CHEAT_EVAL_FILE)
            if (evalFile.exists()) {
                val text = evalFile.readText(Charsets.UTF_8)
                evalFile.delete()
                text
            } else {
                val gameSubFile = File(File(dir, "game"), CHEAT_EVAL_FILE)
                if (gameSubFile.exists()) {
                    val text = gameSubFile.readText(Charsets.UTF_8)
                    gameSubFile.delete()
                    text
                } else {
                    ""
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read eval result: ${e.message}", e)
            ""
        }
    }

    open fun onKeyDown(jsKeyCode: Int) {
        if (!pressedJsKeys.add(jsKeyCode)) {
            return
        }
        val (androidKeys, vkKeys) = mapJsKey(jsKeyCode)
        synchronized(virtualKeyBuffer) {
            for (vk in vkKeys) {
                if (vk in 0..255) {
                    virtualKeyBuffer[vk] = 0x80.toByte()
                }
            }
        }
        flushVirtualKeyBuffer()
        for (androidKey in androidKeys) {
            dispatchNativeKeyDown(androidKey)
        }
    }

    open fun onKeyUp(jsKeyCode: Int) {
        pressedJsKeys.remove(jsKeyCode)
        val (androidKeys, vkKeys) = mapJsKey(jsKeyCode)
        synchronized(virtualKeyBuffer) {
            for (vk in vkKeys) {
                if (vk in 0..255) {
                    virtualKeyBuffer[vk] = 0x00.toByte()
                }
            }
            for (pressedKey in pressedJsKeys) {
                val (_, remainingVks) = mapJsKey(pressedKey)
                for (vk in remainingVks) {
                    if (vk in 0..255) {
                        virtualKeyBuffer[vk] = 0x80.toByte()
                    }
                }
            }
        }
        flushVirtualKeyBuffer()
        for (androidKey in androidKeys) {
            dispatchNativeKeyUp(androidKey)
        }
    }

    protected open fun dispatchNativeKeyDown(androidKey: Int) {
        try {
            onNativeKeyDown(androidKey)
        } catch (e: UnsatisfiedLinkError) {
            // Ignored if native library not loaded (e.g. host unit tests)
        }
    }

    protected open fun dispatchNativeKeyUp(androidKey: Int) {
        try {
            onNativeKeyUp(androidKey)
        } catch (e: UnsatisfiedLinkError) {
            // Ignored if native library not loaded (e.g. host unit tests)
        }
    }

    open fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            startActivity(intent)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to open URL: $url", e)
        }
    }

    private fun mapJsToAndroidKey(jsKey: Int): Int {
        val (androidKeys, _) = mapJsKey(jsKey)
        return androidKeys.firstOrNull() ?: KeyEvent.KEYCODE_UNKNOWN
    }

    override fun onStart() {
        super.onStart()
        resumeNativeThread()
    }

    override fun onPause() {
        super.onPause()
        overlayTextFocused = false
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        overlayWebView?.onResume()
    }

    override fun onDestroy() {
        overlayTextFocused = false
        overlayWebView?.destroy()
        super.onDestroy()
        System.exit(0)
    }
}
