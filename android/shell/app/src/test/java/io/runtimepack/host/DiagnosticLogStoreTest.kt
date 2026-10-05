package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class DiagnosticLogStoreTest {

    @Before
    fun setUp() {
        DiagnosticLogStore.resetAll()
    }

    @Test
    fun testGameErrorRecordBoundsAndOmission() {
        for (i in 1..2500) {
            DiagnosticLogStore.recordError(
                message = "Error number $i",
                stack = "Error: at line $i",
                file = "js/main.js",
                line = i,
                column = 1,
                kind = "error",
                source = "GAME"
            )
        }

        val json = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(json.contains("\"total_recorded\":2500") || json.contains("\"total_recorded\": 2500"))
        assertTrue(json.contains("\"retained_count\":2000") || json.contains("\"retained_count\": 2000"))
        assertTrue(json.contains("\"omitted_count\":500") || json.contains("\"omitted_count\": 500"))
        assertFalse("Earliest errors must be evicted", json.contains("Error number 1,"))
        assertTrue("Recent errors must be retained", json.contains("Error number 2500"))
    }

    @Test
    fun testPerMessageAndStackTruncation() {
        val hugeMessage = "A".repeat(40000)
        DiagnosticLogStore.recordError(
            message = hugeMessage,
            stack = "stack trace",
            file = "file.js",
            line = 10,
            column = 5
        )

        val json = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(json.contains("\"truncated_count\":1") || json.contains("\"truncated_count\": 1"))
        assertTrue(json.contains("[TRUNCATED]"))
    }

    @Test
    fun testRecentLogsRingBufferCapacity() {
        for (i in 1..600) {
            DiagnosticLogStore.recordLog(
                level = "INFO",
                message = "Log message $i",
                source = "HOST"
            )
        }

        val json = DiagnosticLogStore.getDiagnosticLogsJson()
        assertTrue(json.contains("\"retained_count\":500") || json.contains("\"retained_count\": 500"))
        assertTrue(json.contains("\"omitted_count\":100") || json.contains("\"omitted_count\": 100"))
        assertFalse(json.contains("Log message 1\""))
        assertTrue(json.contains("Log message 600"))
    }

    @Test
    fun testCredentialRedaction() {
        val rawMsg = """Connecting to server with password="SuperSecret123" and token: "Bearer eyJhbGciOiJIUzI1NiJ9""""
        val sanitized = DiagnosticLogStore.sanitizeText(rawMsg)
        assertFalse(sanitized.contains("SuperSecret123"))
        assertTrue(sanitized.contains("[REDACTED]"))
    }

    @Test
    fun testJsonErrorParsing() {
        val validJson = """
            {
                "message": "Uncaught TypeError: Cannot read property of undefined",
                "filename": "js/plugins/ENGINE.js",
                "lineno": 120,
                "colno": 45,
                "stack": "TypeError: Cannot read property\n at ENGINE.js:120:45",
                "kind": "error"
            }
        """.trimIndent()

        val ok = DiagnosticLogStore.recordJsonError(validJson)
        assertTrue(ok)

        val json = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(json.contains("Cannot read property of undefined"))
        assertTrue(json.contains("ENGINE.js"))
        assertTrue(json.contains("\"line\":120") || json.contains("\"line\": 120"))
    }

    @Test
    fun testMultilingualUtf8ByteCounting() {
        val cjkMsg = "エラー: ファイルが見つかりません 🎮"
        val record = DiagnosticLogStore.recordError(
            message = cjkMsg,
            stack = "Stack at 行 42",
            file = "スクリプト.js"
        )
        val json = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(json.contains(cjkMsg))
        val serializedBytes = record.toJsonObjectString().toByteArray(Charsets.UTF_8).size
        assertTrue("Serialized bytes must exceed char length for multibyte text", serializedBytes > record.message.length + 50)
    }
}
