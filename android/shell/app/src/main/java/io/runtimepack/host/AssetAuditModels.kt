package io.runtimepack.host

import java.io.InputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

enum class RuntimeAuditStage {
    REFERENCE_FOUND,
    PATH_RESOLVED,
    FILE_EXISTS,
    FILE_READABLE,
    INTEGRITY_VERIFIED,
    DECODE_SUCCESS,
    ENGINE_LOAD_SUCCESS,
    RUNTIME_USE_SUCCESS
}

enum class RuntimeAuditOutcome {
    PASS,
    WARNING,
    FAIL,
    INDETERMINATE,
    NOT_RUN
}

enum class ProbeMode {
    SAMPLED,
    EXHAUSTIVE
}

internal fun escapeJson(s: String): String {
    val sb = StringBuilder()
    for (c in s) {
        when (c) {
            '\\' -> sb.append("\\\\")
            '"' -> sb.append("\\\"")
            '\b' -> sb.append("\\b")
            '\u000C' -> sb.append("\\f")
            '\n' -> sb.append("\\n")
            '\r' -> sb.append("\\r")
            '\t' -> sb.append("\\t")
            else -> {
                if (c.code in 0x00..0x1F) {
                    sb.append(String.format("\\u%04x", c.code))
                } else {
                    sb.append(c)
                }
            }
        }
    }
    return sb.toString()
}

internal fun currentIsoTimestamp(): String {
    val sdf = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.US)
    sdf.timeZone = TimeZone.getTimeZone("UTC")
    return sdf.format(Date())
}

fun interface BitmapProbeDecoder {
    fun decodeAndVerify(stream: InputStream): Boolean
}

object DefaultBitmapProbeDecoder : BitmapProbeDecoder {
    override fun decodeAndVerify(stream: InputStream): Boolean {
        val buffered = if (stream.markSupported()) stream else java.io.BufferedInputStream(stream)
        buffered.mark(1024)
        var androidBmpSuccess = false
        try {
            val bmp = android.graphics.BitmapFactory.decodeStream(buffered)
            if (bmp != null) {
                try {
                    bmp.recycle()
                } catch (_: Throwable) {}
                androidBmpSuccess = true
            }
        } catch (_: Throwable) {}
        if (androidBmpSuccess) return true

        return try {
            buffered.reset()
            val header = ByteArray(8)
            val read = buffered.read(header)
            if (read >= 4) {
                val isPng = header[0] == 0x89.toByte() && header[1] == 0x50.toByte() && header[2] == 0x4E.toByte() && header[3] == 0x47.toByte()
                val isJpg = header[0] == 0xFF.toByte() && header[1] == 0xD8.toByte() && header[2] == 0xFF.toByte()
                val isGif = header[0] == 'G'.code.toByte() && header[1] == 'I'.code.toByte() && header[2] == 'F'.code.toByte()
                val isBmp = header[0] == 'B'.code.toByte() && header[1] == 'M'.code.toByte()
                val isRiff = header[0] == 'R'.code.toByte() && header[1] == 'I'.code.toByte() && header[2] == 'F'.code.toByte() && header[3] == 'F'.code.toByte()
                val isRpgmvp = header[0] == 'R'.code.toByte() && header[1] == 'P'.code.toByte() && header[2] == 'G'.code.toByte()
                isPng || isJpg || isGif || isBmp || isRiff || isRpgmvp
            } else {
                false
            }
        } catch (_: Throwable) {
            false
        }
    }
}

