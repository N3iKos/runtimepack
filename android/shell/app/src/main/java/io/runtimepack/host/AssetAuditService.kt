package io.runtimepack.host

import java.io.File
import java.io.FileInputStream
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

enum class AuditSessionState {
    PROCESS_RECREATED_IDLE,
    RUN_ACTIVE,
    RUN_COMPLETED
}

object AssetAuditService {
    const val MAX_EVENTS = 5000

    val runLock = Any()

    private val eventQueue = ConcurrentLinkedQueue<RuntimeAuditEvent>()
    private val queueSize = AtomicInteger(0)
    private val sequenceIdGenerator = AtomicLong(0L)
    private val runGeneration = AtomicLong(0L)

    data class ProblemRecord(
        val sequence_id: Long,
        val stage: String,
        val asset_path: String,
        val outcome: String,
        val category: String? = null,
        val error_message: String? = null,
        val source_location: String? = null,
        val fallback_action: String? = null
    ) {
        fun toJson(): String {
            val sb = StringBuilder()
            sb.append("{")
            sb.append("\"sequence_id\":").append(sequence_id).append(",")
            sb.append("\"stage\":\"").append(escapeJson(stage)).append("\",")
            sb.append("\"asset_path\":\"").append(escapeJson(asset_path)).append("\",")
            sb.append("\"outcome\":\"").append(escapeJson(outcome)).append("\"")
            if (category != null) sb.append(",\"category\":\"").append(escapeJson(category)).append("\"") else sb.append(",\"category\":null")
            if (error_message != null) sb.append(",\"error_message\":\"").append(escapeJson(error_message)).append("\"") else sb.append(",\"error_message\":null")
            if (source_location != null) sb.append(",\"source_location\":\"").append(escapeJson(source_location)).append("\"") else sb.append(",\"source_location\":null")
            if (fallback_action != null) sb.append(",\"fallback_action\":\"").append(escapeJson(fallback_action)).append("\"") else sb.append(",\"fallback_action\":null")
            sb.append("}")
            return sb.toString()
        }
    }

    const val MAX_PROBLEM_RECORDS = 2000
    const val MAX_PROBLEM_BYTES = 4 * 1024 * 1024L

    private val currentRunProblemRecords = java.util.concurrent.ConcurrentLinkedDeque<ProblemRecord>()
    private val totalProblemsCount = AtomicInteger(0)
    private val omittedProblemsCount = AtomicInteger(0)
    private val totalProblemBytes = AtomicLong(0L)

    @Volatile
    var lastCompletedRunSnapshot: String? = null
        private set

    @Volatile
    var lastCompletedRunId: String? = null
        private set

    @Volatile
    var currentSourceHash: String? = null
        private set

    @Volatile
    var currentRunId: String = "run-0"
        private set

    @Volatile
    var sessionState: AuditSessionState = AuditSessionState.PROCESS_RECREATED_IDLE
        private set

    fun setSessionState(state: AuditSessionState) {
        synchronized(runLock) {
            sessionState = state
        }
    }

    fun setCurrentRunId(runId: String) {
        synchronized(runLock) {
            currentRunId = runId
        }
    }

    fun setSourceHash(hash: String?) {
        synchronized(runLock) {
            currentSourceHash = hash
        }
    }

    fun newRunId(): String {
        synchronized(runLock) {
            val gen = runGeneration.incrementAndGet()
            val id = "run-${System.currentTimeMillis()}-$gen"
            currentRunId = id
            return id
        }
    }

    @Volatile
    private var baseFilesDir: File? = null

    @Volatile
    private var cachedReport: RuntimeAuditReport? = null

    @Volatile
    var currentEngine: String = "unknown"
        private set

    @Volatile
    private var appContext: android.content.Context? = null

    fun init(
        filesDir: File,
        engine: String = "unknown",
        context: android.content.Context? = null,
        sourceHash: String? = null
    ) {
        synchronized(runLock) {
            baseFilesDir = filesDir
            currentEngine = engine
            currentSourceHash = sourceHash
            if (context != null) {
                appContext = context.applicationContext ?: context
            }
        }
    }

    fun setContext(context: android.content.Context) {
        synchronized(runLock) {
            appContext = context.applicationContext ?: context
        }
    }

    fun setEngine(engine: String) {
        synchronized(runLock) {
            currentEngine = engine
        }
    }

