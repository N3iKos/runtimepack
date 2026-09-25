package io.runtimepack.host

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

data class RuntimeAuditEvent(
    val sequence_id: Long,
    val timestamp_ns: Long = System.nanoTime(),
    val stage: RuntimeAuditStage,
    val asset_path: String,
    val outcome: RuntimeAuditOutcome,
    val fallback_action: String? = null,
    val error_message: String? = null,
    val source_location: String? = null
) {
    val sequenceId: Long get() = sequence_id
    val timestampNs: Long get() = timestamp_ns
    val assetPath: String get() = asset_path
    val fallbackAction: String? get() = fallback_action
    val errorMessage: String? get() = error_message
    val sourceLocation: String? get() = source_location

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
        sb.append("}")
        return sb.toString()
    }

    companion object {
        private fun escapeJson(s: String): String {
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

    companion object {
        private fun escapeJson(s: String): String {
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
    }
}