data class RuntimeAuditEvent(
    val sequence_id: Long,
    val timestamp_ns: Long = System.nanoTime(),
    val stage: RuntimeAuditStage,
    val asset_path: String,
    val outcome: RuntimeAuditOutcome,
    val fallback_action: String? = null,
    val error_message: String? = null,
    val source_location: String? = null,
    val is_complete: Boolean? = null
) {
    val sequenceId: Long get() = sequence_id
    val timestampNs: Long get() = timestamp_ns
    val assetPath: String get() = asset_path
    val fallbackAction: String? get() = fallback_action
    val errorMessage: String? get() = error_message
    val sourceLocation: String? get() = source_location
    val isComplete: Boolean? get() = is_complete

    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"sequence_id\":").append(sequence_id).append(",")
        sb.append("\"timestamp_ns\":").append(timestamp_ns).append(",")
        sb.append("\"stage\":\"").append(stage.name).append("\",")
        sb.append("\"asset_path\":\"").append(escapeJson(asset_path)).append("\",")
        sb.append("\"outcome\":\"").append(outcome.name).append("\"")
        if (fallback_action != null) {
            sb.append(",\"fallback_action\":\"").append(escapeJson(fallback_action)).append("\"")
        } else {
            sb.append(",\"fallback_action\":null")
        }
        if (error_message != null) {
            sb.append(",\"error_message\":\"").append(escapeJson(error_message)).append("\"")
        } else {
            sb.append(",\"error_message\":null")
        }
        if (source_location != null) {
            sb.append(",\"source_location\":\"").append(escapeJson(source_location)).append("\"")
        } else {
            sb.append(",\"source_location\":null")
        }
        if (is_complete != null) {
            sb.append(",\"is_complete\":").append(is_complete)
        }
        sb.append("}")
        return sb.toString()
    }

    companion object {
        fun fromJson(jsonStr: String): RuntimeAuditEvent? {
            return try {
                fun extractString(key: String): String? {
                    val pattern = Regex("\"$key\"\\s*:\\s*(?:\"((?:\\\\\"|[^\"])*)\"|null)")
                    val match = pattern.find(jsonStr) ?: return null
                    if (match.groupValues[1].isEmpty() && match.value.contains("null")) return null
                    return match.groupValues[1]
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\")
                        .replace("\\n", "\n")
                        .replace("\\r", "\r")
                        .replace("\\t", "\t")
                }

                fun extractLong(key: String): Long? {
                    val pattern = Regex("\"$key\"\\s*:\\s*([0-9]+)")
                    val match = pattern.find(jsonStr) ?: return null
                    return match.groupValues[1].toLongOrNull()
                }

                fun extractBoolean(key: String): Boolean? {
                    val pattern = Regex("\"$key\"\\s*:\\s*(true|false)")
                    val match = pattern.find(jsonStr) ?: return null
                    return match.groupValues[1].toBoolean()
                }

                val stageStr = extractString("stage") ?: "FILE_EXISTS"
                val stage = try {
                    RuntimeAuditStage.valueOf(stageStr)
                } catch (_: Exception) {
                    RuntimeAuditStage.FILE_EXISTS
                }

                val outcomeStr = extractString("outcome") ?: "FAIL"
                val outcome = try {
                    RuntimeAuditOutcome.valueOf(outcomeStr)
                } catch (_: Exception) {
                    RuntimeAuditOutcome.FAIL
                }

                val assetPath = extractString("asset_path") ?: ""
                val seq = extractLong("sequence_id") ?: 0L
                val ts = extractLong("timestamp_ns") ?: System.nanoTime()
                val fallback = extractString("fallback_action")
                val error = extractString("error_message")
                val sourceLoc = extractString("source_location")
                val isComplete = extractBoolean("is_complete")

                RuntimeAuditEvent(
                    sequence_id = seq,
                    timestamp_ns = ts,
                    stage = stage,
                    asset_path = assetPath,
                    outcome = outcome,
                    fallback_action = fallback,
                    error_message = error,
                    source_location = sourceLoc,
                    is_complete = isComplete
                )
            } catch (_: Exception) {
                null
            }
        }

        fun fromJsonArray(jsonStr: String): List<RuntimeAuditEvent> {
            val list = mutableListOf<RuntimeAuditEvent>()
            var depth = 0
            var inString = false
            var escape = false
            var startIdx = -1

            for (i in jsonStr.indices) {
                val c = jsonStr[i]
                if (escape) {
                    escape = false
                    continue
                }
                if (c == '\\') {
                    if (inString) escape = true
                    continue
                }
                if (c == '"') {
                    inString = !inString
                    continue
                }
                if (inString) continue

                if (c == '{') {
                    if (depth == 0) startIdx = i
                    depth++
                } else if (c == '}') {
                    depth--
                    if (depth == 0 && startIdx != -1) {
                        val objStr = jsonStr.substring(startIdx, i + 1)
                        fromJson(objStr)?.let { list.add(it) }
                        startIdx = -1
                    }
                }
            }
            return list
        }
    }
}

