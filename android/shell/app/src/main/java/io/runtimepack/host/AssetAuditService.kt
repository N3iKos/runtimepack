package io.runtimepack.host

import java.io.File
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

object AssetAuditService {
    const val MAX_EVENTS = 5000

    private val eventQueue = ConcurrentLinkedQueue<RuntimeAuditEvent>()
    private val queueSize = AtomicInteger(0)
    private val sequenceIdGenerator = AtomicLong(0L)

    @Volatile
    private var baseFilesDir: File? = null

    @Volatile
    private var cachedReport: RuntimeAuditReport? = null

    @Volatile
    private var currentEngine: String = "unknown"

    fun init(filesDir: File, engine: String = "unknown") {
        baseFilesDir = filesDir
        currentEngine = engine
    }

    fun setEngine(engine: String) {
        currentEngine = engine
    }

    fun recordEvent(event: RuntimeAuditEvent): RuntimeAuditEvent {
        val finalEvent = if (event.sequence_id <= 0L) {
            val nextSeq = sequenceIdGenerator.incrementAndGet()
            event.copy(sequence_id = nextSeq)
        } else {
            sequenceIdGenerator.updateAndGet { current -> maxOf(current, event.sequence_id) }
            event
        }

        eventQueue.add(finalEvent)
        if (queueSize.incrementAndGet() > MAX_EVENTS) {
            if (eventQueue.poll() != null) {
                queueSize.decrementAndGet()
            }
        }
        return finalEvent
    }

    fun recordEvent(
        stage: RuntimeAuditStage,
        assetPath: String,
        outcome: RuntimeAuditOutcome,
        fallbackAction: String? = null,
        errorMessage: String? = null,
        sourceLocation: String? = null
    ): RuntimeAuditEvent {
        val nextSeq = sequenceIdGenerator.incrementAndGet()
        val event = RuntimeAuditEvent(
            sequence_id = nextSeq,
            timestamp_ns = System.nanoTime(),
            stage = stage,
            asset_path = assetPath,
            outcome = outcome,
            fallback_action = fallbackAction,
            error_message = errorMessage,
            source_location = sourceLocation
        )
        eventQueue.add(event)
        if (queueSize.incrementAndGet() > MAX_EVENTS) {
            if (eventQueue.poll() != null) {
                queueSize.decrementAndGet()
            }
        }
        return event
    }

    fun getEvents(afterSeq: Long = 0L): List<RuntimeAuditEvent> {
        val snapshot = eventQueue.toList()
        return if (afterSeq <= 0L) {
            snapshot
        } else {
            snapshot.filter { it.sequence_id > afterSeq }
        }
    }

    fun getReport(): RuntimeAuditReport? {
        if (cachedReport != null) return cachedReport
        val events = eventQueue.toList()
        if (events.isEmpty()) return null
        return computeReport(events)
    }

    private fun computeReport(events: List<RuntimeAuditEvent>): RuntimeAuditReport {
        val outcome = when {
            events.isEmpty() -> RuntimeAuditOutcome.NOT_RUN
            events.any { it.outcome == RuntimeAuditOutcome.FAIL } -> RuntimeAuditOutcome.FAIL
            events.any { it.outcome == RuntimeAuditOutcome.INDETERMINATE } -> RuntimeAuditOutcome.INDETERMINATE
            events.any { it.outcome == RuntimeAuditOutcome.WARNING } -> RuntimeAuditOutcome.WARNING
            else -> RuntimeAuditOutcome.PASS
        }

        val stageCounts = mutableMapOf<String, Int>()
        for (ev in events) {
            stageCounts[ev.stage.name] = (stageCounts[ev.stage.name] ?: 0) + 1
        }

        val fallbacksDetected = events.mapNotNull { it.fallback_action }.distinct()

        return RuntimeAuditReport(
            audit_id = "runtime-audit-" + System.currentTimeMillis(),
            engine = currentEngine,
            outcome = outcome,
            total_events = events.size,
            stage_counts = stageCounts,
            fallbacks_detected = fallbacksDetected,
            events = events
        )
    }

    fun persistReport(baseDir: File? = null): File {
        val targetBase = baseDir ?: baseFilesDir ?: File("runtime-state")
        val auditDir = File(targetBase, "runtime-state/audit")
        if (!auditDir.exists()) {
            auditDir.mkdirs()
        }

        val report = getReport() ?: computeReport(eventQueue.toList())
        cachedReport = report

        val targetFile = File(auditDir, "runtime-audit-report.json")
        val tmpFile = File(auditDir, "runtime-audit-report.json.tmp")

        tmpFile.writeText(report.toJson(), Charsets.UTF_8)
        if (!tmpFile.renameTo(targetFile)) {
            targetFile.delete()
            if (!tmpFile.renameTo(targetFile)) {
                targetFile.writeText(tmpFile.readText(Charsets.UTF_8), Charsets.UTF_8)
                tmpFile.delete()
            }
        }
        return targetFile
    }

    fun reset() {
        eventQueue.clear()
        queueSize.set(0)
        sequenceIdGenerator.set(0L)
        cachedReport = null
    }

    fun startAudit(requestJson: String = ""): String {
        reset()
        val loc = if (requestJson.isNotBlank()) "audit_start:$requestJson" else "audit_start"
        recordEvent(
            stage = RuntimeAuditStage.REFERENCE_FOUND,
            assetPath = "root",
            outcome = RuntimeAuditOutcome.PASS,
            sourceLocation = loc
        )
        return """{"status":"STARTED"}"""
    }

    fun getAuditEvents(afterSeq: Long): String {
        val events = getEvents(afterSeq)
        return "[" + events.joinToString(",") { it.toJson() } + "]"
    }

    fun cancelAudit(): Boolean {
        recordEvent(
            stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
            assetPath = "root",
            outcome = RuntimeAuditOutcome.INDETERMINATE,
            errorMessage = "Audit cancelled by user"
        )
        return true
    }
}
