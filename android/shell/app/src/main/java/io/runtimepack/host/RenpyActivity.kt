package io.runtimepack.host

import android.app.AlertDialog
import android.os.Bundle
import android.system.Os
import android.util.Log
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

    companion object {
        private const val TAG = "RenpyActivity"

        const val RENPY_LIBRARY = "renpython"
        const val GAME_DIR_NAME = "game"
        const val SAVES_DIR_NAME = "saves"
        const val LEGACY_PUBLIC_DIR_NAME = "legacy_public"
        const val MAIN_SCRIPT_NAME = "main.py"
        const val ENGINE_DIR_NAME = "renpy"
        const val PYTHON_LIB_DIR_NAME = "lib"

        const val ENV_ANDROID_PRIVATE = "ANDROID_PRIVATE"
        const val ENV_ANDROID_PUBLIC = "ANDROID_PUBLIC"
        const val ENV_ANDROID_OLD_PUBLIC = "ANDROID_OLD_PUBLIC"
        const val ENV_ANDROID_APK = "ANDROID_APK"

        @JvmStatic
        fun gameRoot(filesDir: File): File = File(filesDir, GAME_DIR_NAME)

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
            ENV_ANDROID_PUBLIC to filesDir.absolutePath,
            ENV_ANDROID_OLD_PUBLIC to legacyPublicDir(filesDir).absolutePath,
            ENV_ANDROID_APK to apkPath
        )
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
    }

    override fun onStart() {
        super.onStart()
        // SDL only starts its native thread from onStart on modern Android
        // (see RgssActivity); guard it so an unmounted game never boots.
        if (isContentMounted(gameRoot(filesDir))) {
            resumeNativeThread()
        }
    }

    fun preparePython() {
        PythonSDLActivity.mActivity = this

        val root = gameRoot(filesDir)
        root.mkdirs()

        val saves = saveDir(filesDir)
        saves.mkdirs()

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