data class RuntimeAuditReport(
    val audit_id: String,
    val engine: String,
    val outcome: RuntimeAuditOutcome,
    val total_events: Int,
    val stage_counts: Map<String, Int>,
    val fallbacks_detected: List<String>,
    val events: List<RuntimeAuditEvent> = emptyList()
) {
    val auditId: String get() = audit_id
    val totalEvents: Int get() = total_events
    val stageCounts: Map<String, Int> get() = stage_counts
    val fallbacksDetected: List<String> get() = fallbacks_detected

    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"audit_id\":\"").append(escapeJson(audit_id)).append("\",")
        sb.append("\"engine\":\"").append(escapeJson(engine)).append("\",")
        sb.append("\"outcome\":\"").append(outcome.name).append("\",")
        sb.append("\"total_events\":").append(total_events).append(",")
        sb.append("\"stage_counts\":{")
        sb.append(stage_counts.entries.joinToString(",") { "\"${escapeJson(it.key)}\":${it.value}" })
        sb.append("},")
        sb.append("\"fallbacks_detected\":[")
        sb.append(fallbacks_detected.joinToString(",") { "\"${escapeJson(it)}\"" })
        sb.append("],")
        sb.append("\"events\":[")
        sb.append(events.joinToString(",") { it.toJson() })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }
}

data class RuntimeValidationReport(
    val schema_version: Int = 1,
    val audit_id: String,
    val engine: String,
    val mode: ProbeMode,
    val outcome: RuntimeAuditOutcome,
    val total_probed: Int,
    val passed_count: Int,
    val failed_count: Int,
    val warning_count: Int,
    val stage_counts: Map<String, Int>,
    val fallbacks_detected: List<String>,
    val checkpoint_restored: Boolean = false,
    val duration_ms: Long,
    val timestamp: String = currentIsoTimestamp(),
    val events: List<RuntimeAuditEvent> = emptyList()
) {
    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"schema_version\":").append(schema_version).append(",")
        sb.append("\"audit_id\":\"").append(escapeJson(audit_id)).append("\",")
        sb.append("\"engine\":\"").append(escapeJson(engine)).append("\",")
        sb.append("\"mode\":\"").append(mode.name).append("\",")
        sb.append("\"outcome\":\"").append(outcome.name).append("\",")
        sb.append("\"total_probed\":").append(total_probed).append(",")
        sb.append("\"passed_count\":").append(passed_count).append(",")
        sb.append("\"failed_count\":").append(failed_count).append(",")
        sb.append("\"warning_count\":").append(warning_count).append(",")
        sb.append("\"stage_counts\":{")
        sb.append(stage_counts.entries.joinToString(",") { "\"${escapeJson(it.key)}\":${it.value}" })
        sb.append("},")
        sb.append("\"fallbacks_detected\":[")
        sb.append(fallbacks_detected.joinToString(",") { "\"${escapeJson(it)}\"" })
        sb.append("],")
        sb.append("\"checkpoint_restored\":").append(checkpoint_restored).append(",")
        sb.append("\"duration_ms\":").append(duration_ms).append(",")
        sb.append("\"timestamp\":\"").append(escapeJson(timestamp)).append("\",")
        sb.append("\"events\":[")
        sb.append(events.joinToString(",") { it.toJson() })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }
}

data class StaticSummaryInfo(
    val mode: String,
    val nodes: Int,
    val references: Int,
    val findings: Int
) {
    fun toJson(): String {
        return "{\"mode\":\"${escapeJson(mode)}\",\"nodes\":$nodes,\"references\":$references,\"findings\":$findings}"
    }
}

