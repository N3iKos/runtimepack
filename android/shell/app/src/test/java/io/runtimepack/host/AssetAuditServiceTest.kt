package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class AssetAuditServiceTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Before
    fun setUp() {
        AssetAuditService.reset()
    }

    @Test
    fun testMonotonicSequenceIds() {
        val ev1 = AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.REFERENCE_FOUND,
            assetPath = "img/characters/Hero.png",
            outcome = RuntimeAuditOutcome.PASS
        )
        val ev2 = AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.PATH_RESOLVED,
            assetPath = "img/characters/Hero.png",
            outcome = RuntimeAuditOutcome.PASS
        )
        val ev3 = AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "img/characters/Hero.png",
            outcome = RuntimeAuditOutcome.PASS
        )

        assertTrue("Sequence IDs must be strictly monotonic", ev1.sequence_id < ev2.sequence_id)
        assertTrue("Sequence IDs must be strictly monotonic", ev2.sequence_id < ev3.sequence_id)
        assertEquals(1L, ev1.sequence_id)
        assertEquals(2L, ev2.sequence_id)
        assertEquals(3L, ev3.sequence_id)

        val subsequent = AssetAuditService.getEvents(afterSeq = ev1.sequence_id)
        assertEquals(2, subsequent.size)
        assertEquals(ev2.sequence_id, subsequent[0].sequence_id)
        assertEquals(ev3.sequence_id, subsequent[1].sequence_id)
    }

    @Test
    fun testQueueBoundingMax5000() {
        val totalToInsert = 5020
        for (i in 1..totalToInsert) {
            AssetAuditService.recordEvent(
                stage = RuntimeAuditStage.FILE_READABLE,
                assetPath = "asset_$i.png",
                outcome = RuntimeAuditOutcome.PASS
            )
        }

        val events = AssetAuditService.getEvents(afterSeq = 0L)
        assertEquals(5000, events.size)

        // Oldest events (1 to 20) should have been evicted
        val firstRetainedSeq = events.first().sequence_id
        assertEquals(21L, firstRetainedSeq)
        val lastRetainedSeq = events.last().sequence_id
        assertEquals(5020L, lastRetainedSeq)
    }

    @Test
    fun testFallbackRecordingAndOutcomeReduction() {
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "audio/bgm/Title.ogg",
            outcome = RuntimeAuditOutcome.PASS
        )
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_READABLE,
            assetPath = "audio/bgm/Title.ogg",
            outcome = RuntimeAuditOutcome.PASS
        )

        val passReport = AssetAuditService.getReport()
        assertNotNull(passReport)
        assertEquals(RuntimeAuditOutcome.PASS, passReport!!.outcome)
        assertEquals(2, passReport.total_events)
        assertTrue(passReport.fallbacks_detected.isEmpty())

        // Record a fallback event
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "img/characters/Missing.png",
            outcome = RuntimeAuditOutcome.FAIL,
            fallbackAction = "PLACEHOLDER_SUBSTITUTED",
            errorMessage = "Resource missing in VFS, transparent placeholder substituted"
        )

        val failReport = AssetAuditService.getReport()
        assertNotNull(failReport)
        assertEquals("Fallback must reduce audit outcome to FAIL", RuntimeAuditOutcome.FAIL, failReport!!.outcome)
        assertEquals(3, failReport.total_events)
        assertEquals(2, failReport.stage_counts[RuntimeAuditStage.FILE_EXISTS.name])
        assertEquals(1, failReport.stage_counts[RuntimeAuditStage.FILE_READABLE.name])
        assertTrue(
            "Fallbacks detected must list PLACEHOLDER_SUBSTITUTED",
            failReport.fallbacks_detected.contains("PLACEHOLDER_SUBSTITUTED")
        )
    }

    @Test
    fun testAtomicReportPersistence() {
        val rootDir = tempFolder.newFolder("audit_test_dir")
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "data/System.json",
            outcome = RuntimeAuditOutcome.PASS
        )

        val reportFile = AssetAuditService.persistReport(rootDir)
        assertTrue("Report file must exist", reportFile.exists())
        assertEquals("runtime-audit-report.json", reportFile.name)
        val parentDir = reportFile.parentFile
        assertNotNull(parentDir)
        assertEquals("audit", parentDir?.name)
        assertEquals("runtime-state", parentDir?.parentFile?.name)

        val tmpFile = File(reportFile.parentFile, "${reportFile.name}.tmp")
        assertFalse("Temporary file must not remain after atomic move", tmpFile.exists())

        val content = reportFile.readText(Charsets.UTF_8)
        assertTrue(content.contains("\"outcome\":\"PASS\""))
        assertTrue(content.contains("\"total_events\":1"))
        assertTrue(content.contains("data/System.json"))
    }

    @Test
    fun testResetClearsQueueAndResetsSequence() {
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "test.png",
            outcome = RuntimeAuditOutcome.PASS
        )
        assertEquals(1, AssetAuditService.getEvents(0L).size)

        AssetAuditService.reset()
        assertEquals(0, AssetAuditService.getEvents(0L).size)
        assertNull(AssetAuditService.getReport())

        val newEv = AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "test2.png",
            outcome = RuntimeAuditOutcome.PASS
        )
        assertEquals(1L, newEv.sequence_id)
    }
}
