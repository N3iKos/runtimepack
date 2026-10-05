package io.runtimepack.host

import java.util.concurrent.ConcurrentLinkedDeque
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

data class GameErrorRecord(
    val sequence_id: Long,
    val timestamp_ns: Long = System.nanoTime(),
    val timestamp: String = currentIsoTimestamp(),
    val message: String,
    val stack: String? = null,
    val file: String? = null,
    val line: Int? = null,
    val column: Int? = null,
    val kind: String = "error",
    val source: String = "GAME",
    val truncated: Boolean = false
) {
    fun toJsonObjectString(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"sequence_id\":").append(sequence_id).append(",")
        sb.append("\"timestamp_ns\":").append(timestamp_ns).append(",")
        sb.append("\"timestamp\":\"").append(escapeJson(timestamp)).append("\",")
        sb.append("\"message\":\"").append(escapeJson(message)).append("\",")
        if (stack != null) {
            sb.append("\"stack\":\"").append(escapeJson(stack)).append("\",")
        } else {
            sb.append("\"stack\":null,")
        }
        if (file != null) {
            sb.append("\"file\":\"").append(escapeJson(file)).append("\",")
        } else {
            sb.append("\"file\":null,")
        }
        if (line != null) {
            sb.append("\"line\":").append(line).append(",")
        } else {
            sb.append("\"line\":null,")
        }
        if (column != null) {
            sb.append("\"column\":").append(column).append(",")
        } else {
            sb.append("\"column\":null,")
        }
        sb.append("\"kind\":\"").append(escapeJson(kind)).append("\",")
        sb.append("\"source\":\"").append(escapeJson(source)).append("\",")
        sb.append("\"truncated\":").append(truncated)
        sb.append("}")
        return sb.toString()
    }
}

data class DiagnosticLogRecord(
    val time: String = currentIsoTimestamp(),
    val source: String,
    val level: String,
    val message: String,
    val stack: String? = null
) {
    fun toJsonObjectString(): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"time\":\"").append(escapeJson(time)).append("\",")
        sb.append("\"source\":\"").append(escapeJson(source)).append("\",")
        sb.append("\"level\":\"").append(escapeJson(level)).append("\",")
        sb.append("\"message\":\"").append(escapeJson(message)).append("\"")
        if (stack != null) {
            sb.append(",\"stack\":\"").append(escapeJson(stack)).append("\"")
        } else {
            sb.append(",\"stack\":null")
        }
        sb.append("}")
        return sb.toString()
    }
}

object DiagnosticLogStore {
    const val MAX_GAME_ERRORS = 2000
    const val MAX_TOTAL_ERROR_BYTES = 4 * 1024 * 1024
    const val MAX_FIELD_BYTES = 32 * 1024
    const val MAX_RECENT_LOGS = 500

    private val errorSeq = AtomicLong(0L)
    private val gameErrors = ConcurrentLinkedDeque<GameErrorRecord>()
    private val totalErrorBytes = AtomicLong(0L)
    private val omittedErrorsCount = AtomicInteger(0)
    private val truncatedErrorsCount = AtomicInteger(0)

    private val recentLogs = ConcurrentLinkedDeque<DiagnosticLogRecord>()
    private val omittedLogsCount = AtomicInteger(0)

    private val REDACT_PATTERNS = listOf(
        Regex("(?i)(\"?(?:password|token|secret|auth|authorization)\"?\\s*[:=]\\s*\")[^\"]*(\")"),
        Regex("(?i)(Bearer\\s+)[A-Za-z0-9_.-]+")
    )

    fun sanitizeText(text: String): String {
        var s = text
        for (p in REDACT_PATTERNS) {
            s = p.replace(s, "$1[REDACTED]$2")
        }
        return s
    }

    private fun boundString(str: String?): Pair<String?, Boolean> {
        if (str == null) return Pair(null, false)
        val sanitized = sanitizeText(str)
        val utf8Bytes = sanitized.toByteArray(Charsets.UTF_8)
        if (utf8Bytes.size > MAX_FIELD_BYTES) {
            truncatedErrorsCount.incrementAndGet()
            val truncatedStr = String(utf8Bytes, 0, MAX_FIELD_BYTES, Charsets.UTF_8)
            return Pair(truncatedStr + " [TRUNCATED]", true)
        }
        return Pair(sanitized, false)
    }

