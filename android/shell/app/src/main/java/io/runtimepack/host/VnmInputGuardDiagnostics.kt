package io.runtimepack.host

import android.util.Log
import java.io.File
import java.util.concurrent.ConcurrentLinkedDeque
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

object VnmInputGuardDiagnostics {
    private const val TAG = "VnmInputGuardDiagnostics"
    const val MAX_SNAPSHOT_RECORDS = 64
    const val MAX_SNAPSHOT_BYTES = 2048
    const val MAX_TOTAL_BYTES = 128 * 1024
    const val PERSIST_INTERVAL_MS = 5000L

    val ALLOWED_KEYS = setOf(
        "schema_version",
        "sequence",
        "elapsed_ms",
        "mode",
        "frame_count",
        "frame_delta",
        "graphics_paused",
        "graphics_stopped",
        "graphics_frozen",
        "transition_present",
        "transition_alpha",
        "transition_duration",
        "left_button_state",
        "button_down",
        "button_up",
        "mouse_down_count",
        "mouse_up_count",
        "touch_cancel_count",
        "dialogue_callback_count",
        "blocked_dialogue_count",
        "delegated_dialogue_count",
        "choice_count",
        "video_present",
        "skip_active",
        "auto_active",
        "unknown_state_count"
    )

    private val BOOLEAN_KEYS = setOf(
        "graphics_paused",
        "graphics_stopped",
        "graphics_frozen",
        "transition_present",
        "left_button_state",
        "button_down",
        "button_up",
        "video_present",
        "skip_active",
        "auto_active"
    )

    private val NUMERIC_KEYS = setOf(
        "sequence",
        "elapsed_ms",
        "frame_count",
        "frame_delta",
        "transition_alpha",
        "transition_duration",
        "mouse_down_count",
        "mouse_up_count",
        "touch_cancel_count",
        "dialogue_callback_count",
        "blocked_dialogue_count",
        "delegated_dialogue_count",
        "choice_count",
        "unknown_state_count"
    )

    @Volatile
    var isEnabled: Boolean = false
        private set

    @Volatile
    var storageDir: File? = null
        private set

    private val snapshots = ConcurrentLinkedDeque<String>()
    private val totalRecorded = AtomicLong(0L)
    private val omittedCount = AtomicInteger(0)
    private val currentTotalBytes = AtomicLong(0L)

    private var persistenceExecutor: ScheduledExecutorService? = null
    private val persistScheduled = AtomicBoolean(false)
    @Volatile
    private var lastPersistTimeMs = 0L

    @Synchronized
    fun configure(isVnm: Boolean, guardMode: String, baseDir: File?) {
        val enabled = isVnm && guardMode == "transition-aware"
        this.isEnabled = enabled
        this.storageDir = baseDir
        if (enabled && persistenceExecutor == null) {
            persistenceExecutor = Executors.newSingleThreadScheduledExecutor { r ->
                Thread(r, "vnm-guard-diagnostics-persist").apply { isDaemon = true }
            }
        } else if (!enabled && persistenceExecutor != null) {
            persistenceExecutor?.shutdownNow()
            persistenceExecutor = null
        }
    }

    private fun serializeValue(v: MiniJsonValue): String = when (v) {
        is MiniJsonValue.JString -> "\"${escapeJson(v.value)}\""
        is MiniJsonValue.JNumber -> {
            if (v.value == v.value.toLong().toDouble()) {
                v.value.toLong().toString()
            } else {
                v.value.toString()
            }
        }
        is MiniJsonValue.JBool -> v.value.toString()
        is MiniJsonValue.JNull -> "null"
        is MiniJsonValue.JObject -> "{" + v.map.entries.joinToString(",") { "\"${escapeJson(it.key)}\":${serializeValue(it.value)}" } + "}"
        is MiniJsonValue.JArray -> "[" + v.items.joinToString(",") { serializeValue(it) } + "]"
    }