    fun recordEvent(event: RuntimeAuditEvent): RuntimeAuditEvent {
        synchronized(runLock) {
            if (event.audit_id != null && event.audit_id != currentRunId) {
                return event
            }
            cachedReport = null
            val nextSeq = sequenceIdGenerator.incrementAndGet()
            val finalEvent = if (event.audit_id == null) {
                event.copy(sequence_id = nextSeq, audit_id = currentRunId)
            } else {
                event.copy(sequence_id = nextSeq)
            }

            if (finalEvent.outcome != RuntimeAuditOutcome.PASS && finalEvent.asset_path != "root" && finalEvent.asset_path != "all") {
                val prob = ProblemRecord(
                    sequence_id = finalEvent.sequence_id,
                    stage = finalEvent.stage.name,
                    asset_path = finalEvent.asset_path,
                    outcome = finalEvent.outcome.name,
                    category = finalEvent.category,
                    error_message = finalEvent.error_message,
                    source_location = finalEvent.source_location,
                    fallback_action = finalEvent.fallback_action
                )
                currentRunProblemRecords.addLast(prob)
                totalProblemsCount.incrementAndGet()
                val sz = prob.toJson().toByteArray(Charsets.UTF_8).size.toLong()
                totalProblemBytes.addAndGet(sz)
                while (currentRunProblemRecords.size > MAX_PROBLEM_RECORDS || totalProblemBytes.get() > MAX_PROBLEM_BYTES) {
                    val removed = currentRunProblemRecords.pollFirst()
                    if (removed != null) {
                        val remSz = removed.toJson().toByteArray(Charsets.UTF_8).size.toLong()
                        totalProblemBytes.addAndGet(-remSz)
                        omittedProblemsCount.incrementAndGet()
                    } else {
                        break
                    }
                }
            }

            if (finalEvent.is_complete == true || finalEvent.source_location == "DONE") {
                sessionState = AuditSessionState.RUN_COMPLETED
                lastCompletedRunSnapshot = buildFailuresSnapshotJson(MAX_PROBLEM_RECORDS)
                lastCompletedRunId = currentRunId
            }

            eventQueue.add(finalEvent)
            if (queueSize.incrementAndGet() > MAX_EVENTS) {
                if (eventQueue.poll() != null) {
                    queueSize.decrementAndGet()
                }
            }
            return finalEvent
        }
    }

