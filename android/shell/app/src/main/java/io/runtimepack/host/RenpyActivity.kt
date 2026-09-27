package io.runtimepack.host

import android.os.Bundle
import android.system.Os
import android.util.Log
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
        const val GAME_PACK_ASSET = "game.pack"
        const val EXTRACTION_MARKER = ".gpack_extracted"

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
        fun extractionMarker(root: File): File = File(root, EXTRACTION_MARKER)

        @JvmStatic
        fun expectedLayout(): List<String> =
            listOf(MAIN_SCRIPT_NAME, ENGINE_DIR_NAME, PYTHON_LIB_DIR_NAME)

        @JvmStatic
        fun missingLayoutEntries(root: File): List<String> =
            expectedLayout().filterNot { File(root, it).exists() }

        @JvmStatic
        fun isGameMounted(root: File): Boolean = missingLayoutEntries(root).isEmpty()

        @JvmStatic
        fun archivesIn(dir: File): List<File> =
            dir.listFiles { f -> f.isFile && f.name.endsWith(".rpa", ignoreCase = true) }
                ?.sortedBy { it.name } ?: emptyList()

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
        val root = gameRoot(filesDir)
        if (!extractionMarker(root).exists()) {
            mountGameContent(root)
        }
        super.onCreate(savedInstanceState)
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

    private fun mountGameContent(root: File) {
        try {
            root.mkdirs()
            val hasPack = try {
                assets.open(GAME_PACK_ASSET).use { true }
            } catch (_: Exception) {
                false
            }

            if (hasPack) {
                if (!GpackExtractor.extract(this, root)) {
                    Log.e(TAG, "Failed to extract $GAME_PACK_ASSET into ${root.absolutePath}")
                }
            } else {
                Log.w(TAG, "$GAME_PACK_ASSET not present; expecting pre-mounted content at ${root.absolutePath}")
            }
        } catch (e: Exception) {
            Log.e(TAG, "mountGameContent failed: ${e.message}", e)
        }
    }

    private fun linkSaveDirectory(root: File, saves: File) {
        val legacySaves = File(root, SAVES_DIR_NAME)
        if (legacySaves.exists()) {
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
}
