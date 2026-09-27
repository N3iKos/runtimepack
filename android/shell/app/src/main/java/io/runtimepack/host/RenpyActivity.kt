package io.runtimepack.host

import android.app.AlertDialog
import android.graphics.Color
import android.os.Bundle
import android.system.Os
import android.util.Log
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import org.libsdl.app.SDLActivity
import java.io.File

open class RenpyActivity : SDLActivity() {

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

    @Volatile
    private var sdlReady = false

    @Volatile
    private var activityResumed = false

    private var pendingState: Bundle? = null
    private var loadingView: View? = null

    override fun getLibraries(): Array<String> = arrayOf(RENPY_LIBRARY)

    override fun onCreate(savedInstanceState: Bundle?) {
        val root = gameRoot(filesDir)
        if (isContentMounted(root)) {
            startSdl(savedInstanceState)
        } else {
            pendingState = savedInstanceState
            // Neutralize SDL lifecycle callbacks until the surface exists.
            SDLActivity.mBrokenLibraries = true
            showLoadingView()
            Thread({ mountInBackground(root) }, "RenpyMount").start()
        }
    }

    private fun mountInBackground(root: File) {
        val extracted = try {
            mountGameContent(root)
        } catch (t: Throwable) {
            Log.e(TAG, "mountGameContent threw: ${t.message}", t)
            false
        }

        runOnUiThread {
            if (isFinishing || isDestroyed) {
                return@runOnUiThread
            }
            if (extracted && isContentMounted(root)) {
                logMountedContent(root)
                startSdl(pendingState)
            } else {
                showMountError(root, extracted)
            }
        }
    }

    private fun startSdl(savedInstanceState: Bundle?) {
        loadingView = null
        sdlReady = true
        super.onCreate(savedInstanceState)
        // The framework already delivered onResume while content was mounting,
        // but SDL's own resume was suppressed. Replay it so the SDL thread starts.
        if (activityResumed) {
            resumeNativeThread()
        }
    }

    override fun onResume() {
        super.onResume()
        activityResumed = true
    }

    override fun onPause() {
        super.onPause()
        activityResumed = false
    }

    override fun onBackPressed() {
        if (!sdlReady) {
            finish()
            return
        }
        super.onBackPressed()
    }

    fun preparePython() {
        val root = gameRoot(filesDir)
        root.mkdirs()

        val saves = saveDir(filesDir)
        saves.mkdirs()

        legacyPublicDir(filesDir).mkdirs()
        linkSaveDirectory(root, saves)

        runtimeEnv(filesDir, applicationInfo.sourceDir).forEach { (name, value) ->
            setEnv(name, value)
        }

        Log.i(TAG, "preparePython complete: ANDROID_PRIVATE=${root.absolutePath}")
    }

    private fun mountGameContent(root: File): Boolean {
        root.mkdirs()
        val hasPack = try {
            assets.open(GpackExtractor.GAME_PACK_ASSET).use { true }
        } catch (_: Exception) {
            false
        }

        if (!hasPack) {
            Log.e(TAG, "${GpackExtractor.GAME_PACK_ASSET} not present in APK assets")
            return false
        }

        val ok = GpackExtractor.extract(this, root)
        if (!ok) {
            Log.e(TAG, "Failed to extract ${GpackExtractor.GAME_PACK_ASSET} into ${root.absolutePath}")
        }
        return ok
    }

    private fun logMountedContent(root: File) {
        Log.i(TAG, "Ren'Py content mounted at ${root.absolutePath}; rpa=${archivesIn(root).map { it.name }}")
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

    private fun showLoadingView() {
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setBackgroundColor(Color.BLACK)
            setPadding(64, 64, 64, 64)
        }
        layout.addView(ProgressBar(this).apply { isIndeterminate = true })
        layout.addView(TextView(this).apply {
            text = "Preparing game content\u2026"
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
        })
        setContentView(layout)
        loadingView = layout
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
