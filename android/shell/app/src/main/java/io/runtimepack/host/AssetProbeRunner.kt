package io.runtimepack.host

import android.content.Context
import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import java.security.MessageDigest
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
    var onAssetProbed: ((Int) -> Unit)? = null,
    val context: Context? = null,
    val gpackReader: GpackStreamReader? = null,
    val auditIdOverride: String? = null,
    val expectedSourceHash: String? = null
) {
    private val isCancelled = AtomicBoolean(false)

    fun cancel() {
        isCancelled.set(true)
    }

    fun isCancelled(): Boolean = isCancelled.get()

    fun run(): RuntimeValidationReport {
        val startTime = System.currentTimeMillis()
        val auditId = auditIdOverride ?: AssetAuditService.currentRunId
        val auditDir = when {
            stateDir != null -> {
                if (stateDir.name == "audit") stateDir
                else if (stateDir.name == "runtime-state") File(stateDir, "audit")
                else File(stateDir, "runtime-state/audit")
            }
            else -> File(gameDir, "runtime-state/audit")
        }
        if (!auditDir.exists()) {
            auditDir.mkdirs()
        }

        val allAssets = discoverAssets(auditDir)
        val assetsToProbe = if (mode == ProbeMode.SAMPLED) {
            selectSampledAssets(allAssets, sampleLimitPerCategory)
        } else {
            allAssets
        }

        val inventoryFingerprint = computeFingerprint(assetsToProbe)
        val packagingExclusions = loadPackagingExclusions(auditDir)
        val vnmProfileStatus = loadVnmPreparationProfileStatus(auditDir)

        val checkpointFile = File(auditDir, "probe_checkpoint.json")
        var startIndex = 0
        var checkpointRestored = false
        var passedCount = 0
        var failedCount = 0
        var warningCount = 0
        var missingCount = 0
        var decodeFailedCount = 0
        var indeterminateCount = 0
        var excludedCount = 0
        var attemptedCount = 0

        val stageCounts = mutableMapOf<String, Int>()
        val stageAttemptCounts = mutableMapOf<String, Int>()
        val fallbacksDetected = mutableListOf<String>()
        val recordedEvents = mutableListOf<RuntimeAuditEvent>()

        if (mode == ProbeMode.EXHAUSTIVE && checkpointFile.exists()) {
            try {
                val cpText = checkpointFile.readText(Charsets.UTF_8)
                val checkpoint = ProbeCheckpoint.fromJson(cpText)
                val canResume = checkpoint != null &&
                    checkpoint.mode == mode.name &&
                    checkpoint.audit_id == auditId &&
                    expectedSourceHash != null &&
                    checkpoint.source_hash == expectedSourceHash &&
                    checkpoint.profile_version == CURRENT_PROBE_PROFILE_VERSION &&
                    checkpoint.inventory_fingerprint == inventoryFingerprint &&
                    checkpoint.total_assets == assetsToProbe.size &&
                    checkpoint.current_index in 1 until assetsToProbe.size

                if (canResume) {
                    startIndex = checkpoint!!.current_index
                    passedCount = checkpoint.passed_count
                    failedCount = checkpoint.failed_count
                    warningCount = checkpoint.warning_count
                    missingCount = checkpoint.missing_count
                    decodeFailedCount = checkpoint.decode_failed_count
                    indeterminateCount = checkpoint.indeterminate_count
                    excludedCount = checkpoint.excluded_count
                    attemptedCount = checkpoint.attempted_count
                    stageCounts.putAll(checkpoint.stage_counts)
                    stageAttemptCounts.putAll(checkpoint.stage_attempt_counts)
                    fallbacksDetected.addAll(checkpoint.fallbacks_detected)
                    checkpointRestored = true
                }
            } catch (_: Exception) {}
        }

        for (i in startIndex until assetsToProbe.size) {
            if (isCancelled.get() || auditId != AssetAuditService.currentRunId) {
                val cancelEv = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
                    asset_path = "root",
                    outcome = RuntimeAuditOutcome.INDETERMINATE,
                    error_message = "Probe cancelled by user",
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(cancelEv)
                recordedEvents.add(recorded)
                break
            }

            if (System.currentTimeMillis() - startTime > timeoutMs) {
                val timeoutEv = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
                    asset_path = "root",
                    outcome = RuntimeAuditOutcome.INDETERMINATE,
                    error_message = "Probe timed out after ${timeoutMs}ms",
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(timeoutEv)
                recordedEvents.add(recorded)
                break
            }

            val relPath = assetsToProbe[i]
            attemptedCount++

            val normalizedRel = relPath.replace('\\', '/').trimStart('/')
            val isExcluded = packagingExclusions.containsKey(normalizedRel) || packagingExclusions.containsKey(relPath)

            stageAttemptCounts["FILE_EXISTS"] = (stageAttemptCounts["FILE_EXISTS"] ?: 0) + 1

            if (isExcluded) {
                excludedCount++
                val reason = packagingExclusions[normalizedRel] ?: packagingExclusions[relPath] ?: "Platform binary excluded from package by policy: $relPath"
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.WARNING,
                    category = AssetAuditCategory.EXCLUDED_PLATFORM_BINARY,
                    error_message = reason,
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(ev)
                recordedEvents.add(recorded)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected,
                    missingCount, decodeFailedCount, indeterminateCount, excludedCount, attemptedCount,
                    stageAttemptCounts, expectedSourceHash, CURRENT_PROBE_PROFILE_VERSION, inventoryFingerprint
                )
                onAssetProbed?.invoke(i + 1)
                continue
            }

            val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader
            val hasPackFile = reader?.hasFile(relPath) == true
            val file = File(gameDir, relPath)
            val exists = hasPackFile || file.exists()

            if (!exists) {
                missingCount++
                failedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    category = AssetAuditCategory.MISSING_ASSET,
                    error_message = "File not found: $relPath",
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(ev)
                recordedEvents.add(recorded)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected,
                    missingCount, decodeFailedCount, indeterminateCount, excludedCount, attemptedCount,
                    stageAttemptCounts, expectedSourceHash, CURRENT_PROBE_PROFILE_VERSION, inventoryFingerprint
                )
                onAssetProbed?.invoke(i + 1)
                continue
            }

            stageCounts["FILE_EXISTS"] = (stageCounts["FILE_EXISTS"] ?: 0) + 1

            stageAttemptCounts["FILE_READABLE"] = (stageAttemptCounts["FILE_READABLE"] ?: 0) + 1
            val rawStream: InputStream = try {
                if (hasPackFile) {
                    reader?.openStream(relPath) ?: if (file.exists()) FileInputStream(file) else throw java.io.FileNotFoundException("GPACK stream was null for $relPath")
                } else {
                    FileInputStream(file)
                }
            } catch (e: Exception) {
                failedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    category = AssetAuditCategory.FILE_UNREADABLE,
                    error_message = "File unreadable: ${e.message}",
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(ev)
                recordedEvents.add(recorded)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected,
                    missingCount, decodeFailedCount, indeterminateCount, excludedCount, attemptedCount,
                    stageAttemptCounts, expectedSourceHash, CURRENT_PROBE_PROFILE_VERSION, inventoryFingerprint
                )
                onAssetProbed?.invoke(i + 1)
                continue
            }

            val buffered = if (rawStream.markSupported()) rawStream else java.io.BufferedInputStream(rawStream, 1024 * 1024)
            buffered.mark(1024 * 1024)
            val prefixBuf = ByteArray(128)
            val prefixReadLen = try {
                buffered.read(prefixBuf)
            } catch (e: Exception) {
                -1
            }

            if (prefixReadLen <= 0) {
                try { buffered.close() } catch (_: Exception) {}
                failedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    category = AssetAuditCategory.FILE_UNREADABLE,
                    error_message = "File is zero length or unreadable",
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(ev)
                recordedEvents.add(recorded)
                saveCheckpointIfNeeded(
                    checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                    stageCounts, passedCount, failedCount, warningCount, fallbacksDetected,
                    missingCount, decodeFailedCount, indeterminateCount, excludedCount, attemptedCount,
                    stageAttemptCounts, expectedSourceHash, CURRENT_PROBE_PROFILE_VERSION, inventoryFingerprint
                )
                onAssetProbed?.invoke(i + 1)
                continue
            }

            stageCounts["FILE_READABLE"] = (stageCounts["FILE_READABLE"] ?: 0) + 1
            try { buffered.reset() } catch (_: Exception) {}

            if (isImageExtension(relPath)) {
                stageAttemptCounts["DECODE_SUCCESS"] = (stageAttemptCounts["DECODE_SUCCESS"] ?: 0) + 1
                val isVnm = isVnmEngine(engine)
                val isVnmGraphics = isVnm && (relPath.startsWith("resources/Graphics/") || relPath.contains("/Graphics/"))
                val isStandardMagic = hasStandardImageMagic(prefixBuf, prefixReadLen)

                if (isVnmGraphics && !isStandardMagic) {
                    try { buffered.close() } catch (_: Exception) {}
                    when (vnmProfileStatus) {
                        "Enabled" -> {
                            indeterminateCount++
                            val ev = RuntimeAuditEvent(
                                sequence_id = 0L,
                                stage = RuntimeAuditStage.DECODE_SUCCESS,
                                asset_path = relPath,
                                outcome = RuntimeAuditOutcome.INDETERMINATE,
                                category = AssetAuditCategory.ENCODED_ASSET_UNVERIFIED,
                                error_message = "VNM prepared graphic requires GS.DataPreparer; no decryption attempted",
                                audit_id = auditId
                            )
                            val recorded = AssetAuditService.recordEvent(ev)
                            recordedEvents.add(recorded)
                        }
                        "Disabled" -> {
                            failedCount++
                            decodeFailedCount++
                            val ev = RuntimeAuditEvent(
                                sequence_id = 0L,
                                stage = RuntimeAuditStage.DECODE_SUCCESS,
                                asset_path = relPath,
                                outcome = RuntimeAuditOutcome.FAIL,
                                category = AssetAuditCategory.DECODE_FAILURE,
                                error_message = "Invalid image header with preparation disabled: $relPath",
                                audit_id = auditId
                            )
                            val recorded = AssetAuditService.recordEvent(ev)
                            recordedEvents.add(recorded)
                        }
                        else -> {
                            indeterminateCount++
                            val ev = RuntimeAuditEvent(
                                sequence_id = 0L,
                                stage = RuntimeAuditStage.DECODE_SUCCESS,
                                asset_path = relPath,
                                outcome = RuntimeAuditOutcome.INDETERMINATE,
                                category = "FORMAT_UNVERIFIED",
                                error_message = "Mismatched image signature with unverified preparation profile: $relPath",
                                audit_id = auditId
                            )
                            val recorded = AssetAuditService.recordEvent(ev)
                            recordedEvents.add(recorded)
                        }
                    }
                } else {
                    var decodeOk = false
                    var limitExceeded = false
                    var limitMsg: String? = null
                    try {
                        decodeOk = bitmapDecoder.decodeAndVerify(buffered)
                    } catch (e: VerificationLimitException) {
                        limitExceeded = true
                        limitMsg = e.message
                    } catch (_: Exception) {
                        decodeOk = false
                    } finally {
                        try { buffered.close() } catch (_: Exception) {}
                    }

                    if (limitExceeded) {
                        indeterminateCount++
                        val ev = RuntimeAuditEvent(
                            sequence_id = 0L,
                            stage = RuntimeAuditStage.DECODE_SUCCESS,
                            asset_path = relPath,
                            outcome = RuntimeAuditOutcome.INDETERMINATE,
                            category = AssetAuditCategory.VERIFICATION_LIMIT,
                            error_message = limitMsg ?: "Verification budget limit reached",
                            audit_id = auditId
                        )
                        val recorded = AssetAuditService.recordEvent(ev)
                        recordedEvents.add(recorded)
                    } else if (decodeOk) {
                        passedCount++
                        stageCounts["DECODE_SUCCESS"] = (stageCounts["DECODE_SUCCESS"] ?: 0) + 1
                        val ev = RuntimeAuditEvent(
                            sequence_id = 0L,
                            stage = RuntimeAuditStage.DECODE_SUCCESS,
                            asset_path = relPath,
                            outcome = RuntimeAuditOutcome.PASS,
                            audit_id = auditId
                        )
                        val recorded = AssetAuditService.recordEvent(ev)
                        recordedEvents.add(recorded)
                    } else {
                        failedCount++
                        decodeFailedCount++
                        val ev = RuntimeAuditEvent(
                            sequence_id = 0L,
                            stage = RuntimeAuditStage.DECODE_SUCCESS,
                            asset_path = relPath,
                            outcome = RuntimeAuditOutcome.FAIL,
                            category = AssetAuditCategory.DECODE_FAILURE,
                            error_message = "Asset decode verification failed: $relPath",
                            audit_id = auditId
                        )
                        val recorded = AssetAuditService.recordEvent(ev)
                        recordedEvents.add(recorded)
                    }
                }
            } else {
                try { buffered.close() } catch (_: Exception) {}
                passedCount++
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.PASS,
                    audit_id = auditId
                )
                val recorded = AssetAuditService.recordEvent(ev)
                recordedEvents.add(recorded)
            }

            saveCheckpointIfNeeded(
                checkpointFile, auditId, i + 1, assetsToProbe.size, relPath,
                stageCounts, passedCount, failedCount, warningCount, fallbacksDetected,
                missingCount, decodeFailedCount, indeterminateCount, excludedCount, attemptedCount,
                stageAttemptCounts, expectedSourceHash, CURRENT_PROBE_PROFILE_VERSION, inventoryFingerprint
            )
            onAssetProbed?.invoke(i + 1)
        }

        val duration = System.currentTimeMillis() - startTime
        val finalOutcome = when {
            isCancelled.get() || duration > timeoutMs -> RuntimeAuditOutcome.INDETERMINATE
            assetsToProbe.isEmpty() -> RuntimeAuditOutcome.INDETERMINATE
            failedCount > 0 -> RuntimeAuditOutcome.FAIL
            indeterminateCount > 0 -> RuntimeAuditOutcome.INDETERMINATE
            warningCount > 0 || excludedCount > 0 -> RuntimeAuditOutcome.WARNING
            else -> RuntimeAuditOutcome.PASS
        }

        val doneMsg = when {
            isCancelled.get() -> "DONE: Probe cancelled"
            duration > timeoutMs -> "DONE: Probe timed out"
            assetsToProbe.isEmpty() -> "DONE: Probe completed with zero assets (INDETERMINATE)"
            finalOutcome == RuntimeAuditOutcome.FAIL -> "DONE: Probe failed ($failedCount failures, $missingCount missing, $decodeFailedCount decode failed)"
            finalOutcome == RuntimeAuditOutcome.INDETERMINATE -> "DONE: Probe completed with indeterminate assets ($indeterminateCount unverified, $missingCount missing)"
            finalOutcome == RuntimeAuditOutcome.WARNING -> "DONE: Probe completed with warnings ($warningCount warnings, $excludedCount excluded)"
            else -> "DONE: Probe completed"
        }
        val doneEv = RuntimeAuditEvent(
            sequence_id = 0L,
            stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
            asset_path = "all",
            outcome = finalOutcome,
            error_message = doneMsg,
            source_location = "DONE",
            is_complete = true,
            audit_id = auditId
        )
        val recordedDone = AssetAuditService.recordEvent(doneEv)
        recordedEvents.add(recordedDone)

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
            missing_count = missingCount,
            decode_failed_count = decodeFailedCount,
            indeterminate_count = indeterminateCount,
            excluded_count = excludedCount,
            attempted_count = attemptedCount,
            stage_attempt_counts = stageAttemptCounts,
            stage_counts = stageCounts,
            fallbacks_detected = fallbacksDetected.distinct(),
            checkpoint_restored = checkpointRestored,
            duration_ms = duration,
            timestamp = currentIsoTimestamp(),
            events = recordedEvents
        )

        val committed = AssetAuditService.withCurrentRun(auditId, { isCancelled.get() }) {
            if (duration <= timeoutMs && assetsToProbe.isNotEmpty()) {
                try {
                    if (checkpointFile.exists()) {
                        checkpointFile.delete()
                    }
                } catch (_: Exception) {}
            }

            val runtimeReportFile = File(auditDir, "runtime-validation-report.json")
            val runtimeTmp = File(auditDir, "runtime-validation-report_${auditId}.json.tmp")
            runtimeTmp.writeText(runtimeReport.toJson(), Charsets.UTF_8)
            if (!runtimeTmp.renameTo(runtimeReportFile)) {
                runtimeReportFile.delete()
                runtimeTmp.renameTo(runtimeReportFile)
            }

            val combinedSummary = buildCombinedSummary(auditDir, runtimeReport)
            val combinedSummaryFile = File(auditDir, "combined-validation-summary.json")
            val combinedTmp = File(auditDir, "combined-validation-summary_${auditId}.json.tmp")
            combinedTmp.writeText(combinedSummary.toJson(), Charsets.UTF_8)
            if (!combinedTmp.renameTo(combinedSummaryFile)) {
                combinedSummaryFile.delete()
                combinedTmp.renameTo(combinedSummaryFile)
            }

            AssetAuditService.persistProblemSnapshot(auditDir, auditId)
            true
        }

        AssetAuditService.markRunnerComplete(this)

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
        fallbacksDetected: List<String>,
        missingCount: Int = 0,
        decodeFailedCount: Int = 0,
        indeterminateCount: Int = 0,
        excludedCount: Int = 0,
        attemptedCount: Int = 0,
        stageAttemptCounts: Map<String, Int> = emptyMap(),
        sourceHash: String? = null,
        profileVersion: Int? = null,
        inventoryFingerprint: String? = null
    ) {
        if (currentIndex % checkpointInterval == 0 || currentIndex == totalAssets) {
            AssetAuditService.withCurrentRun(auditId, { isCancelled.get() }) {
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
                        timestamp_ms = System.currentTimeMillis(),
                        missing_count = missingCount,
                        decode_failed_count = decodeFailedCount,
                        indeterminate_count = indeterminateCount,
                        excluded_count = excludedCount,
                        attempted_count = attemptedCount,
                        stage_attempt_counts = HashMap(stageAttemptCounts),
                        source_hash = sourceHash,
                        profile_version = profileVersion,
                        inventory_fingerprint = inventoryFingerprint
                    )
                    val tmp = File(checkpointFile.parentFile, "probe_checkpoint_${auditId}.json.tmp")
                    tmp.writeText(checkpoint.toJson(), Charsets.UTF_8)
                    if (!tmp.renameTo(checkpointFile)) {
                        checkpointFile.delete()
                        tmp.renameTo(checkpointFile)
                    }
                    true
                } catch (_: Exception) {
                    false
                }
            }
        }
    }

    companion object {
        val VALID_FALLBACK_MEDIA_EXTENSIONS = setOf(
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "rpgmvp",
            "ogg", "wav", "mp3", "m4a", "mid", "midi", "rpgmvo",
            "webm", "mp4", "ogv",
            "ttf", "otf", "woff", "woff2",
            "efkefc",
            "rxdata", "rvdata", "rvdata2"
        )

        val EXCLUDED_FALLBACK_EXTENSIONS = setOf(
            "bin", "rb", "txt", "json", "tmp", "bak", "ds_store", "gitignore", "gpack_extracted"
        )

        fun isVnmEngine(eng: String?): Boolean {
            if (eng.isNullOrBlank()) return false
            val lower = eng.lowercase()
            return lower.contains("vnm") || lower.contains("visual-novel-maker") || lower.contains("visual_novel_maker")
        }

        fun hasStandardImageMagic(header: ByteArray, len: Int): Boolean {
            if (len < 2) return false
            if (len >= 8 && header[0] == 0x89.toByte() && header[1] == 0x50.toByte() && header[2] == 0x4E.toByte() && header[3] == 0x47.toByte()
                && header[4] == 0x0D.toByte() && header[5] == 0x0A.toByte() && header[6] == 0x1A.toByte() && header[7] == 0x0A.toByte()) {
                return true
            }
            if (len >= 3 && header[0] == 0xFF.toByte() && header[1] == 0xD8.toByte() && header[2] == 0xFF.toByte()) {
                return true
            }
            if (len >= 6 && header[0] == 'G'.code.toByte() && header[1] == 'I'.code.toByte() && header[2] == 'F'.code.toByte()
                && header[3] == '8'.code.toByte() && (header[4] == '7'.code.toByte() || header[4] == '9'.code.toByte()) && header[5] == 'a'.code.toByte()) {
                return true
            }
            if (len >= 2 && header[0] == 'B'.code.toByte() && header[1] == 'M'.code.toByte()) {
                return true
            }
            if (len >= 12 && header[0] == 'R'.code.toByte() && header[1] == 'I'.code.toByte() && header[2] == 'F'.code.toByte() && header[3] == 'F'.code.toByte()
                && header[8] == 'W'.code.toByte() && header[9] == 'E'.code.toByte() && header[10] == 'B'.code.toByte() && header[11] == 'P'.code.toByte()) {
                return true
            }
            return false
        }

        fun computeFingerprint(paths: List<String>): String {
            val md = MessageDigest.getInstance("SHA-256")
            for (p in paths) {
                md.update(p.toByteArray(Charsets.UTF_8))
                md.update(0.toByte())
            }
            return md.digest().joinToString("") { "%02x".format(it) }
        }
    }

    private fun loadVnmPreparationProfileStatus(auditDir: File): String {
        val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader

        fun parseStatus(jsonText: String): String? {
            val root = MiniJson.parseObject(jsonText) ?: return null
            return root.getString("status")
        }

        if (context != null) {
            try {
                context.assets.open("vnm-preparation-profile.json").use { s ->
                    val status = parseStatus(s.bufferedReader(Charsets.UTF_8).use { it.readText() })
                    if (status != null) return status
                }
            } catch (_: Exception) {}
        }

        if (reader != null) {
            try {
                reader.openStream("vnm-preparation-profile.json")?.use { s ->
                    val status = parseStatus(s.bufferedReader(Charsets.UTF_8).use { it.readText() })
                    if (status != null) return status
                }
            } catch (_: Exception) {}
        }

        val candidates = listOfNotNull(
            File(gameDir, "vnm-preparation-profile.json"),
            File(auditDir, "vnm-preparation-profile.json"),
            stateDir?.let { File(it, "vnm-preparation-profile.json") },
            stateDir?.let { File(it, "audit/vnm-preparation-profile.json") },
            gameDir.parentFile?.let { File(it, "vnm-preparation-profile.json") }
        )
        for (c in candidates) {
            if (c.exists()) {
                val status = parseStatus(c.readText(Charsets.UTF_8))
                if (status != null) return status
            }
        }
        return "Unknown"
    }

    private fun loadPackagingExclusions(auditDir: File): Map<String, String> {
        val map = mutableMapOf<String, String>()
        val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader

        fun parseExclusions(jsonText: String) {
            val root = MiniJson.parseObject(jsonText) ?: return
            val entries = root.getArray("entries") ?: return
            for (obj in entries.objects()) {
                val path = obj.getString("logical_path")?.replace('\\', '/')?.trimStart('/')
                val reason = obj.getString("reason") ?: "EXCLUDED_PLATFORM_BINARY"
                if (path != null) {
                    map[path] = reason
                }
            }
        }

        if (context != null) {
            try {
                context.assets.open("packaging-exclusions.json").use { s ->
                    parseExclusions(s.bufferedReader(Charsets.UTF_8).use { it.readText() })
                }
                if (map.isNotEmpty()) return map
            } catch (_: Exception) {}
        }

        if (reader != null) {
            try {
                reader.openStream("packaging-exclusions.json")?.use { s ->
                    parseExclusions(s.bufferedReader(Charsets.UTF_8).use { it.readText() })
                }
                if (map.isNotEmpty()) return map
            } catch (_: Exception) {}
        }

        val candidates = listOfNotNull(
            File(gameDir, "packaging-exclusions.json"),
            File(auditDir, "packaging-exclusions.json"),
            stateDir?.let { File(it, "packaging-exclusions.json") },
            stateDir?.let { File(it, "audit/packaging-exclusions.json") },
            gameDir.parentFile?.let { File(it, "packaging-exclusions.json") }
        )
        for (c in candidates) {
            if (c.exists()) {
                parseExclusions(c.readText(Charsets.UTF_8))
                if (map.isNotEmpty()) return map
            }
        }
        return map
    }

    private fun discoverAssets(auditDir: File): List<String> {
        val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader
        val graphPaths = mutableListOf<String>()

        if (context != null) {
            try {
                context.assets.open("asset-dependency-graph.json").use { stream ->
                    val text = stream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                    graphPaths.addAll(parseLogicalPathsFromJson(text))
                }
            } catch (_: Throwable) {}

            if (graphPaths.isEmpty()) {
                try {
                    context.assets.open("asset-audit-report.json").use { stream ->
                        val text = stream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                        graphPaths.addAll(parseLogicalPathsFromJson(text))
                    }
                } catch (_: Throwable) {}
            }
        }

        if (graphPaths.isEmpty() && reader != null) {
            try {
                reader.openStream("asset-dependency-graph.json")?.use { stream ->
                    val text = stream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                    graphPaths.addAll(parseLogicalPathsFromJson(text))
                }
            } catch (_: Throwable) {}

            if (graphPaths.isEmpty()) {
                try {
                    reader.openStream("asset-audit-report.json")?.use { stream ->
                        val text = stream.bufferedReader(Charsets.UTF_8).use { it.readText() }
                        graphPaths.addAll(parseLogicalPathsFromJson(text))
                    }
                } catch (_: Throwable) {}
            }
        }

        if (graphPaths.isEmpty()) {
            val graphCandidates = listOfNotNull(
                File(gameDir, "asset-dependency-graph.json"),
                File(auditDir, "asset-dependency-graph.json"),
                File(gameDir, "asset-audit-report.json"),
                File(auditDir, "asset-audit-report.json"),
                stateDir?.let { File(it, "asset-dependency-graph.json") },
                stateDir?.let { File(it, "asset-audit-report.json") },
                gameDir.parentFile?.let { File(it, "asset-dependency-graph.json") },
                gameDir.parentFile?.let { File(it, "asset-audit-report.json") }
            )
            for (candidate in graphCandidates) {
                if (candidate.exists()) {
                    val parsed = parseLogicalPathsFromJson(candidate.readText(Charsets.UTF_8))
                    if (parsed.isNotEmpty()) {
                        graphPaths.addAll(parsed)
                        break
                    }
                }
            }
        }

        val unionSet = LinkedHashSet<String>(graphPaths)
        if (reader != null) {
            for (entry in reader.entries) {
                val rel = entry.logicalPath.replace('\\', '/').trimStart('/')
                if (!isExcludedPath(rel)) {
                    unionSet.add(rel)
                }
            }
        }

        if (unionSet.isEmpty() && gameDir.exists() && gameDir.isDirectory) {
            gameDir.walkTopDown().forEach { file ->
                if (file.isFile) {
                    val rel = file.relativeTo(gameDir).path.replace('\\', '/')
                    if (!isExcludedPath(rel) && isValidFallbackMediaExtension(rel)) {
                        unionSet.add(rel)
                    }
                }
            }
        }

        return unionSet.toList().sorted()
    }

    private fun parseLogicalPathsFromJson(jsonStr: String): List<String> {
        val paths = mutableListOf<String>()
        val root = MiniJson.parseObject(jsonStr)
        if (root != null) {
            val nodes = root.getArray("nodes")
            if (nodes != null) {
                for (n in nodes.objects()) {
                    val p = n.getString("logical_path")?.replace('\\', '/')?.trimStart('/')
                    if (!p.isNullOrBlank() && !isExcludedPath(p)) {
                        paths.add(p)
                    }
                }
            }
            val graph = root.getObject("graph")
            val gNodes = graph?.getArray("nodes")
            if (gNodes != null) {
                for (n in gNodes.objects()) {
                    val p = n.getString("logical_path")?.replace('\\', '/')?.trimStart('/')
                    if (!p.isNullOrBlank() && !isExcludedPath(p)) {
                        paths.add(p)
                    }
                }
            }
        }
        if (paths.isEmpty()) {
            val pattern = Regex("\"logical_path\"\\s*:\\s*\"([^\"]+)\"")
            for (match in pattern.findAll(jsonStr)) {
                val p = match.groupValues[1].replace('\\', '/').trimStart('/')
                if (p.isNotBlank() && !isExcludedPath(p)) {
                    paths.add(p)
                }
            }
        }
        return paths.distinct()
    }

    private fun isExcludedPath(path: String): Boolean {
        val lower = path.lowercase()
        return lower.startsWith(".git") ||
                lower.startsWith("runtime-state") ||
                lower.startsWith("save") ||
                lower.endsWith(".tmp") ||
                lower.endsWith(".bak") ||
                lower.endsWith(".ds_store") ||
                lower.endsWith(".gitignore") ||
                lower.endsWith(".gpack_extracted")
    }

    private fun isValidFallbackMediaExtension(path: String): Boolean {
        val lower = path.lowercase()
        val fileName = lower.substringAfterLast('/')
        if (fileName.startsWith(".git") || fileName == ".ds_store" || fileName == ".gitignore" || fileName == ".gpack_extracted") {
            return false
        }
        val ext = path.substringAfterLast('.', "").lowercase()
        if (ext in EXCLUDED_FALLBACK_EXTENSIONS) {
            return false
        }
        return ext in VALID_FALLBACK_MEDIA_EXTENSIONS
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
        var staticOutcome = "NOT_RUN"
        var staticSummary: StaticSummaryInfo? = null
        val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader

        fun parseStaticReport(text: String): Boolean {
            val root = MiniJson.parseObject(text) ?: run {
                staticOutcome = "INDETERMINATE"
                return false
            }
            val outcome = root.getString("outcome")
            if (outcome == null) {
                staticOutcome = "INDETERMINATE"
                return false
            }
            staticOutcome = outcome
            val mode = root.getString("mode") ?: "STANDARD"

            val statsObj = root.getObject("stats") ?: root.getObject("statistics")
            val nodes = statsObj?.getInt("nodes") ?: root.getObject("graph")?.getArray("nodes")?.items?.size ?: 0
            val refs = statsObj?.getInt("references") ?: 0
            val findings = statsObj?.getInt("findings") ?: root.getArray("findings")?.items?.size ?: 0

            staticSummary = StaticSummaryInfo(
                mode = mode,
                nodes = nodes,
                references = refs,
                findings = findings
            )
            return true
        }

        var loaded = false
        if (context != null) {
            try {
                context.assets.open("asset-audit-report.json").use { s ->
                    val text = s.bufferedReader(Charsets.UTF_8).use { it.readText() }
                    loaded = parseStaticReport(text)
                }
            } catch (_: Exception) {}
        }

        if (!loaded && reader != null) {
            try {
                reader.openStream("asset-audit-report.json")?.use { s ->
                    val text = s.bufferedReader(Charsets.UTF_8).use { it.readText() }
                    loaded = parseStaticReport(text)
                }
            } catch (_: Exception) {}
        }

        if (!loaded) {
            val staticCandidates = listOfNotNull(
                File(gameDir, "asset-audit-report.json"),
                File(auditDir, "asset-audit-report.json"),
                stateDir?.let { File(it, "asset-audit-report.json") },
                stateDir?.let { File(it, "audit/asset-audit-report.json") },
                gameDir.parentFile?.let { File(it, "asset-audit-report.json") }
            )
            for (cand in staticCandidates) {
                if (cand.exists()) {
                    try {
                        val text = cand.readText(Charsets.UTF_8)
                        if (parseStaticReport(text)) {
                            break
                        }
                    } catch (_: Exception) {}
                }
            }
        }

        val runtimeOutcome = runtimeReport.outcome.name
        val combinedOutcome = when {
            runtimeOutcome == "FAIL" || staticOutcome == "FAIL" -> "FAIL"
            runtimeOutcome == "INDETERMINATE" || staticOutcome == "INDETERMINATE" -> "INDETERMINATE"
            runtimeOutcome == "WARNING" || staticOutcome == "WARNING" -> "WARNING"
            runtimeOutcome == "PASS" -> "PASS"
            else -> "INDETERMINATE"
        }

        val runtimeSummary = RuntimeSummaryInfo(
            mode = runtimeReport.mode.name,
            total_probed = runtimeReport.total_probed,
            passed_count = runtimeReport.passed_count,
            failed_count = runtimeReport.failed_count,
            warning_count = runtimeReport.warning_count,
            missing_count = runtimeReport.missing_count,
            decode_failed_count = runtimeReport.decode_failed_count,
            indeterminate_count = runtimeReport.indeterminate_count,
            excluded_count = runtimeReport.excluded_count,
            attempted_count = runtimeReport.attempted_count,
            stage_attempt_counts = runtimeReport.stage_attempt_counts,
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
