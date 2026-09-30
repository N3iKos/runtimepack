package io.runtimepack.host

import android.app.Activity
import android.content.Context
import android.util.Log
import java.security.MessageDigest

sealed class IntegrityResult {
    data class Ok(val hash: String) : IntegrityResult()
    data class Mismatch(val expected: String, val actual: String) : IntegrityResult()
    data class Unreadable(val code: String, val detail: String) : IntegrityResult()
}

object RuntimeIntegrity {
    private const val TAG = "RuntimeIntegrity"

    fun computeHash(pathsAndBytes: List<Pair<String, ByteArray>>): String {
        val sorted = pathsAndBytes.sortedBy { it.first.replace('\\', '/').trimStart('/').lowercase() }
        val md = MessageDigest.getInstance("SHA-256")
        for ((path, bytes) in sorted) {
            val normPath = path.replace('\\', '/').trimStart('/').lowercase()
            md.update(normPath.toByteArray(Charsets.UTF_8))
            md.update(0.toByte())
            md.update(bytes)
            md.update(0.toByte())
        }
        return md.digest().joinToString("") { "%02x".format(it) }
    }

    fun verify(
        context: Context? = null,
        expectedHash: String = RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256,
        buildFlavor: String = RuntimeBuildInfo.BUILD_FLAVOR
    ): IntegrityResult {
        val expected = expectedHash
        val flavor = buildFlavor

        if (context != null && OverlayAssetSource.gpackReader == null) {
            OverlayAssetSource.ensureInitialized(context)
        }

        val files = mutableListOf<Pair<String, ByteArray>>()
        val reader = OverlayAssetSource.gpackReader

        if (reader != null) {
            for (key in reader.fileIndexMap.keys) {
                if (key.startsWith("overlay/") || key.startsWith("web_shims/") || key.startsWith("fonts/")) {
                    val bytes = OverlayAssetSource.read(key, context)
                    if (bytes != null) {
                        files.add(key to bytes)
                    }
                }
            }
        } else if (context != null) {
            val subdirs = listOf("overlay", "web_shims", "fonts")
            fun collectAssets(prefix: String) {
                try {
                    val list = context.assets.list(prefix) ?: return
                    for (item in list) {
                        val path = if (prefix.isEmpty()) item else "$prefix/$item"
                        val sublist = context.assets.list(path)
                        if (!sublist.isNullOrEmpty()) {
                            collectAssets(path)
                        } else {
                            val bytes = OverlayAssetSource.read(path, context)
                            if (bytes != null) {
                                files.add(path to bytes)
                            }
                        }
                    }
                } catch (_: Exception) {}
            }
            for (sub in subdirs) {
                collectAssets(sub)
            }
        }

        if (files.isEmpty() && flavor.equals("release", ignoreCase = true)) {
            if (reader == null) {
                return IntegrityResult.Unreadable("AND-403", "GPACK container could not be opened")
            }
            return IntegrityResult.Unreadable("AND-402", "No runtime overlay assets found in container")
        }

        val actual = computeHash(files)

        if (!flavor.equals("release", ignoreCase = true)) {
            Log.i(TAG, "[RuntimePack] (Beta) Runtime integrity: $actual (expected: $expected)")
            return IntegrityResult.Ok(actual)
        }

        if (expected.isEmpty()) {
            Log.e(TAG, "[RuntimePack] AND-403 Expected integrity hash is missing in release build")
            return IntegrityResult.Unreadable("AND-403", "Expected integrity hash is missing in release build")
        }

        if (!actual.equals(expected, ignoreCase = true)) {
            Log.e(TAG, "[RuntimePack] AND-401 Runtime integrity mismatch: expected $expected, got $actual")
            return IntegrityResult.Mismatch(expected, actual)
        }

        return IntegrityResult.Ok(actual)
    }

    fun checkOrShowCorrupted(activity: Activity): Boolean {
        val result = verify(activity)
        return when (result) {
            is IntegrityResult.Ok -> true
            is IntegrityResult.Mismatch, is IntegrityResult.Unreadable -> {
                val code = if (result is IntegrityResult.Unreadable) result.code else "AND-401"
                val detail = if (result is IntegrityResult.Mismatch) "Hash mismatch" else (result as IntegrityResult.Unreadable).detail
                showCorruptedScreen(activity, code, detail)
                false
            }
        }
    }

    private fun showCorruptedScreen(activity: Activity, code: String, detail: String) {
        activity.runOnUiThread {
            try {
                val pkg = activity.packageName
                val layoutResId = activity.resources.getIdentifier("activity_corrupted", "layout", pkg)
                if (layoutResId != 0) {
                    activity.setContentView(layoutResId)
                    val tvCodeId = activity.resources.getIdentifier("tv_error_code", "id", pkg)
                    val tvCode = if (tvCodeId != 0) activity.findViewById<android.widget.TextView>(tvCodeId) else null
                    tvCode?.text = "ERROR: $code ($detail)"
                    val btnExitId = activity.resources.getIdentifier("btn_exit", "id", pkg)
                    val btnExit = if (btnExitId != 0) activity.findViewById<android.widget.Button>(btnExitId) else null
                    btnExit?.setOnClickListener {
                        activity.finishAffinity()
                        kotlin.system.exitProcess(1)
                    }
                } else {
                    val layout = android.widget.LinearLayout(activity).apply {
                        orientation = android.widget.LinearLayout.VERTICAL
                        gravity = android.view.Gravity.CENTER
                        setBackgroundColor(0xFF121212.toInt())
                        setPadding(48, 48, 48, 48)
                    }
                    val tvTitle = android.widget.TextView(activity).apply {
                        text = "APPLICATION CORRUPTED"
                        setTextColor(0xFFFF5252.toInt())
                        textSize = 22f
                        setTypeface(null, android.graphics.Typeface.BOLD)
                    }
                    val tvCode = android.widget.TextView(activity).apply {
                        text = "ERROR: $code ($detail)"
                        setTextColor(0xFFE0E0E0.toInt())
                        textSize = 14f
                        setPadding(0, 32, 0, 32)
                    }
                    val btnExit = android.widget.Button(activity).apply {
                        text = "Exit"
                        setOnClickListener {
                            activity.finishAffinity()
                            kotlin.system.exitProcess(1)
                        }
                    }
                    layout.addView(tvTitle)
                    layout.addView(tvCode)
                    layout.addView(btnExit)
                    activity.setContentView(layout)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to inflate activity_corrupted: ${e.message}", e)
                activity.finishAffinity()
                kotlin.system.exitProcess(1)
            }
        }
    }
}
