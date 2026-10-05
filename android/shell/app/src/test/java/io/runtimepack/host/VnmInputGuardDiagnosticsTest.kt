package io.runtimepack.host

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files

class VnmInputGuardDiagnosticsTest {
    private lateinit var tempDir: File

    @Before
    fun setUp() {
        tempDir = Files.createTempDirectory("vnm-diag-test").toFile()
        VnmInputGuardDiagnostics.resetAll()
    }

    @After
    fun tearDown() {
        VnmInputGuardDiagnostics.resetAll()
        tempDir.deleteRecursively()
    }

    @Test
    fun testDisabledWhenOffOrNonVnm() {
        VnmInputGuardDiagnostics.configure(isVnm = false, guardMode = "transition-aware", baseDir = tempDir)
        assertFalse(VnmInputGuardDiagnostics.isEnabled)
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("{\"schema_version\":1,\"mode\":\"transition-aware\"}"))

        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "off", baseDir = tempDir)
        assertFalse(VnmInputGuardDiagnostics.isEnabled)
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("{\"schema_version\":1,\"mode\":\"transition-aware\"}"))
    }

    @Test
    fun testValidSnapshotAccepted() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        assertTrue(VnmInputGuardDiagnostics.isEnabled)

        val json = """
            {
                "schema_version": 1,
                "sequence": 1,
                "elapsed_ms": 1000,
                "mode": "transition-aware",
                "frame_count": 60,
                "frame_delta": 60,
                "graphics_paused": false,
                "graphics_stopped": false,
                "graphics_frozen": false,
                "transition_present": true,
                "transition_alpha": 128,
                "transition_duration": 30,
                "left_button_state": true,
                "button_down": false,
                "button_up": false,
                "mouse_down_count": 1,
                "mouse_up_count": 1,
                "touch_cancel_count": 0,
                "dialogue_callback_count": 1,
                "blocked_dialogue_count": 1,
                "delegated_dialogue_count": 0,
                "choice_count": null,
                "video_present": false,
                "skip_active": false,
                "auto_active": false,
                "unknown_state_count": 0
            }
        """.trimIndent()

        val accepted = VnmInputGuardDiagnostics.postSnapshot(json)
        assertTrue(accepted)
        assertEquals(1, VnmInputGuardDiagnostics.getSnapshotCount())
        assertEquals(1L, VnmInputGuardDiagnostics.getTotalRecorded())
        assertEquals(0, VnmInputGuardDiagnostics.getOmittedCount())

        val report = VnmInputGuardDiagnostics.getSnapshotsJson()
        assertTrue(report.contains("\"total_recorded\":1"))
        assertTrue(report.contains("\"retained_count\":1"))
        assertTrue(report.contains("\"transition_alpha\":128"))
    }

    @Test
    fun testRejectUnknownKeys() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        val json = """{"schema_version":1,"mode":"transition-aware","player_text":"hello secret"}"""
        assertFalse(VnmInputGuardDiagnostics.postSnapshot(json))
        assertEquals(0, VnmInputGuardDiagnostics.getSnapshotCount())
    }

    @Test
    fun testRejectOversizePayload() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        val bigVal = "1".repeat(2050)
        val json = """{"schema_version":1,"mode":"transition-aware","sequence":$bigVal}"""
        assertFalse(VnmInputGuardDiagnostics.postSnapshot(json))
        assertEquals(0, VnmInputGuardDiagnostics.getSnapshotCount())
    }

    @Test
    fun testRejectMalformedJsonAndWrongTypes() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("{not json}"))
        assertFalse(VnmInputGuardDiagnostics.postSnapshot(""))
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("""{"schema_version":2,"mode":"transition-aware"}"""))
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("""{"schema_version":1,"mode":"other"}"""))
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("""{"schema_version":1,"mode":"transition-aware","graphics_frozen":"not-a-bool"}"""))
        assertFalse(VnmInputGuardDiagnostics.postSnapshot("""{"schema_version":1,"mode":"transition-aware","frame_count":{"sub":1}}"""))
    }

    @Test
    fun testFifoBoundedRetentionAt64Records() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        for (i in 1..70) {
            val json = """{"schema_version":1,"sequence":$i,"mode":"transition-aware"}"""
            val accepted = VnmInputGuardDiagnostics.postSnapshot(json)
            assertTrue(accepted)
        }

        assertEquals(64, VnmInputGuardDiagnostics.getSnapshotCount())
        assertEquals(70L, VnmInputGuardDiagnostics.getTotalRecorded())
        assertEquals(6, VnmInputGuardDiagnostics.getOmittedCount())

        val report = VnmInputGuardDiagnostics.getSnapshotsJson()
        assertTrue(report.contains("\"retained_count\":64"))
        assertTrue(report.contains("\"omitted_count\":6"))
        assertFalse(report.contains("\"sequence\":1,"))
        assertTrue(report.contains("\"sequence\":70"))
    }

    @Test
    fun testAtomicPersistence() {
        VnmInputGuardDiagnostics.configure(isVnm = true, guardMode = "transition-aware", baseDir = tempDir)
        val json = """{"schema_version":1,"sequence":42,"mode":"transition-aware"}"""
        assertTrue(VnmInputGuardDiagnostics.postSnapshot(json))

        VnmInputGuardDiagnostics.persistNow()

        val persisted = File(tempDir, "vnm_input_snapshots.json")
        assertTrue(persisted.exists())
        val content = persisted.readText()
        assertTrue(content.contains("\"sequence\":42"))
    }
}
