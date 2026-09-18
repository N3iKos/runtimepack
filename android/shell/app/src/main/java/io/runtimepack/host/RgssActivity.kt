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
import android.view.KeyEvent
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
                    listOf(KeyEvent.KEYCODE_Z, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_SPACE),
                    listOf(0x5A, 0x0D, 0x20)
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
                27 -> Pair(listOf(KeyEvent.KEYCODE_ESCAPE), listOf(0x1B))
                13 -> Pair(listOf(KeyEvent.KEYCODE_ENTER), listOf(0x0D))
                32 -> Pair(listOf(KeyEvent.KEYCODE_SPACE), listOf(0x20))
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
                9 -> Pair(listOf(KeyEvent.KEYCODE_TAB), listOf(0x09))
                8 -> Pair(listOf(KeyEvent.KEYCODE_DEL), listOf(0x08))
                116 -> Pair(listOf(KeyEvent.KEYCODE_F5), listOf(0x74))
                120 -> Pair(listOf(KeyEvent.KEYCODE_F9), listOf(0x78))
                123 -> Pair(listOf(KeyEvent.KEYCODE_F12), listOf(0x7B))
                else -> Pair(emptyList(), emptyList())
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
    }

    val virtualKeyBuffer = ByteArray(256)
    val pressedJsKeys: MutableSet<Int> = ConcurrentHashMap.newKeySet()
    private var overlayWebView: WebView? = null

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
            }, "RuntimePackVPad")

            wv.webViewClient = object : android.webkit.WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                }
            }

            wv.loadUrl("file:///android_asset/overlay/index.html")
            mLayout.addView(wv)
            overlayWebView = wv
            Log.i(TAG, "Virtual Gamepad & Cheat Engine overlay attached successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to attach overlay: ${e.message}", e)
        }
    }

    protected open fun getInputStateDirectory(): File {
        return try {
            filesDir ?: File(".")
        } catch (e: Exception) {
            File(".")
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
            writeVirtualKeyBufferAtomic(virtualKeyBuffer, getInputStateDirectory())
        }
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
            writeVirtualKeyBufferAtomic(virtualKeyBuffer, getInputStateDirectory())
        }
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
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        overlayWebView?.onResume()
    }

    override fun onDestroy() {
        overlayWebView?.destroy()
        super.onDestroy()
        System.exit(0)
    }
}