    fun recordError(
        message: String,
        stack: String? = null,
        file: String? = null,
        line: Int? = null,
        column: Int? = null,
        kind: String = "error",
        source: String = "GAME"
    ): GameErrorRecord {
        val (boundedMsg, msgTruncated) = boundString(message)
        val (boundedStack, stackTruncated) = boundString(stack)
        val isTruncated = msgTruncated || stackTruncated

        val seq = errorSeq.incrementAndGet()
        val record = GameErrorRecord(
            sequence_id = seq,
            message = boundedMsg ?: "",
            stack = boundedStack,
            file = file?.take(512),
            line = line,
            column = column,
            kind = kind,
            source = source,
            truncated = isTruncated
        )

        val recordSerialized = record.toJsonObjectString()
        val recordSize = recordSerialized.toByteArray(Charsets.UTF_8).size.toLong()

        synchronized(gameErrors) {
            gameErrors.addLast(record)
            totalErrorBytes.addAndGet(recordSize)

            while (gameErrors.size > MAX_GAME_ERRORS || totalErrorBytes.get() > MAX_TOTAL_ERROR_BYTES) {
                val removed = gameErrors.pollFirst()
                if (removed != null) {
                    val remSize = removed.toJsonObjectString().toByteArray(Charsets.UTF_8).size.toLong()
                    totalErrorBytes.addAndGet(-remSize)
                    omittedErrorsCount.incrementAndGet()
                } else {
                    break
                }
            }
        }
        return record
    }

    fun recordJsonError(jsonStr: String): Boolean {
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

            fun extractInt(key: String): Int? {
                val pattern = Regex("\"$key\"\\s*:\\s*([0-9]+)")
                val match = pattern.find(jsonStr) ?: return null
                return match.groupValues[1].toIntOrNull()
            }

            val msg = extractString("message") ?: extractString("error") ?: "Unknown error"
            val stack = extractString("stack")
            val file = extractString("file") ?: extractString("filename")
            val line = extractInt("line") ?: extractInt("lineno")
            val col = extractInt("column") ?: extractInt("colno")
            val kind = extractString("kind") ?: "error"
            val source = extractString("source") ?: "GAME"

            recordError(msg, stack, file, line, col, kind, source)
            true
        } catch (_: Exception) {
            false
        }
    }

    fun recordLog(level: String, message: String, source: String = "HOST", stack: String? = null) {
        val (boundedMsg, _) = boundString(message)
        val (boundedStack, _) = boundString(stack)
        val record = DiagnosticLogRecord(
            source = source,
            level = level,
            message = boundedMsg ?: "",
            stack = boundedStack
        )
        synchronized(recentLogs) {
            recentLogs.addLast(record)
            while (recentLogs.size > MAX_RECENT_LOGS) {
                recentLogs.pollFirst()
                omittedLogsCount.incrementAndGet()
            }
        }
    }

    fun getDiagnosticErrorsJson(): String {
        val snapshot = synchronized(gameErrors) { gameErrors.toList() }
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"total_recorded\":").append(errorSeq.get()).append(",")
        sb.append("\"retained_count\":").append(snapshot.size).append(",")
        sb.append("\"omitted_count\":").append(omittedErrorsCount.get()).append(",")
        sb.append("\"truncated_count\":").append(truncatedErrorsCount.get()).append(",")
        sb.append("\"items\":[")
        sb.append(snapshot.joinToString(",") { it.toJsonObjectString() })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }

    fun getDiagnosticLogsJson(): String {
        val snapshot = synchronized(recentLogs) { recentLogs.toList() }
        val sb = StringBuilder()
        sb.append("{")
        sb.append("\"retained_count\":").append(snapshot.size).append(",")
        sb.append("\"omitted_count\":").append(omittedLogsCount.get()).append(",")
        sb.append("\"items\":[")
        sb.append(snapshot.joinToString(",") { it.toJsonObjectString() })
        sb.append("]")
        sb.append("}")
        return sb.toString()
    }

    fun clearRecentLogs() {
        synchronized(recentLogs) {
            recentLogs.clear()
            omittedLogsCount.set(0)
        }
    }

    fun resetAll() {
        synchronized(gameErrors) {
            gameErrors.clear()
            totalErrorBytes.set(0L)
            omittedErrorsCount.set(0)
            truncatedErrorsCount.set(0)
            errorSeq.set(0L)
        }
        clearRecentLogs()
    }
}