data class RuntimeSummaryInfo(
    val mode: String,
    val total_probed: Int,
    val passed_count: Int,
    val failed_count: Int,
    val warning_count: Int,
    val stage_counts: Map<String, Int>,
    val fallbacks_detected: List<String>
) {
    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"mode\":\"").append(escapeJson(mode)).append("\",")
        sb.append("\"total_probed\":").append(total_probed).append(",")
        sb.append("\"passed_count\":").append(passed_count).append(",")
        sb.append("\"failed_count\":").append(failed_count).append(",")
        sb.append("\"warning_count\":").append(warning_count).append(",")
        sb.append("\"stage_counts\":{")
        sb.append(stage_counts.entries.joinToString(",") { "\"${escapeJson(it.key)}\":${it.value}" })
        sb.append("},")
        sb.append("\"fallbacks_detected\":[")
        sb.append(fallbacks_detected.joinToString(",") { "\"${escapeJson(it)}\"" })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }
}

data class DiscrepancyItem(
    val asset_path: String,
    val static_status: String? = null,
    val runtime_status: String? = null,
    val issue: String
) {
    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"asset_path\":\"").append(escapeJson(asset_path)).append("\",")
        if (static_status != null) {
            sb.append("\"static_status\":\"").append(escapeJson(static_status)).append("\",")
        } else {
            sb.append("\"static_status\":null,")
        }
        if (runtime_status != null) {
            sb.append("\"runtime_status\":\"").append(escapeJson(runtime_status)).append("\",")
        } else {
            sb.append("\"runtime_status\":null,")
        }
        sb.append("\"issue\":\"").append(escapeJson(issue)).append("\"")
        sb.append("}")
        return sb.toString()
    }
}

data class CombinedValidationSummary(
    val schema_version: Int = 1,
    val summary_id: String,
    val engine: String,
    val static_outcome: String,
    val runtime_outcome: String,
    val combined_outcome: String,
    val static_summary: StaticSummaryInfo? = null,
    val runtime_summary: RuntimeSummaryInfo? = null,
    val discrepancies: List<DiscrepancyItem> = emptyList(),
    val timestamp: String = currentIsoTimestamp()
) {
    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"schema_version\":").append(schema_version).append(",")
        sb.append("\"summary_id\":\"").append(escapeJson(summary_id)).append("\",")
        sb.append("\"engine\":\"").append(escapeJson(engine)).append("\",")
        sb.append("\"static_outcome\":\"").append(escapeJson(static_outcome)).append("\",")
        sb.append("\"runtime_outcome\":\"").append(escapeJson(runtime_outcome)).append("\",")
        sb.append("\"combined_outcome\":\"").append(escapeJson(combined_outcome)).append("\",")
        if (static_summary != null) {
            sb.append("\"static_summary\":").append(static_summary.toJson()).append(",")
        } else {
            sb.append("\"static_summary\":null,")
        }
        if (runtime_summary != null) {
            sb.append("\"runtime_summary\":").append(runtime_summary.toJson()).append(",")
        } else {
            sb.append("\"runtime_summary\":null,")
        }
        sb.append("\"discrepancies\":[")
        sb.append(discrepancies.joinToString(",") { it.toJson() })
        sb.append("],")
        sb.append("\"timestamp\":\"").append(escapeJson(timestamp)).append("\"")
        sb.append("}")
        return sb.toString()
    }
}

