package io.runtimepack.host

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import java.io.File

/**
 * Launcher activity for Ren'Py games.
 *
 * The SDL runtime ([RenpyActivity]) must not start before the game content is
 * mounted, and mounting an ~850 MB `game.pack` must not run on the UI thread.
 * This activity owns that bootstrap: it shows a loading view, mounts content on
 * a background thread, then starts [RenpyActivity] (whose `onCreate` calls
 * through to super synchronously). On failure it shows an error dialog and
 * never starts the runtime, so the game cannot boot into a black screen.
 */
class RenpyBootstrapActivity : Activity() {

    companion object {
        private const val TAG = "RenpyBootstrap"

        /**
         * Extracts `game.pack` from APK assets into [root]. Returns true when
         * the Ren'Py engine layout is present afterwards. Safe to call off the
         * main thread.
         */
        @JvmStatic
        fun mountGameContent(context: Context, root: File): Boolean {
            root.mkdirs()
            val hasPack = try {
                context.assets.open(GpackExtractor.GAME_PACK_ASSET).use { true }
            } catch (_: Exception) {
                false
            }

            if (!hasPack) {
                Log.e(TAG, "${GpackExtractor.GAME_PACK_ASSET} not present in APK assets")
                return false
            }

            val ok = GpackExtractor.extract(context, root)
            if (!ok) {
                Log.e(TAG, "Failed to extract ${GpackExtractor.GAME_PACK_ASSET} into ${root.absolutePath}")
            }
            return ok
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Plain Activity: super.onCreate is synchronous and there is no SDL
        // surface to worry about yet.
        super.onCreate(savedInstanceState)

        val root = RenpyActivity.gameRoot(filesDir)
        if (RenpyActivity.isContentMounted(root)) {
            launchGame()
            return
        }

        showLoadingView()
        Thread({ mountInBackground(root) }, "RenpyMount").start()
    }

    private fun mountInBackground(root: File) {
        val mounted = try {
            mountGameContent(this, root)
        } catch (t: Throwable) {
            Log.e(TAG, "mountGameContent threw: ${t.message}", t)
            false
        }

        runOnUiThread {
            if (isFinishing || isDestroyed) {
                return@runOnUiThread
            }
            if (mounted && RenpyActivity.isContentMounted(root)) {
                Log.i(
                    TAG,
                    "Ren'Py content mounted at ${root.absolutePath}; rpa=" +
                        RenpyActivity.archivesIn(root).map { it.name }
                )
                launchGame()
            } else {
                showMountError(root, mounted)
            }
        }
    }

    private fun launchGame() {
        try {
            startActivity(Intent(this, RenpyActivity::class.java))
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start RenpyActivity: ${e.message}", e)
            showMountError(RenpyActivity.gameRoot(filesDir), extractionSucceeded = true)
            return
        }
        finish()
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
    }

    private fun showMountError(root: File, extractionSucceeded: Boolean) {
        val missing = RenpyActivity.missingLayoutEntries(root)
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
