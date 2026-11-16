package io.runtimepack.host

import java.io.File
import java.io.FileOutputStream
import java.nio.file.Files
import java.nio.file.StandardCopyOption

class NativeSaveRepository(
    private val rootDir: File,
    initialAllowedNamespaces: Set<String> = emptySet()
) {

    init {
        if (!rootDir.exists()) {
            rootDir.mkdirs()
        }
    }

    private val allowedNamespaces = mutableSetOf("mz", "mv", "vnm").apply {
        addAll(initialAllowedNamespaces.map { it.lowercase() })
    }
    private val slotRegex = Regex("^[a-zA-Z0-9_.-]{1,128}$")
    private val namespaceRegex = Regex("^[a-zA-Z0-9_.-]{1,64}$")

    private fun isValidSlot(slot: String): Boolean {
        if (!slotRegex.matches(slot)) return false
        if (slot.startsWith(".")) return false
        if (slot.contains("..")) return false
        return true
    }

    private fun isValidNamespace(namespace: String): Boolean {
        if (!namespaceRegex.matches(namespace)) return false
        if (namespace.startsWith(".")) return false
        if (namespace.contains("..")) return false
        return true
    }

    @Synchronized
    fun registerNamespace(namespace: String): Boolean {
        val ns = namespace.lowercase()
        if (!isValidNamespace(ns)) return false
        allowedNamespaces.add(ns)
        return true
    }

    private fun resolveReadOnlyFile(namespace: String, slot: String): File? {
        val ns = namespace.lowercase()
        if (!isValidNamespace(ns)) return null
        if (!isValidSlot(slot)) return null
        val targetDir = File(rootDir, ns)
        if (!targetDir.exists() || !targetDir.isDirectory) return null
        return try {
            val rootCanonical = rootDir.canonicalFile
            val targetFile = File(targetDir, slot).canonicalFile
            if (!targetFile.toPath().startsWith(rootCanonical.toPath())) {
                null
            } else {
                targetFile
            }
        } catch (_: Exception) {
            null
        }
    }

    private fun resolveMutationFile(namespace: String, slot: String, createDir: Boolean): File? {
        val ns = namespace.lowercase()
        if (!allowedNamespaces.contains(ns)) return null
        if (!isValidNamespace(ns)) return null
        if (!isValidSlot(slot)) return null
        val targetDir = File(rootDir, ns)
        if (createDir && !targetDir.exists()) {
            targetDir.mkdirs()
        }
        return try {
            val rootCanonical = rootDir.canonicalFile
            val targetFile = File(targetDir, slot).canonicalFile
            if (!targetFile.toPath().startsWith(rootCanonical.toPath())) {
                null
            } else {
                targetFile
            }
        } catch (_: Exception) {
            null
        }
    }

    @Synchronized
    fun put(namespace: String, slot: String, payload: String): Boolean {
        val targetFile = resolveMutationFile(namespace, slot, createDir = true) ?: return false
        val tempFile = File(targetFile.parentFile, "${targetFile.name}.${System.nanoTime()}.tmp")
        return try {
            val bytes = payload.toByteArray(Charsets.UTF_8)
            FileOutputStream(tempFile).use { fos ->
                fos.write(bytes)
                fos.flush()
                fos.fd.sync()
            }
            Files.move(
                tempFile.toPath(),
                targetFile.toPath(),
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING
            )
            true
        } catch (_: Exception) {
            try {
                if (tempFile.exists()) tempFile.delete()
            } catch (_: Exception) {}
            false
        }
    }

    @Synchronized
    fun get(namespace: String, slot: String): String? {
        val targetFile = resolveReadOnlyFile(namespace, slot) ?: return null
        if (!targetFile.exists() || !targetFile.isFile) return null
        return try {
            targetFile.readText(Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

    @Synchronized
    fun contains(namespace: String, slot: String): Boolean {
        val targetFile = resolveReadOnlyFile(namespace, slot) ?: return false
        return targetFile.exists() && targetFile.isFile
    }

    @Synchronized
    fun remove(namespace: String, slot: String): Boolean {
        val targetFile = resolveMutationFile(namespace, slot, createDir = false) ?: return false
        return if (targetFile.exists()) targetFile.delete() else false
    }

    @Synchronized
    fun keys(namespace: String): List<String> {
        val ns = namespace.lowercase()
        if (!allowedNamespaces.contains(ns)) return emptyList()
        val targetDir = File(rootDir, ns)
        if (!targetDir.exists() || !targetDir.isDirectory) return emptyList()
        return targetDir.listFiles()
            ?.filter { it.isFile && !it.name.contains(".tmp") && isValidSlot(it.name) }
            ?.map { it.name }
            ?.sorted()
            ?: emptyList()
    }
}
