package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class AssetAuditBridgeTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    private class TestRgssActivity(private val workingDir: File) : RgssActivity() {
        override fun getInputStateDirectory(): File = workingDir
        override fun dispatchNativeKeyDown(androidKey: Int) {}
        override fun dispatchNativeKeyUp(androidKey: Int) {}
    }

    @Before
    fun setUp() {
        AssetAuditService.reset()
    }

    @Test
    fun testRgssActivityAuditBridgesAndFileIngestion() {
        val testDir = tempFolder.newFolder("rgss_audit_bridge_test")
        val activity = TestRgssActivity(testDir)

        // 1. startAudit
        val startRes = activity.startAudit("""{"mode":"sampled"}""")
        assertTrue(startRes.contains("STARTED"))

        // 2. Write rp_audit_events.json in workingDir (simulating Ruby loader failure)
        val eventsFile = File(testDir, "rp_audit_events.json")
        val sampleEventsJson = """
            [
                {
                    "sequence_id": 10,
                    "timestamp_ns": 1000000,
                    "stage": "FILE_EXISTS",
                    "asset_path": "Graphics/Characters/Ghost.png",
                    "outcome": "FAIL",
                    "fallback_action": "PLACEHOLDER_SUBSTITUTED",
                    "error_message": "Errno::ENOENT: No such file or directory",
                    "source_location": "Bitmap.new"
                }
            ]
        """.trimIndent()
        eventsFile.writeText(sampleEventsJson, Charsets.UTF_8)

        // 3. getAuditEvents should ingest from file and return
        val rawEvents = activity.getAuditEvents(0L)
        assertTrue(rawEvents.startsWith("["))
        assertTrue(rawEvents.contains("Ghost.png"))
        assertTrue(rawEvents.contains("PLACEHOLDER_SUBSTITUTED"))

        // 4. postAuditEvent
        val postOk = activity.postAuditEvent("""
            {
                "stage": "FILE_EXISTS",
                "asset_path": "Audio/BGM/MissingBgm.ogg",
                "outcome": "FAIL",
                "fallback_action": "AUDIO_LOAD_SUPPRESSED",
                "source_location": "Audio.bgm_play"
            }
        """.trimIndent())
        assertTrue("postAuditEvent should succeed", postOk)

        // 5. getAuditReport
        val reportJson = activity.getAuditReport()
        assertTrue(reportJson.contains("\"outcome\":\"FAIL\""))
        assertTrue(reportJson.contains("PLACEHOLDER_SUBSTITUTED"))
        assertTrue(reportJson.contains("AUDIO_LOAD_SUPPRESSED"))

        // 6. Test File caching: calling getAuditEvents again without file change should not duplicate events
        val countBefore = activity.getAuditEvents(0L).split("sequence_id").size
        val countAfter = activity.getAuditEvents(0L).split("sequence_id").size
        assertEquals("Unchanged file should not duplicate ingested events", countBefore, countAfter)

        // 7. cancelAudit
        val cancelOk = activity.cancelAudit()
        assertTrue(cancelOk)
        val cancelledReport = activity.getAuditReport()
        assertTrue(cancelledReport.contains("Audit cancelled by user"))

        // 8. startAudit resets file and removes rp_audit_events.json
        assertTrue("Events file existed before startAudit", eventsFile.exists())
        activity.startAudit()
        assertFalse("startAudit must remove old rp_audit_events.json", eventsFile.exists())
    }

    @Test
    fun testWebActivityAuditBridges() {
        val activity = WebActivity()

        // 1. startAudit
        val startRes = activity.startAudit("""{"mode":"exhaustive"}""")
        assertTrue(startRes.contains("STARTED"))

        // 2. postAuditEvent from JS web loader
        val ok = activity.postAuditEvent("""
            {
                "stage": "FILE_EXISTS",
                "asset_path": "img/pictures/HeroPortrait.png",
                "outcome": "FAIL",
                "fallback_action": "PLACEHOLDER_SUBSTITUTED",
                "error_message": "Failed to load bitmap",
                "source_location": "ImageManager.loadBitmap"
            }
        """.trimIndent())
        assertTrue(ok)

        // 3. getAuditEvents
        val rawEvents = activity.getAuditEvents(0L)
        assertTrue(rawEvents.contains("HeroPortrait.png"))
        assertTrue(rawEvents.contains("ImageManager.loadBitmap"))

        // 4. getAuditReport
        val report = activity.getAuditReport()
        assertTrue(report.contains("\"outcome\":\"FAIL\""))
        assertTrue(report.contains("HeroPortrait.png"))

        // 5. cancelAudit
        val cancelled = activity.cancelAudit()
        assertTrue(cancelled)
    }

    @Test
    fun testRgssActivityAuditReportFallbackToFile() {
        val testDir = tempFolder.newFolder("rgss_audit_report_fallback_test")
        val activity = TestRgssActivity(testDir)

        // When AssetAuditService is empty ("{}"), and no file exists:
        assertEquals("{}", activity.getAuditReport())

        // Create audit_report.json in dir
        val reportFile = File(testDir, "audit_report.json")
        reportFile.writeText("""{"stats":{"total_maps_scanned":10,"missing_count":2}}""", Charsets.UTF_8)

        val reportFromDir = activity.getAuditReport()
        assertTrue("Fallback report from dir should contain stats", reportFromDir.contains("total_maps_scanned"))
        assertTrue(reportFromDir.contains("\"missing_count\":2"))

        // Test fallback in game/ subdirectory
        reportFile.delete()
        val gameDir = File(testDir, "game")
        gameDir.mkdirs()
        val gameReportFile = File(gameDir, "audit_report.json")
        gameReportFile.writeText("""{"stats":{"total_maps_scanned":25,"missing_count":5}}""", Charsets.UTF_8)

        val reportFromGame = activity.getAuditReport()
        assertTrue("Fallback report from game/ should contain stats", reportFromGame.contains("total_maps_scanned"))
        assertTrue(reportFromGame.contains("\"missing_count\":5"))
    }
}
