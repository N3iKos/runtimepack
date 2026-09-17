package io.runtimepack.host

import android.content.Context
import java.io.InputStream

interface ContentService {
    fun open(logicalPath: String): InputStream
    fun exists(logicalPath: String): Boolean
}

class AssetContentService(
    private val context: Context,
    private val contentMode: String
) : ContentService {

    private fun normalizePath(path: String): String {
        return path.replace('\\', '/').trimStart('/')
    }

    override fun open(logicalPath: String): InputStream {
        val normalized = normalizePath(logicalPath)
        return when (contentMode) {
            "plain" -> {
                context.assets.open("game/$normalized")
            }
            "packed", "encrypted-streaming" -> {
                // In packed mode, assets can be loaded from game.pack or asset provider
                context.assets.open("game.pack")
            }
            else -> context.assets.open(normalized)
        }
    }

    override fun exists(logicalPath: String): Boolean {
        return try {
            open(logicalPath).close()
            true
        } catch (_: Exception) {
            false
        }
    }
}
