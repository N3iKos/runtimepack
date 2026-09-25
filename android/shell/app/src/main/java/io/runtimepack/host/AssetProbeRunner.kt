package io.runtimepack.host

import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import java.util.concurrent.atomic.AtomicBoolean

class AssetProbeRunner(
    val gameDir: File,
    val stateDir: File? = null,
    val engine: String = "unknown",
    val mode: ProbeMode = ProbeMode.SAMPLED,
    val sampleLimitPerCategory: Int = 5,
    val checkpointInterval: Int = 100,
    val timeoutMs: Long = 60000L,
    var bitmapDecoder: BitmapProbeDecoder = DefaultBitmapProbeDecoder,
    var onAssetProbed: ((Int) -> Unit)? = null
) {
    private val isCancelled = AtomicBoolean(false)

    fun cancel() {
        isCancelled.set(true)
    }

    fun isCancelled(): Boolean = isCancelled.get()

    fun run(): RuntimeValidationReport {
        val startTime = System.currentTimeMillis()
        val auditId = "runtime-probe-" + startTime
        val auditDir = when {
            stateDir != null -> File(stateDir, "audit")
            else -> File(gameDir, "runtime-state/audit")
        }
        if (!auditDir.exists()) {
            auditDir.mkdirs()
        }

        // 1. Discover assets
        val allAssets = discoverAssets(auditDir)

        // 2. Select assets based on mode
        val assetsToProbe = if (mode == ProbeMode.SAMPLED) {
            selectSampledAssets(allAssets, sampleLimitPerCategory)
        } else {
            allAssets
        }

        val checkpointFile = File(auditDir, "probe_checkpoint.json")
        var startIndex = 0
        var checkpointRestored = false
        var passedCount = 0
        var failedCount = 0
        var warningCount = 0
        val stageCounts = mutableMapOf<String, Int>()
        val fallbacksDetected = mutableListOf<String>()
        val recordedEvents = mutableListOf<RuntimeAuditEvent>()

        // 3. Resume from checkpoint if available in exhaustive mode
        if (mode == ProbeMode.EXHAUSTIVE && checkpointFile.exists()) {
            try {
                val cpText = checkpointFile.readText(Charsets.UTF_8)
                val checkpoint = ProbeCheckpoint.fromJson(cpText)
                if (checkpoint != null &&
                    checkpoint.mode == mode.name &&
                    checkpoint.total_assets == assetsToProbe.size &&
                    checkpoint.current_index in 1 until assetsToProbe.size
                ) {
                    startIndex = checkpoint.current_index
                    passedCount = checkpoint.passed_count
                    failedCount = checkpoint.failed_count
                    warningCount = checkpoint.warning_count
                    stageCounts.putAll(checkpoint.stage_counts)
                    fallbacksDetected.addAll(checkpoint.fallbacks_detected)
                    checkpointRestored = true
                }
            } catch (_: Exception) {}
        }

        // 4. Sequential Execution Loop with memory limits & checkpoints
        for (i in startIndex until assetsToProbe.size) {
            if (isCancelled.get()) {
                val cancelEv = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
                    asset_path = "root",
                    outcome = RuntimeAuditOutcome.INDETERMINATE,
                    error_message = "Probe cancelled by user"
                )
                AssetAuditService.recordEvent(cancelEv)
                recordedEvents.add(cancelEv)
                break
            }

            if (System.currentTimeMillis() - startTime > timeoutMs) {
                val timeoutEv = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
                    asset_path = "root",
                    outcome = RuntimeAuditOutcome.INDETERMINATE,
                    error_message = "Probe timed out after ${timeoutMs}ms"
                )
                AssetAuditService.recordEvent(timeoutEv)
                recordedEvents.add(timeoutEv)
                break
            }

            val relPath = assetsToProbe[i]
            val file = File(gameDir, relPath)

            // Stage 1: FILE_EXISTS
            if (!file.exists()) {
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    error_message = "File not found: $relPath"
                )
                AssetAuditService.recordEvent(ev)
                recordedEvents.add(ev)
                failedCount++
                stageCounts["FILE_EXISTS"] = (stageCounts["FILE_EXISTS"] ?: 0) + 1
                onAssetProbed?.invoke(i + 1)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected
                )
                continue
            }
            stageCounts["FILE_EXISTS"] = (stageCounts["FILE_EXISTS"] ?: 0) + 1

            // Stage 2: FILE_READABLE
            val fis: InputStream = try {
                val s = FileInputStream(file)
                stageCounts["FILE_READABLE"] = (stageCounts["FILE_READABLE"] ?: 0) + 1
                s
            } catch (e: Exception) {
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    error_message = "File unreadable: ${e.message}"
                )
                AssetAuditService.recordEvent(ev)
                recordedEvents.add(ev)
                failedCount++
                onAssetProbed?.invoke(i + 1)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected
                )
                continue
            }

            // Stage 3: INTEGRITY_VERIFIED
            if (file.length() == 0L) {
                try { fis.close() } catch (_: Exception) {}
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.INTEGRITY_VERIFIED,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    error_message = "File integrity error: zero length"
                )
                AssetAuditService.recordEvent(ev)
                recordedEvents.add(ev)
                failedCount++
                stageCounts["INTEGRITY_VERIFIED"] = (stageCounts["INTEGRITY_VERIFIED"] ?: 0) + 1
                onAssetProbed?.invoke(i + 1)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected
                )
                continue
            }
            stageCounts["INTEGRITY_VERIFIED"] = (stageCounts["INTEGRITY_VERIFIED"] ?: 0) + 1

            // Stage 4: DECODE_SUCCESS
            val decodeOk = try {
                if (isImageExtension(relPath)) {
                    bitmapDecoder.decodeAndVerify(fis)
                } else {
                    // For audio/font/video/data, verify that stream is readable
                    val buf = ByteArray(128)
                    val read = fis.read(buf)
                    read > 0
                }
            } catch (_: Exception) {
                false
            } finally {
                try { fis.close() } catch (_: Exception) {}
            }

            stageCounts["DECODE_SUCCESS"] = (stageCounts["DECODE_SUCCESS"] ?: 0) + 1
            if (decodeOk) {
                passedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.DECODE_SUCCESS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.PASS
                )
                AssetAuditService.recordEvent(ev)
                recordedEvents.add(ev)
            } else {
                failedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.DECODE_SUCCESS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    error_message = "Asset decode verification failed: $relPath"
                )
                AssetAuditService.recordEvent(ev)
                recordedEvents.add(ev)
            }

            onAssetProbed?.invoke(i + 1)
            saveCheckpointIfNeeded(
                checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                stageCounts, passedCount, failedCount, warningCount, fallbacksDetected
            )
        }

        val duration = System.currentTimeMillis() - startTime
        val finalOutcome = when {
            isCancelled.get() || duration > timeoutMs -> RuntimeAuditOutcome.INDETERMINATE
            failedCount > 0 -> RuntimeAuditOutcome.FAIL
            warningCount > 0 -> RuntimeAuditOutcome.WARNING
            else -> RuntimeAuditOutcome.PASS
        }

        // Emitting final completion event
        val doneEv = RuntimeAuditEvent(
            sequence_id = 0L,
            stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
            asset_path = "all",
            outcome = finalOutcome,
            error_message = if (finalOutcome == RuntimeAuditOutcome.FAIL) "Probe completed with $failedCount failure(s)" else "DONE: Probe completed",
            source_location = "DONE",
            is_complete = true
        )
        AssetAuditService.recordEvent(doneEv)
        recordedEvents.add(doneEv)

        // 5. Generate and persist RuntimeValidationReport
        val runtimeReport = RuntimeValidationReport(
            schema_version = 1,
            audit_id = auditId,
            engine = engine,
            mode = mode,
            outcome = finalOutcome,
            total_probed = assetsToProbe.size,
            passed_count = passedCount,
            failed_count = failedCount,
            warning_count = warningCount,
            stage_counts = stageCounts,
            fallbacks_detected = fallbacksDetected.distinct(),
            checkpoint_restored = checkpointRestored,
            duration_ms = duration,
            timestamp = currentIsoTimestamp(),
            events = recordedEvents
        )

        val runtimeReportFile = File(auditDir, "runtime-validation-report.json")
        val runtimeTmp = File(auditDir, "runtime-validation-report.json.tmp")
        runtimeTmp.writeText(runtimeReport.toJson(), Charsets.UTF_8)
        if (!runtimeTmp.renameTo(runtimeReportFile)) {
            runtimeReportFile.delete()
            runtimeTmp.renameTo(runtimeReportFile)
        }

        // 6. Generate and persist CombinedValidationSummary
        val combinedSummary = buildCombinedSummary(auditDir, runtimeReport)
        val combinedSummaryFile = File(auditDir, "combined-validation-summary.json")
        val combinedTmp = File(auditDir, "combined-validation-summary.json.tmp")
        combinedTmp.writeText(combinedSummary.toJson(), Charsets.UTF_8)
        if (!combinedTmp.renameTo(combinedSummaryFile)) {
            combinedSummaryFile.delete()
            combinedTmp.renameTo(combinedSummaryFile)
        }

        return runtimeReport
    }

    private fun saveCheckpointIfNeeded(
        checkpointFile: File,
        auditId: String,
        currentIndex: Int,
        totalAssets: Int,
        lastPath: String,
        stageCounts: Map<String, Int>,
        passedCount: Int,
        failedCount: Int,
        warningCount: Int,
        fallbacksDetected: List<String>
    ) {
        if (currentIndex % checkpointInterval == 0 || currentIndex == totalAssets) {
            try {
                val checkpoint = ProbeCheckpoint(
                    audit_id = auditId,
                    mode = mode.name,
                    current_index = currentIndex,
                    total_assets = totalAssets,
                    last_processed_path = lastPath,
                    stage_counts = HashMap(stageCounts),
                    passed_count = passedCount,
                    failed_count = failedCount,
                    warning_count = warningCount,
                    fallbacks_detected = ArrayList(fallbacksDetected),
                    timestamp_ms = System.currentTimeMillis()
                )
                val tmp = File(checkpointFile.parentFile, "probe_checkpoint.json.tmp")
                tmp.writeText(checkpoint.toJson(), Charsets.UTF_8)
                if (!tmp.renameTo(checkpointFile)) {
                    checkpointFile.delete()
                    tmp.renameTo(checkpointFile)
                }
            } catch (_: Exception) {}
        }
    }

    private fun discoverAssets(auditDir: File): List<String> {
        // Look for asset-dependency-graph.json or asset-audit-report.json
        val graphCandidates = listOf(
            File(gameDir, "asset-dependency-graph.json"),
            File(auditDir, "asset-dependency-graph.json"),
            File(gameDir, "asset-audit-report.json"),
            File(auditDir, "asset-audit-report.json")
        )
        for (candidate in graphCandidates) {
            if (candidate.exists()) {
                val parsed = parseLogicalPathsFromJson(candidate.readText(Charsets.UTF_8))
                if (parsed.isNotEmpty()) {
                    return parsed.distinct().sorted()
                }
            }
        }

        // Fallback: scan gameDir recursively
        val results = mutableListOf<String>()
        if (gameDir.exists() && gameDir.isDirectory) {
            gameDir.walkTopDown().forEach { file ->
                if (file.isFile) {
                    val rel = file.relativeTo(gameDir).path.replace('\\', '/')
                    if (!isExcludedPath(rel)) {
                        results.add(rel)
                    }
                }
            }
        }
        return results.distinct().sorted()
    }

    private fun parseLogicalPathsFromJson(jsonStr: String): List<String> {
        val paths = mutableListOf<String>()
        val pattern = Regex("\"logical_path\"\\s*:\\s*\"([^\"]+)\"")
        for (match in pattern.findAll(jsonStr)) {
            val p = match.groupValues[1].replace('\\', '/').trimStart('/')
            if (p.isNotBlank() && !isExcludedPath(p)) {
                paths.add(p)
            }
        }
        return paths
    }

    private fun isExcludedPath(path: String): Boolean {
        val lower = path.lowercase()
        return lower.startsWith(".git") ||
                lower.startsWith("runtime-state") ||
                lower.startsWith("save") ||
                lower.endsWith(".tmp") ||
                lower.endsWith(".bak") ||
                lower.endsWith(".ds_store")
    }

    private fun selectSampledAssets(allAssets: List<String>, limitPerCategory: Int): List<String> {
        val images = mutableListOf<String>()
        val audios = mutableListOf<String>()
        val fonts = mutableListOf<String>()
        val videos = mutableListOf<String>()
        val datas = mutableListOf<String>()
        val others = mutableListOf<String>()

        for (asset in allAssets) {
            when {
                isImageExtension(asset) -> images.add(asset)
                isAudioExtension(asset) -> audios.add(asset)
                isFontExtension(asset) -> fonts.add(asset)
                isVideoExtension(asset) -> videos.add(asset)
                isDataExtension(asset) -> datas.add(asset)
                else -> others.add(asset)
            }
        }

        val sampled = mutableListOf<String>()
        sampled.addAll(images.take(limitPerCategory))
        sampled.addAll(audios.take(limitPerCategory))
        sampled.addAll(fonts.take(limitPerCategory))
        sampled.addAll(videos.take(limitPerCategory))
        sampled.addAll(datas.take(limitPerCategory))
        if (sampled.isEmpty()) {
            sampled.addAll(others.take(limitPerCategory))
        }

        return sampled.distinct().sorted()
    }

    private fun isImageExtension(path: String): Boolean {
        val lower = path.lowercase()
        return lower.endsWith(".png") || lower.endsWith(".jpg") || lower.endsWith(".jpeg") ||
                lower.endsWith(".bmp") || lower.endsWith(".webp") || lower.endsWith(".gif") ||
                lower.endsWith(".rpgmvp")
    }

    private fun isAudioExtension(path: String): Boolean {
        val lower = path.lowercase()
        return lower.endsWith(".ogg") || lower.endsWith(".mp3") || lower.endsWith(".wav") ||
                lower.endsWith(".m4a") || lower.endsWith(".mid") || lower.endsWith(".midi") ||
                lower.endsWith(".rpgmvo")
    }

    private fun isFontExtension(path: String): Boolean {
        val lower = path.lowercase()
        return lower.endsWith(".ttf") || lower.endsWith(".otf") || lower.endsWith(".woff") ||
                lower.endsWith(".woff2")
    }

    private fun isVideoExtension(path: String): Boolean {
        val lower = path.lowercase()
        return lower.endsWith(".mp4") || lower.endsWith(".webm") || lower.endsWith(".ogv")
    }

    private fun isDataExtension(path: String): Boolean {
        val lower = path.lowercase()
        return lower.endsWith(".json") || lower.endsWith(".rvdata2") || lower.endsWith(".rvdata") ||
                lower.endsWith(".rxdata") || lower.endsWith(".txt") || lower.endsWith(".csv")
    }

    private fun buildCombinedSummary(auditDir: File, runtimeReport: RuntimeValidationReport): CombinedValidationSummary {
        val staticCandidates = listOf(
            File(gameDir, "asset-audit-report.json"),
            File(auditDir, "asset-audit-report.json")
        )

        var staticOutcome = "NOT_RUN"
        var staticSummary: StaticSummaryInfo? = null

        for (cand in staticCandidates) {
            if (cand.exists()) {
                try {
                    val text = cand.readText(Charsets.UTF_8)
                    val outMatch = Regex("\"outcome\"\\s*:\\s*\"([^\"]+)\"").find(text)
                    if (outMatch != null) {
                        staticOutcome = outMatch.groupValues[1]
                    }
                    val modeMatch = Regex("\"mode\"\\s*:\\s*\"([^\"]+)\"").find(text)
                    val mode = modeMatch?.groupValues?.get(1) ?: "STANDARD"

                    val nodesMatch = Regex("\"nodes\"\\s*:\\s*([0-9]+)").find(text)
                    val nodes = nodesMatch?.groupValues?.get(1)?.toIntOrNull() ?: 0

                    val refsMatch = Regex("\"references\"\\s*:\\s*([0-9]+)").find(text)
                    val refs = refsMatch?.groupValues?.get(1)?.toIntOrNull() ?: 0

                    val findingsMatch = Regex("\"findings\"\\s*:\\s*([0-9]+)").find(text)
                    val findings = findingsMatch?.groupValues?.get(1)?.toIntOrNull() ?: 0

                    staticSummary = StaticSummaryInfo(
                        mode = mode,
                        nodes = nodes,
                        references = refs,
                        findings = findings
                    )
                    break
                } catch (_: Exception) {}
            }
        }

        val runtimeOutcome = runtimeReport.outcome.name
        val combinedOutcome = when {
            runtimeOutcome == "FAIL" || staticOutcome == "FAIL" -> "FAIL"
            runtimeOutcome == "INDETERMINATE" || staticOutcome == "INDETERMINATE" -> "INDETERMINATE"
            runtimeOutcome == "WARNING" || staticOutcome == "WARNING" -> "WARNING"
            else -> "PASS"
        }

        val runtimeSummary = RuntimeSummaryInfo(
            mode = runtimeReport.mode.name,
            total_probed = runtimeReport.total_probed,
            passed_count = runtimeReport.passed_count,
            failed_count = runtimeReport.failed_count,
            warning_count = runtimeReport.warning_count,
            stage_counts = runtimeReport.stage_counts,
            fallbacks_detected = runtimeReport.fallbacks_detected
        )

        val discrepancies = mutableListOf<DiscrepancyItem>()
        if (staticOutcome == "PASS" && runtimeOutcome == "FAIL") {
            for (ev in runtimeReport.events) {
                if (ev.outcome == RuntimeAuditOutcome.FAIL && ev.asset_path != "root" && ev.asset_path != "all") {
                    discrepancies.add(
                        DiscrepancyItem(
                            asset_path = ev.asset_path,
                            static_status = "PASS",
                            runtime_status = "FAIL",
                            issue = ev.error_message ?: "Runtime decode failed"
                        )
                    )
                }
            }
        }

        return CombinedValidationSummary(
            schema_version = 1,
            summary_id = "summary-" + System.currentTimeMillis(),
            engine = engine,
            static_outcome = staticOutcome,
            runtime_outcome = runtimeOutcome,
            combined_outcome = combinedOutcome,
            static_summary = staticSummary,
            runtime_summary = runtimeSummary,
            discrepancies = discrepancies,
            timestamp = currentIsoTimestamp()
        )
    }
}
