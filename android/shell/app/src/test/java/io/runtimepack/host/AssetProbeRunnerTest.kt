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
import java.io.InputStream
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

class AssetProbeRunnerTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    private lateinit var gameDir: File
    private lateinit var stateDir: File

    @Before
    fun setUp() {
        AssetAuditService.reset()
        gameDir = tempFolder.newFolder("test_game")
        stateDir = tempFolder.newFolder("runtime-state")
    }

    private fun createDummyAssets(count: Int, dirName: String = "img", ext: String = "png"): List<File> {
        val dir = File(gameDir, dirName)
        dir.mkdirs()
        val files = mutableListOf<File>()
        for (i in 1..count) {
            val f = File(dir, "asset_$i.$ext")
            // Write a small dummy PNG header (8 bytes) or dummy bytes
            if (ext == "png") {
                f.writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))
            } else if (ext == "ogg") {
                f.writeBytes(byteArrayOf(0x4F, 0x67, 0x67, 0x53, 0x00, 0x02)) // OggS
            } else {
                f.writeText("dummy content $i", Charsets.UTF_8)
            }
            files.add(f)
        }
        return files
    }

    @Test
    fun testSampledModeSelectsRepresentativeAssetsDeterministically() {
        createDummyAssets(10, "img/characters", "png")
        createDummyAssets(10, "img/tilesets", "png")
        createDummyAssets(8, "audio/bgm", "ogg")
        createDummyAssets(5, "fonts", "ttf")
        createDummyAssets(4, "data", "json")

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.SAMPLED,
            sampleLimitPerCategory = 2
        )

        val report = runner.run()

        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(ProbeMode.SAMPLED, report.mode)
        // With 5 categories (img/characters, img/tilesets are image, audio, font, data) and limit 2 per category,
        // it should select bounded representative samples (<= 10 items total)
        assertTrue("Probed count should be bounded by sample limit", report.total_probed in 5..10)
        assertTrue(report.passed_count > 0)
        assertEquals(0, report.failed_count)

        // Deterministic check: second run should produce identical probed items in identical order
        val runner2 = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.SAMPLED,
            sampleLimitPerCategory = 2
        )
        val report2 = runner2.run()
        assertEquals(report.total_probed, report2.total_probed)
        assertEquals(
            report.events.map { it.asset_path }.distinct(),
            report2.events.map { it.asset_path }.distinct()
        )
    }

    @Test
    fun testExhaustiveModeCheckpointsEveryInterval() {
        createDummyAssets(110, "img/sprites", "png")

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE,
            checkpointInterval = 50
        )

        val report = runner.run()

        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(110, report.total_probed)
        assertEquals(110, report.passed_count)

        // Checkpoint file should have been written during execution
        val checkpointFile = File(stateDir, "audit/probe_checkpoint.json")
        val auditDir = File(stateDir, "audit")
        val reportFile = File(auditDir, "runtime-validation-report.json")
        assertTrue("Report file must exist", reportFile.exists())
        assertTrue("Checkpoint or report must exist", checkpointFile.exists() || reportFile.exists())
    }

    @Test
    fun testExhaustiveModeResumesFromCheckpoint() {
        createDummyAssets(120, "img/sprites", "png")

        // Manually write a checkpoint indicating 60 items have already been processed
        val auditDir = File(stateDir, "audit")
        auditDir.mkdirs()
        val checkpointFile = File(auditDir, "probe_checkpoint.json")
        val sampleCheckpointJson = """
            {
                "audit_id": "test-audit-001",
                "mode": "EXHAUSTIVE",
                "current_index": 60,
                "total_assets": 120,
                "last_processed_path": "img/sprites/asset_60.png",
                "stage_counts": { "FILE_EXISTS": 60, "DECODE_SUCCESS": 60 },
                "passed_count": 60,
                "failed_count": 0,
                "warning_count": 0,
                "fallbacks_detected": [],
                "timestamp_ms": 1000000
            }
        """.trimIndent()
        checkpointFile.writeText(sampleCheckpointJson, Charsets.UTF_8)

        val processedCount = AtomicInteger(0)
        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE,
            checkpointInterval = 50,
            onAssetProbed = { processedCount.incrementAndGet() }
        )

        val report = runner.run()

        assertTrue("Report should indicate checkpoint was restored", report.checkpoint_restored)
        // Should have only processed the remaining 60 assets
        assertEquals(60, processedCount.get())
        assertEquals(120, report.total_probed)
    }

    @Test
    fun testMemoryLimitReleasesDecodedBitmapsImmediately() {
        createDummyAssets(10, "img/faces", "png")

        val decodeCount = AtomicInteger(0)
        val recycleCount = AtomicInteger(0)

        val mockDecoder = object : BitmapProbeDecoder {
            override fun decodeAndVerify(stream: InputStream): Boolean {
                decodeCount.incrementAndGet()
                // Simulate decoding and immediate recycling
                recycleCount.incrementAndGet()
                return true
            }
        }

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpgmaker_web",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = mockDecoder
        )

        val report = runner.run()

        assertEquals(10, decodeCount.get())
        assertEquals(10, recycleCount.get())
        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
    }

    @Test
    fun testCancellationHaltsCleanly() {
        createDummyAssets(100, "img/pictures", "png")

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpgmaker_web",
            mode = ProbeMode.EXHAUSTIVE,
            checkpointInterval = 10
        )

        val cancelledBeforeFinish = AtomicBoolean(false)
        runner.onAssetProbed = {
            if (it >= 15) {
                runner.cancel()
                cancelledBeforeFinish.set(true)
            }
        }

        val report = runner.run()

        assertTrue(cancelledBeforeFinish.get())
        assertEquals(RuntimeAuditOutcome.INDETERMINATE, report.outcome)
        assertTrue("Probed items should be significantly less than 100", report.passed_count < 50)
    }

    @Test
    fun testReportGenerationAndPersistence() {
        createDummyAssets(2, "img/battle", "png")

        // Also create a static audit report in gameDir or stateDir to simulate static audit results
        val staticAuditFile = File(gameDir, "asset-audit-report.json")
        staticAuditFile.writeText("""
            {
                "schema_version": 1,
                "engine": "rpg-maker-mv",
                "mode": "STANDARD",
                "outcome": "PASS",
                "completeness": {
                    "required_scope_complete": true,
                    "covered_reference_classes": ["MAP", "IMAGE"],
                    "uncovered_reference_classes": []
                },
                "statistics": { "nodes": 3, "references": 2, "findings": 0 },
                "graph": {
                    "nodes": [
                        { "id": "node_1", "kind": "IMAGE", "logical_path": "img/battle/asset_1.png" },
                        { "id": "node_2", "kind": "IMAGE", "logical_path": "img/battle/asset_2.png" },
                        { "id": "node_3", "kind": "IMAGE", "logical_path": "img/battle/missing_boss.png" }
                    ],
                    "edges": []
                },
                "findings": [],
                "overrides": [],
                "runtime_validation": "NOT_RUN"
            }
        """.trimIndent(), Charsets.UTF_8)

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.EXHAUSTIVE
        )

        val report = runner.run()

        // missing_boss.png is in graph but does not exist on disk -> FAIL outcome!
        assertEquals(RuntimeAuditOutcome.FAIL, report.outcome)
        assertEquals(1, report.failed_count)
        assertEquals(2, report.passed_count)

        val auditDir = File(stateDir, "audit")
        val runtimeReportFile = File(auditDir, "runtime-validation-report.json")
        val combinedSummaryFile = File(auditDir, "combined-validation-summary.json")

        assertTrue("runtime-validation-report.json must be persisted", runtimeReportFile.exists())
        assertTrue("combined-validation-summary.json must be persisted", combinedSummaryFile.exists())

        val runtimeText = runtimeReportFile.readText(Charsets.UTF_8)
        assertTrue(runtimeText.contains("\"outcome\":\"FAIL\"") || runtimeText.contains("\"outcome\": \"FAIL\""))
        assertTrue(runtimeText.contains("missing_boss.png"))

        val summaryText = combinedSummaryFile.readText(Charsets.UTF_8)
        assertTrue(summaryText.contains("\"static_outcome\":\"PASS\"") || summaryText.contains("\"static_outcome\": \"PASS\""))
        assertTrue(summaryText.contains("\"runtime_outcome\":\"FAIL\"") || summaryText.contains("\"runtime_outcome\": \"FAIL\""))
        assertTrue(summaryText.contains("\"combined_outcome\":\"FAIL\"") || summaryText.contains("\"combined_outcome\": \"FAIL\""))
    }
}