data class ProbeCheckpoint(
    val audit_id: String,
    val mode: String,
    val current_index: Int,
    val total_assets: Int,
    val last_processed_path: String,
    val stage_counts: Map<String, Int>,
    val passed_count: Int,
    val failed_count: Int,
    val warning_count: Int,
    val fallbacks_detected: List<String>,
    val timestamp_ms: Long = System.currentTimeMillis()
) {
    fun toJson(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"audit_id\":\"").append(escapeJson(audit_id)).append("\",")
        sb.append("\"mode\":\"").append(escapeJson(mode)).append("\",")
        sb.append("\"current_index\":").append(current_index).append(",")
        sb.append("\"total_assets\":").append(total_assets).append(",")
        sb.append("\"last_processed_path\":\"").append(escapeJson(last_processed_path)).append("\",")
        sb.append("\"stage_counts\":{")
        sb.append(stage_counts.entries.joinToString(",") { "\"${escapeJson(it.key)}\":${it.value}" })
        sb.append("},")
        sb.append("\"passed_count\":").append(passed_count).append(",")
        sb.append("\"failed_count\":").append(failed_count).append(",")
        sb.append("\"warning_count\":").append(warning_count).append(",")
        sb.append("\"fallbacks_detected\":[")
        sb.append(fallbacks_detected.joinToString(",") { "\"${escapeJson(it)}\"" })
        sb.append("],")
        sb.append("\"timestamp_ms\":").append(timestamp_ms)
        sb.append("}")
        return sb.toString()
    }

    companion object {
        fun fromJson(jsonStr: String): ProbeCheckpoint? {
            return try {
                fun extractString(key: String): String? {
                    val pattern = Regex("\"$key\"\\s*:\\s*(?:\"((?:\\\\\"|[^\"])*)\"|null)")
                    val match = pattern.find(jsonStr) ?: return null
                    if (match.groupValues[1].isEmpty() && match.value.contains("null")) return null
                    return match.groupValues[1]
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\")
                }

                fun extractInt(key: String): Int? {
                    val pattern = Regex("\"$key\"\\s*:\\s*([0-9]+)")
                    val match = pattern.find(jsonStr) ?: return null
                    return match.groupValues[1].toIntOrNull()
                }

                fun extractLong(key: String): Long? {
                    val pattern = Regex("\"$key\"\\s*:\\s*([0-9]+)")
                    val match = pattern.find(jsonStr) ?: return null
                    return match.groupValues[1].toLongOrNull()
                }

                fun extractStringMap(key: String): Map<String, Int> {
                    val map = mutableMapOf<String, Int>()
                    val objPattern = Regex("\"$key\"\\s*:\\s*\\{([^}]*)\\}")
                    val objMatch = objPattern.find(jsonStr) ?: return map
                    val inner = objMatch.groupValues[1]
                    val itemPattern = Regex("\"([^\"]+)\"\\s*:\\s*([0-9]+)")
                    for (m in itemPattern.findAll(inner)) {
                        val k = m.groupValues[1]
                        val v = m.groupValues[2].toIntOrNull() ?: 0
                        map[k] = v
                    }
                    return map
                }

                fun extractStringArray(key: String): List<String> {
                    val list = mutableListOf<String>()
                    val arrPattern = Regex("\"$key\"\\s*:\\s*\\[([^\\]]*)\\]")
                    val arrMatch = arrPattern.find(jsonStr) ?: return list
                    val inner = arrMatch.groupValues[1]
                    val itemPattern = Regex("\"((?:\\\\\"|[^\"])*)\"")
                    for (m in itemPattern.findAll(inner)) {
                        list.add(m.groupValues[1].replace("\\\"", "\""))
                    }
                    return list
                }

                val auditId = extractString("audit_id") ?: return null
                val mode = extractString("mode") ?: return null
                val currentIndex = extractInt("current_index") ?: return null
                val totalAssets = extractInt("total_assets") ?: return null
                val lastPath = extractString("last_processed_path") ?: ""
                val stageCounts = extractStringMap("stage_counts")
                val passed = extractInt("passed_count") ?: 0
                val failed = extractInt("failed_count") ?: 0
                val warning = extractInt("warning_count") ?: 0
                val fallbacks = extractStringArray("fallbacks_detected")
                val ts = extractLong("timestamp_ms") ?: System.currentTimeMillis()

                ProbeCheckpoint(
                    audit_id = auditId,
                    mode = mode,
                    current_index = currentIndex,
                    total_assets = totalAssets,
                    last_processed_path = lastPath,
                    stage_counts = stageCounts,
                    passed_count = passed,
                    failed_count = failed,
                    warning_count = warning,
                    fallbacks_detected = fallbacks,
                    timestamp_ms = ts
                )
            } catch (_: Exception) {
                null
            }
        }
    }
}
