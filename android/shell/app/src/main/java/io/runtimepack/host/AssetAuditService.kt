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
        cachedReport = null
        val nextSeq = sequenceIdGenerator.incrementAndGet()
        val finalEvent = event.copy(sequence_id = nextSeq)

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
        cachedReport = null
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

    private fun resolveAuditDir(baseDir: File? = null): File {
        return when {
            baseDir != null -> {
                if (baseDir.name == "audit") baseDir
                else if (baseDir.name == "runtime-state") File(baseDir, "audit")
                else File(baseDir, "runtime-state/audit")
            }
            baseFilesDir != null -> File(baseFilesDir, "runtime-state/audit")
            else -> File("runtime-state/audit")
        }
    }

    fun persistReport(baseDir: File? = null): File {
        val auditDir = resolveAuditDir(baseDir)
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

    @Volatile
    private var activeRunner: AssetProbeRunner? = null

    private val probeExecutor: java.util.concurrent.ExecutorService = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "RuntimePack-AssetProbe").apply {
            isDaemon = true
            setUncaughtExceptionHandler { t, e ->
                android.util.Log.e("AssetAuditService", "Uncaught exception in probe thread ${t.name}: ${e.message}", e)
            }
        }
    }

    fun reset() {
        activeRunner?.cancel()
        activeRunner = null
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

        var requestedMode: ProbeMode? = null
        if (requestJson.contains("\"exhaustive\"", ignoreCase = true)) {
            requestedMode = ProbeMode.EXHAUSTIVE
        } else if (requestJson.contains("\"sampled\"", ignoreCase = true)) {
            requestedMode = ProbeMode.SAMPLED
        }

        if (requestedMode != null && baseFilesDir != null) {
            val gameSub = File(baseFilesDir, "game")
            val gameDir = if (gameSub.exists() && gameSub.isDirectory) gameSub else baseFilesDir!!
            val runner = AssetProbeRunner(
                gameDir = gameDir,
                stateDir = baseFilesDir,
                engine = currentEngine,
                mode = requestedMode
            )
            activeRunner = runner
            probeExecutor.submit {
                try {
                    runner.run()
                } catch (e: Throwable) {
                    android.util.Log.e("AssetAuditService", "Probe execution failed: ${e.message}", e)
                }
            }
        }

        return """{"status":"STARTED"}"""
    }

    fun getAuditEvents(afterSeq: Long): String {
        val events = getEvents(afterSeq)
        return "[" + events.joinToString(",") { it.toJson() } + "]"
    }

    fun getAuditReport(): String {
        return getReport()?.toJson() ?: "{}"
    }

    fun getCombinedSummary(baseDir: File? = null): String {
        val auditDir = resolveAuditDir(baseDir)
        val file = File(auditDir, "combined-validation-summary.json")
        return if (file.exists()) {
            file.readText(Charsets.UTF_8)
        } else {
            "{}"
        }
    }

    fun cancelAudit(): Boolean {
        activeRunner?.cancel()
        activeRunner = null
        recordEvent(
            stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
            assetPath = "root",
            outcome = RuntimeAuditOutcome.INDETERMINATE,
            errorMessage = "Audit cancelled by user"
        )
        return true
    }

    fun runProbe(
        mode: ProbeMode = ProbeMode.SAMPLED,
        gameDir: File? = null,
        stateDir: File? = null,
        sampleLimitPerCategory: Int = 5,
        checkpointInterval: Int = 100,
        timeoutMs: Long = 60000L
    ): RuntimeValidationReport {
        val gDir = gameDir ?: run {
            val gameSub = baseFilesDir?.let { File(it, "game") }
            if (gameSub != null && gameSub.exists() && gameSub.isDirectory) gameSub else (baseFilesDir ?: File("."))
        }
        val sDir = stateDir ?: baseFilesDir
        val runner = AssetProbeRunner(
            gameDir = gDir,
            stateDir = sDir,
            engine = currentEngine,
            mode = mode,
            sampleLimitPerCategory = sampleLimitPerCategory,
            checkpointInterval = checkpointInterval,
            timeoutMs = timeoutMs
        )
        activeRunner = runner
        return runner.run()
    }
}