    fun recordEvent(
        stage: RuntimeAuditStage,
        assetPath: String,
        outcome: RuntimeAuditOutcome,
        fallbackAction: String? = null,
        errorMessage: String? = null,
        sourceLocation: String? = null
    ): RuntimeAuditEvent {
        return recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                timestamp_ns = System.nanoTime(),
                stage = stage,
                asset_path = assetPath,
                outcome = outcome,
                fallback_action = fallbackAction,
                error_message = errorMessage,
                source_location = sourceLocation,
                audit_id = currentRunId
            )
        )
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
            if (ev.outcome == RuntimeAuditOutcome.PASS && ev.asset_path != "root" && ev.asset_path != "all" && ev.stage != RuntimeAuditStage.RUNTIME_USE_SUCCESS) {
                stageCounts[ev.stage.name] = (stageCounts[ev.stage.name] ?: 0) + 1
            }
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
        val tmpFile = File(auditDir, "runtime-audit-report_${currentRunId}.json.tmp")

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

    fun markRunnerComplete(runner: AssetProbeRunner) {
        synchronized(runLock) {
            if (activeRunner === runner) {
                activeRunner = null
            }
        }
    }

    fun <T> withCurrentRun(auditId: String, isCancelled: () -> Boolean, action: () -> T): T? {
        synchronized(runLock) {
            if (auditId != currentRunId || isCancelled()) {
                return null
            }
            return action()
        }
    }

    fun reset() {
        synchronized(runLock) {
            activeRunner?.cancel()
            activeRunner = null
            newRunId()
            sessionState = AuditSessionState.RUN_ACTIVE
            eventQueue.clear()
            queueSize.set(0)
            sequenceIdGenerator.set(0L)
            cachedReport = null
            lastCompletedRunSnapshot = null
            lastCompletedRunId = null
            currentRunProblemRecords.clear()
            totalProblemsCount.set(0)
            omittedProblemsCount.set(0)
            totalProblemBytes.set(0L)
        }
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
                mode = requestedMode,
                context = appContext,
                gpackReader = WebActivity.currentInstance?.gpackReader,
                auditIdOverride = currentRunId,
                expectedSourceHash = currentSourceHash
            )
            synchronized(runLock) {
                activeRunner = runner
            }
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

    fun buildFailuresSnapshotJson(limit: Int = 2000): String {
        val safeLimit = limit.coerceIn(0, MAX_PROBLEM_RECORDS)
        val totalFailures = totalProblemsCount.get()
        val originalOmitted = omittedProblemsCount.get()
        val allItems = synchronized(currentRunProblemRecords) { currentRunProblemRecords.toList() }
        val snapshot = allItems.take(safeLimit)
        val limitOmitted = if (allItems.size > safeLimit) allItems.size - safeLimit else 0
        val totalOmitted = originalOmitted + limitOmitted
        val isTruncated = totalOmitted > 0

        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"total_failures\":").append(totalFailures).append(",")
        sb.append("\"retained_count\":").append(snapshot.size).append(",")
        sb.append("\"omitted_count\":").append(totalOmitted).append(",")
        sb.append("\"truncated\":").append(isTruncated).append(",")
        sb.append("\"audit_id\":\"").append(escapeJson(currentRunId)).append("\",")
        sb.append("\"timestamp\":\"").append(escapeJson(currentIsoTimestamp())).append("\",")
        sb.append("\"engine\":\"").append(escapeJson(currentEngine)).append("\",")
        if (currentSourceHash != null) {
            sb.append("\"source_hash\":\"").append(escapeJson(currentSourceHash!!)).append("\",")
        } else {
            sb.append("\"source_hash\":null,")
        }
        sb.append("\"items\":[")
        sb.append(snapshot.joinToString(",") { it.toJson() })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }

    fun getFailuresJson(limit: Int = 2000): String {
        synchronized(runLock) {
            val safeLimit = limit.coerceIn(0, MAX_PROBLEM_RECORDS)

            if (sessionState == AuditSessionState.RUN_ACTIVE) {
                return buildFailuresSnapshotJson(safeLimit)
            }

            if (sessionState == AuditSessionState.RUN_COMPLETED && lastCompletedRunSnapshot != null && lastCompletedRunId == currentRunId) {
                val root = MiniJson.parseObject(lastCompletedRunSnapshot!!)
                if (root != null) {
                    val totalFailures = root.getInt("total_failures") ?: 0
                    val originalOmitted = root.getInt("omitted_count") ?: 0
                    val itemsArr = root.getArray("items")
                    val allItems = itemsArr?.objects() ?: emptyList()
                    if (safeLimit < allItems.size) {
                        val trimmed = allItems.take(safeLimit)
                        val additionalOmitted = allItems.size - safeLimit
                        val sb = StringBuilder()
                        sb.append("{")
                        sb.append("\"total_failures\":").append(totalFailures).append(",")
                        sb.append("\"retained_count\":").append(trimmed.size).append(",")
                        sb.append("\"omitted_count\":").append(originalOmitted + additionalOmitted).append(",")
                        sb.append("\"truncated\":true,")
                        sb.append("\"audit_id\":\"").append(escapeJson(currentRunId)).append("\",")
                        sb.append("\"timestamp\":\"").append(escapeJson(root.getString("timestamp") ?: currentIsoTimestamp())).append("\",")
                        sb.append("\"engine\":\"").append(escapeJson(currentEngine)).append("\",")
                        if (currentSourceHash != null) {
                            sb.append("\"source_hash\":\"").append(escapeJson(currentSourceHash!!)).append("\",")
                        } else {
                            sb.append("\"source_hash\":null,")
                        }
                        sb.append("\"items\":[")
                        val itemsJson = trimmed.map { obj ->
                            val parts = obj.keys().mapNotNull { k ->
                                val v = obj.getString(k)
                                val num = obj.getInt(k)
                                if (v != null) "\"${escapeJson(k)}\":\"${escapeJson(v)}\""
                                else if (num != null) "\"${escapeJson(k)}\":$num"
                                else null
                            }
                            "{" + parts.joinToString(",") + "}"
                        }
                        sb.append(itemsJson.joinToString(","))
                        sb.append("]")
                        sb.append("}")
                        return sb.toString()
                    }
                    return lastCompletedRunSnapshot!!
                }
            }

            if (sessionState == AuditSessionState.PROCESS_RECREATED_IDLE && baseFilesDir != null) {
                val auditDir = resolveAuditDir(baseFilesDir)
                val snapshotFile = File(auditDir, "problem-snapshot.json")
                if (snapshotFile.exists()) {
                    if (currentSourceHash == null || currentEngine.isBlank() || currentEngine == "unknown") {
                        return unavailableSnapshotJson("Missing current engine or source hash proof")
                    }
                    val fileLen = snapshotFile.length()
                    if (fileLen > MAX_PROBLEM_BYTES) {
                        return unavailableSnapshotJson("Persisted snapshot file size ($fileLen bytes) exceeds 4 MiB budget")
                    }

                    val text = try {
                        val buf = ByteArray(fileLen.toInt())
                        var readTotal = 0
                        FileInputStream(snapshotFile).use { fis ->
                            while (readTotal < buf.size) {
                                val r = fis.read(buf, readTotal, buf.size - readTotal)
                                if (r <= 0) break
                                readTotal += r
                            }
                        }
                        String(buf, 0, readTotal, Charsets.UTF_8)
                    } catch (_: Exception) {
                        return unavailableSnapshotJson("Failed to read persisted snapshot stream")
                    }

                    val root = MiniJson.parseObject(text) ?: return unavailableSnapshotJson("Malformed persisted snapshot JSON")
                    val snapEngine = root.getString("engine")
                    val snapSource = root.getString("source_hash")
                    val schemaVer = root.getInt("schema_version") ?: 1

                    if (snapEngine != currentEngine || snapSource != currentSourceHash || schemaVer != 1) {
                        return unavailableSnapshotJson("Snapshot source ($snapSource) or engine ($snapEngine) mismatch with current source ($currentSourceHash) and engine ($currentEngine)")
                    }

                    val totalFailures = root.getInt("total_failures") ?: 0
                    val originalOmitted = root.getInt("omitted_count") ?: 0
                    val origAuditId = root.getString("audit_id") ?: "unknown"
                    val origTimestamp = root.getString("timestamp") ?: ""
                    val itemsArr = root.getArray("items")
                    val allItems = itemsArr?.objects() ?: emptyList()

                    val trimmed = allItems.take(safeLimit)
                    val additionalOmitted = if (allItems.size > safeLimit) allItems.size - safeLimit else 0
                    val isTruncated = (originalOmitted + additionalOmitted) > 0

                    val sb = StringBuilder()
                    sb.append("{")
                    sb.append("\"total_failures\":").append(totalFailures).append(",")
                    sb.append("\"retained_count\":").append(trimmed.size).append(",")
                    sb.append("\"omitted_count\":").append(originalOmitted + additionalOmitted).append(",")
                    sb.append("\"truncated\":").append(isTruncated).append(",")
                    sb.append("\"audit_id\":\"").append(escapeJson(origAuditId)).append("\",")
                    sb.append("\"timestamp\":\"").append(escapeJson(origTimestamp)).append("\",")
                    sb.append("\"engine\":\"").append(escapeJson(snapEngine ?: "")).append("\",")
                    sb.append("\"source_hash\":\"").append(escapeJson(snapSource ?: "")).append("\",")
                    sb.append("\"historical\":true,")
                    sb.append("\"items\":[")
                    val itemsJson = trimmed.map { obj ->
                        val parts = obj.keys().mapNotNull { k ->
                            val v = obj.getString(k)
                            val num = obj.getInt(k)
                            if (v != null) "\"${escapeJson(k)}\":\"${escapeJson(v)}\""
                            else if (num != null) "\"${escapeJson(k)}\":$num"
                            else null
                        }
                        "{" + parts.joinToString(",") + "}"
                    }
                    sb.append(itemsJson.joinToString(","))
                    sb.append("]")
                    sb.append("}")
                    return sb.toString()
                }
            }

            return buildFailuresSnapshotJson(safeLimit)
        }
    }

    private fun unavailableSnapshotJson(reason: String): String {
        return "{\"status\":\"UNAVAILABLE\",\"reason\":\"${escapeJson(reason)}\",\"total_failures\":0,\"retained_count\":0,\"omitted_count\":0,\"truncated\":false,\"items\":[]}"
    }

    fun persistProblemSnapshot(auditDir: File, auditId: String) {
        val snapshotJson = buildFailuresSnapshotJson(MAX_PROBLEM_RECORDS)
        lastCompletedRunSnapshot = snapshotJson
        lastCompletedRunId = auditId
        val target = File(auditDir, "problem-snapshot.json")
        val tmp = File(auditDir, "problem-snapshot_${auditId}.json.tmp")
        tmp.writeText(snapshotJson, Charsets.UTF_8)
        if (!tmp.renameTo(target)) {
            target.delete()
            tmp.renameTo(target)
        }
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
        synchronized(runLock) {
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
    }

    fun runProbe(
        mode: ProbeMode = ProbeMode.SAMPLED,
        gameDir: File? = null,
        stateDir: File? = null,
        sampleLimitPerCategory: Int = 5,
        checkpointInterval: Int = 100,
        timeoutMs: Long = 60000L,
        bitmapDecoder: BitmapProbeDecoder = DefaultBitmapProbeDecoder
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
            timeoutMs = timeoutMs,
            bitmapDecoder = bitmapDecoder,
            context = appContext,
            gpackReader = WebActivity.currentInstance?.gpackReader,
            auditIdOverride = currentRunId,
            expectedSourceHash = currentSourceHash
        )
        synchronized(runLock) {
            activeRunner = runner
        }
        return runner.run()
    }
}
