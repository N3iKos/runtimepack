package io.runtimepack.host

import android.content.Context
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.io.OutputStream

interface SaveService {
    fun openRead(slot: String): InputStream?
    fun openWrite(slot: String): OutputStream
    fun exists(slot: String): Boolean
    fun list(): List<String>
    fun delete(slot: String): Boolean
}

class InternalStorageSaveService(context: Context) : SaveService {
    private val saveDir: File = File(context.filesDir, "saves").apply {
        if (!exists()) {
            mkdirs()
        }
    }

    private fun sanitizeSlot(slot: String): String {
        return slot.replace("/", "_").replace("\\", "_").replace("..", "_")
    }

    override fun openRead(slot: String): InputStream? {
        val file = File(saveDir, sanitizeSlot(slot))
        return if (file.exists() && file.isFile) FileInputStream(file) else null
    }

    override fun openWrite(slot: String): OutputStream {
        val file = File(saveDir, sanitizeSlot(slot))
        return FileOutputStream(file)
    }

    override fun exists(slot: String): Boolean {
        val file = File(saveDir, sanitizeSlot(slot))
        return file.exists() && file.isFile
    }

    override fun list(): List<String> {
        return saveDir.listFiles()?.filter { it.isFile }?.map { it.name } ?: emptyList()
    }

    override fun delete(slot: String): Boolean {
        val file = File(saveDir, sanitizeSlot(slot))
        return if (file.exists()) file.delete() else false
    }
}