    fun postSnapshot(json: String): Boolean {
        if (!isEnabled) {
            return false
        }

        val utf8Bytes = json.toByteArray(Charsets.UTF_8)
        if (utf8Bytes.size > MAX_SNAPSHOT_BYTES) {
            return false
        }

        val root = try {
            MiniJson.parseObjectOrThrow(json)
        } catch (_: Exception) {
            return false
        }

        if (root.map.isEmpty()) {
            return false
        }

        for ((key, value) in root.map) {
            if (key !in ALLOWED_KEYS) {
                return false
            }
            when (key) {
                "schema_version" -> {
                    if (value !is MiniJsonValue.JNumber || value.value.toInt() != 1) {
                        return false
                    }
                }
                "mode" -> {
                    if (value !is MiniJsonValue.JString || value.value != "transition-aware") {
                        return false
                    }
                }
                in BOOLEAN_KEYS -> {
                    if (value !is MiniJsonValue.JBool && value !is MiniJsonValue.JNull) {
                        return false
                    }
                }
                in NUMERIC_KEYS -> {
                    if (value is MiniJsonValue.JNumber) {
                        if (value.value.isNaN() || value.value.isInfinite()) {
                            return false
                        }
                    } else if (value !is MiniJsonValue.JNull) {
                        return false
                    }
                }
                else -> return false
            }
        }

        val recordJson = serializeValue(root)
        val recordSize = recordJson.toByteArray(Charsets.UTF_8).size.toLong()

        synchronized(snapshots) {
            snapshots.addLast(recordJson)
            totalRecorded.incrementAndGet()
            currentTotalBytes.addAndGet(recordSize)

            while (snapshots.size > MAX_SNAPSHOT_RECORDS || currentTotalBytes.get() > MAX_TOTAL_BYTES) {
                val removed = snapshots.pollFirst()
                if (removed != null) {
                    val remSize = removed.toByteArray(Charsets.UTF_8).size.toLong()
                    currentTotalBytes.addAndGet(-remSize)
                    omittedCount.incrementAndGet()
                } else {
                    break
                }
            }
        }

        schedulePersist()
        return true
    }

    private fun schedulePersist() {
        val dir = storageDir ?: return
        val exec = persistenceExecutor ?: return

        val now = System.currentTimeMillis()
        val elapsed = now - lastPersistTimeMs
        if (elapsed >= PERSIST_INTERVAL_MS) {
            if (persistScheduled.compareAndSet(false, true)) {
                exec.execute {
                    persistLatest(dir)
                }
            }
        } else {
            val delay = PERSIST_INTERVAL_MS - elapsed
            if (persistScheduled.compareAndSet(false, true)) {
                exec.schedule({
                    persistLatest(dir)
                }, delay, TimeUnit.MILLISECONDS)
            }
        }
    }

    fun persistNow() {
        val dir = storageDir ?: return
        persistLatest(dir)
    }

    private fun persistLatest(dir: File) {
        try {
            persistScheduled.set(false)
            lastPersistTimeMs = System.currentTimeMillis()

            val content = getSnapshotsJson()
            if (!dir.exists()) {
                dir.mkdirs()
            }
            val targetFile = File(dir, "vnm_input_snapshots.json")
            val tmpFile = File(dir, "vnm_input_snapshots.json.tmp")
            tmpFile.writeText(content, Charsets.UTF_8)
            if (!tmpFile.renameTo(targetFile)) {
                targetFile.delete()
                tmpFile.renameTo(targetFile)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to persist VNM input snapshots: ${e.message}")
        }
    }

    fun getSnapshotsJson(): String {
        val list = synchronized(snapshots) { snapshots.toList() }
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"total_recorded\":").append(totalRecorded.get()).append(",")
        sb.append("\"retained_count\":").append(list.size).append(",")
        sb.append("\"omitted_count\":").append(omittedCount.get()).append(",")
        sb.append("\"items\":[")
        sb.append(list.joinToString(","))
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }

    fun getSnapshotCount(): Int = snapshots.size

    fun getTotalRecorded(): Long = totalRecorded.get()

    fun getOmittedCount(): Int = omittedCount.get()

    fun resetAll() {
        synchronized(snapshots) {
            snapshots.clear()
            totalRecorded.set(0L)
            omittedCount.set(0)
            currentTotalBytes.set(0L)
        }
        isEnabled = false
        storageDir = null
        persistenceExecutor?.shutdownNow()
        persistenceExecutor = null
        persistScheduled.set(false)
        lastPersistTimeMs = 0L
    }
}
