package io.runtimepack.host

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.VibrationEffect
import android.os.Vibrator
import android.util.Log
import android.view.KeyEvent
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.FrameLayout
import org.libsdl.app.SDLActivity
import java.io.File

open class RgssActivity : SDLActivity() {

    companion object {
        private const val TAG = "RgssActivity"

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
    }

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

    override fun onCreate(savedInstanceState: Bundle?) {
        // 1. Prepare Game Content in app-specific internal storage
        val gameDir = File(filesDir, "game")
        GpackExtractor.extractIfNecessary(this, gameDir)
        GAME_PATH = gameDir.absolutePath
        Log.i(TAG, "Initialized RGSS GAME_PATH: $GAME_PATH")

        // 2. Initialize SDLActivity and OpenGL ES Native Surface
        super.onCreate(savedInstanceState)

        // 3. Attach In-Game Virtual Gamepad (vpad.js) & Cheat Overlay on top of SDL Surface
        attachVirtualGamepadOverlay()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun attachVirtualGamepadOverlay() {
        if (mLayout == null) return

        try {
            val wv = WebView(this)
            wv.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            wv.setBackgroundColor(Color.TRANSPARENT)
            wv.setLayerType(android.view.View.LAYER_TYPE_HARDWARE, null)

            val settings = wv.settings
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = true
            settings.allowContentAccess = true

            // VPad Bridge
            wv.addJavascriptInterface(object : Any() {
                @JavascriptInterface
                fun onKeyDown(jsKeyCode: Int) {
                    val androidKey = mapJsToAndroidKey(jsKeyCode)
                    if (androidKey != KeyEvent.KEYCODE_UNKNOWN) {
                        onNativeKeyDown(androidKey)
                    }
                }

                @JavascriptInterface
                fun onKeyUp(jsKeyCode: Int) {
                    val androidKey = mapJsToAndroidKey(jsKeyCode)
                    if (androidKey != KeyEvent.KEYCODE_UNKNOWN) {
                        onNativeKeyUp(androidKey)
                    }
                }

                @JavascriptInterface
                fun openUrl(url: String) {
                    try {
                        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
                        startActivity(intent)
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to open URL: $url", e)
                    }
                }
            }, "RuntimePackVPad")

            wv.loadUrl("file:///android_asset/overlay/index.html")
            mLayout.addView(wv)
            overlayWebView = wv
            Log.i(TAG, "Virtual Gamepad & Cheat Engine overlay attached successfully.")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to attach overlay: ${e.message}", e)
        }
    }

    private fun mapJsToAndroidKey(jsKey: Int): Int {
        return when (jsKey) {
            38 -> KeyEvent.KEYCODE_DPAD_UP
            40 -> KeyEvent.KEYCODE_DPAD_DOWN
            37 -> KeyEvent.KEYCODE_DPAD_LEFT
            39 -> KeyEvent.KEYCODE_DPAD_RIGHT
            90 -> KeyEvent.KEYCODE_Z      // A (OK / Z)
            88 -> KeyEvent.KEYCODE_X      // B (Cancel / X)
            67 -> KeyEvent.KEYCODE_C      // C (Action 2)
            16 -> KeyEvent.KEYCODE_SHIFT_LEFT // DASH (Shift)
            27 -> KeyEvent.KEYCODE_ESCAPE // MENU (Escape)
            13 -> KeyEvent.KEYCODE_ENTER  // ENTER
            32 -> KeyEvent.KEYCODE_SPACE  // SPACE
            81 -> KeyEvent.KEYCODE_Q      // Q (L / Prev)
            69 -> KeyEvent.KEYCODE_E      // E (R / Next)
            17 -> KeyEvent.KEYCODE_CTRL_LEFT // SKIP (Ctrl)
            65 -> KeyEvent.KEYCODE_A
            83 -> KeyEvent.KEYCODE_S
            68 -> KeyEvent.KEYCODE_D
            87 -> KeyEvent.KEYCODE_W
            9  -> KeyEvent.KEYCODE_TAB
            8  -> KeyEvent.KEYCODE_DEL
            49 -> KeyEvent.KEYCODE_1
            50 -> KeyEvent.KEYCODE_2
            51 -> KeyEvent.KEYCODE_3
            52 -> KeyEvent.KEYCODE_4
            53 -> KeyEvent.KEYCODE_5
            54 -> KeyEvent.KEYCODE_6
            55 -> KeyEvent.KEYCODE_7
            56 -> KeyEvent.KEYCODE_8
            57 -> KeyEvent.KEYCODE_9
            48 -> KeyEvent.KEYCODE_0
            116 -> KeyEvent.KEYCODE_F5
            120 -> KeyEvent.KEYCODE_F9
            123 -> KeyEvent.KEYCODE_F12
            else -> KeyEvent.KEYCODE_UNKNOWN
        }
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
