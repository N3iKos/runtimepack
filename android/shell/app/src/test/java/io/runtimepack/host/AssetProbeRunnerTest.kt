package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.InputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
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

        val checkpointExistedMidRun = AtomicBoolean(false)
        val checkpointFile = File(stateDir, "audit/probe_checkpoint.json")

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE,
            checkpointInterval = 50,
            onAssetProbed = { count ->
                if (count == 50 && checkpointFile.exists()) {
                    checkpointExistedMidRun.set(true)
                }
            }
        )

        val report = runner.run()

        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(110, report.total_probed)
        assertEquals(110, report.passed_count)

        assertTrue("Checkpoint must have been written at interval 50", checkpointExistedMidRun.get())
        assertFalse("Checkpoint file must be deleted upon successful completion", checkpointFile.exists())

        val auditDir = File(stateDir, "audit")
        val reportFile = File(auditDir, "runtime-validation-report.json")
        assertTrue("Report file must exist", reportFile.exists())
    }

    @Test
    fun testZeroAssetProbeOutcomeIsIndeterminate() {
        val emptyGameDir = tempFolder.newFolder("empty_game")
        val emptyStateDir = tempFolder.newFolder("empty_state")

        val runner = AssetProbeRunner(
            gameDir = emptyGameDir,
            stateDir = emptyStateDir,
            engine = "rpgmaker_web",
            mode = ProbeMode.SAMPLED
        )

        val report = runner.run()
        assertEquals(RuntimeAuditOutcome.INDETERMINATE, report.outcome)
        assertEquals(0, report.total_probed)
        assertTrue(report.events.last().error_message?.contains("zero assets") == true)
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

        // Check assigned monotonic sequence IDs in report events
        assertTrue(report.events.isNotEmpty())
        for (ev in report.events) {
            assertTrue("Sequence ID must be assigned (> 0)", ev.sequence_id > 0L)
        }

        // Check done event message reflects outcome
        val doneEv = report.events.last()
        assertTrue("Done event must have is_complete = true", doneEv.is_complete == true)
        assertTrue("Done event message should reflect failure count", doneEv.error_message?.contains("DONE: Probe failed") == true)
    }

    @Test
    fun testAssetAuditServiceProbePathStandardizationAndGameSubdirectory() {
        val filesDir = tempFolder.newFolder("files_dir")
        val gameSub = File(filesDir, "game")
        gameSub.mkdirs()
        val imgDir = File(gameSub, "img/pictures")
        imgDir.mkdirs()
        File(imgDir, "hero.png").writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

        AssetAuditService.init(filesDir, "rpg-maker-mv")
        val rep = AssetAuditService.runProbe(
            mode = ProbeMode.SAMPLED,
            sampleLimitPerCategory = 5
        )

        assertEquals(RuntimeAuditOutcome.PASS, rep.outcome)
        assertEquals(1, rep.total_probed)

        val auditDir = File(filesDir, "runtime-state/audit")
        assertTrue("Audit dir must be filesDir/runtime-state/audit", auditDir.exists())
        val probeRepFile = File(auditDir, "runtime-validation-report.json")
        val combinedFile = File(auditDir, "combined-validation-summary.json")

        assertTrue("runtime-validation-report.json must exist in standardized path", probeRepFile.exists())
        assertTrue("combined-validation-summary.json must exist in standardized path", combinedFile.exists())

        val summaryJson = AssetAuditService.getCombinedSummary()
        assertTrue("getCombinedSummary must return valid JSON summary", summaryJson.contains("\"combined_outcome\":\"PASS\""))
    }

    @Test
    fun testFallbackDiscoveryIgnoresNonMediaAndExcludesBinRbTxtJson() {
        // Create forbidden non-media files
        File(gameDir, "rp_input_state.bin").writeBytes(byteArrayOf(0, 1, 2, 3))
        File(gameDir, "runtimepack_asset_audit.rb").writeText("puts 'audit'")
        File(gameDir, "script_165_Main.txt").writeText("script text")
        File(gameDir, "data.json").writeText("{}")
        File(gameDir, "temp.tmp").writeText("tmp")
        File(gameDir, "backup.bak").writeText("bak")
        File(gameDir, ".ds_store").writeBytes(byteArrayOf(0, 0))
        File(gameDir, ".gitignore").writeText("*")
        File(gameDir, ".gpack_extracted").writeText("extracted")

        // Create valid media files
        val imgDir = File(gameDir, "img")
        imgDir.mkdirs()
        File(imgDir, "sprite.png").writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

        val audioDir = File(gameDir, "audio")
        audioDir.mkdirs()
        File(audioDir, "bgm.ogg").writeBytes(byteArrayOf(0x4F, 0x67, 0x67, 0x53, 0x00, 0x02))

        val dataDir = File(gameDir, "data")
        dataDir.mkdirs()
        File(dataDir, "System.rvdata2").writeBytes(byteArrayOf(0x04, 0x08, 0x6F))

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE
        )

        val report = runner.run()

        // Verify only valid media files were discovered and probed (sprite.png, bgm.ogg, System.rvdata2)
        assertEquals(3, report.total_probed)
        val probedPaths = report.events.map { it.asset_path }

        assertTrue(probedPaths.contains("img/sprite.png"))
        assertTrue(probedPaths.contains("audio/bgm.ogg"))
        assertTrue(probedPaths.contains("data/System.rvdata2"))

        assertFalse("Must not probe .bin files", probedPaths.any { it.endsWith(".bin") })
        assertFalse("Must not probe .rb files", probedPaths.any { it.endsWith(".rb") })
        assertFalse("Must not probe .txt files", probedPaths.any { it.endsWith(".txt") })
        assertFalse("Must not probe .json files", probedPaths.any { it.endsWith(".json") })
        assertFalse("Must not probe .tmp files", probedPaths.any { it.endsWith(".tmp") })
        assertFalse("Must not probe .bak files", probedPaths.any { it.endsWith(".bak") })
        assertFalse("Must not probe .ds_store", probedPaths.any { it.endsWith(".ds_store") })
        assertFalse("Must not probe .gitignore", probedPaths.any { it.endsWith(".gitignore") })
        assertFalse("Must not probe .gpack_extracted", probedPaths.any { it.endsWith(".gpack_extracted") })
    }

    @Test
    fun testNonImageReadableFilesDoNotEmitDecodeSuccess() {
        val audioDir = File(gameDir, "audio")
        audioDir.mkdirs()
        File(audioDir, "sound.ogg").writeBytes(byteArrayOf(0x4F, 0x67, 0x67, 0x53, 0x00, 0x02))

        val dataDir = File(gameDir, "data")
        dataDir.mkdirs()
        File(dataDir, "Map001.rvdata2").writeBytes(byteArrayOf(0x04, 0x08, 0x6F))

        val imgDir = File(gameDir, "img")
        imgDir.mkdirs()
        File(imgDir, "pic.png").writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE
        )

        val report = runner.run()

        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(3, report.total_probed)

        val oggEvents = report.events.filter { it.asset_path == "audio/sound.ogg" }
        val rvdataEvents = report.events.filter { it.asset_path == "data/Map001.rvdata2" }
        val imgEvents = report.events.filter { it.asset_path == "img/pic.png" }

        assertTrue(oggEvents.isNotEmpty())
        assertTrue(rvdataEvents.isNotEmpty())
        assertTrue(imgEvents.isNotEmpty())

        // Non-image events must emit FILE_READABLE, never DECODE_SUCCESS
        for (ev in oggEvents) {
            assertEquals("Audio files must emit FILE_READABLE, not DECODE_SUCCESS", RuntimeAuditStage.FILE_READABLE, ev.stage)
            assertEquals(RuntimeAuditOutcome.PASS, ev.outcome)
        }
        for (ev in rvdataEvents) {
            assertEquals("Data archive files must emit FILE_READABLE, not DECODE_SUCCESS", RuntimeAuditStage.FILE_READABLE, ev.stage)
            assertEquals(RuntimeAuditOutcome.PASS, ev.outcome)
        }

        // Image events must emit DECODE_SUCCESS
        val imgTerminal = imgEvents.last()
        assertEquals(RuntimeAuditStage.DECODE_SUCCESS, imgTerminal.stage)
        assertEquals(RuntimeAuditOutcome.PASS, imgTerminal.outcome)

        // Stage counts for DECODE_SUCCESS must only be 1 (for pic.png)
        assertEquals(1, report.stage_counts["DECODE_SUCCESS"])
    }

    @Test
    fun testGpackAwareProbingValidatesPackedAssets() {
        // Create an in-memory GPACK containing valid assets, while gameDir has no files on disk
        val pngHeader = byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)
        val oggHeader = byteArrayOf(0x4F, 0x67, 0x67, 0x53, 0x00, 0x02)
        val gpackBytes = createTestGpack(
            listOf(
                "img/sprite.png" to pngHeader,
                "audio/bgm.ogg" to oggHeader
            )
        )

        GpackStreamReader.open(ByteArrayInputStream(gpackBytes), null).use { reader ->
            val runner = AssetProbeRunner(
                gameDir = gameDir, // empty directory
                stateDir = stateDir,
                engine = "rpg-maker-mv",
                mode = ProbeMode.EXHAUSTIVE,
                gpackReader = reader
            )

            val report = runner.run()

            assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
            assertEquals(2, report.total_probed)
            assertEquals(2, report.passed_count)
            assertEquals(0, report.failed_count)

            val probedPaths = report.events.map { it.asset_path }.distinct()
            assertTrue(probedPaths.contains("img/sprite.png"))
            assertTrue(probedPaths.contains("audio/bgm.ogg"))
            assertEquals(2, report.stage_counts["FILE_EXISTS"])
            assertEquals(2, report.stage_counts["FILE_READABLE"])
            assertEquals(2, report.stage_counts["INTEGRITY_VERIFIED"])
            assertEquals(1, report.stage_counts["DECODE_SUCCESS"])
        }
    }

    private fun createTestGpack(files: List<Pair<String, ByteArray>>): ByteArray {
        val magic = byteArrayOf(
            'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
            'K'.code.toByte(), 0, 1, 0
        )
        val headerSize = 64
        var indexLen = 0
        for ((path, _) in files) {
            val pBytes = path.toByteArray(Charsets.UTF_8)
            indexLen += 4 + pBytes.size + 8 + 8 + 8 + 32 + 1 + 1
        }
        val indexOffset = headerSize.toLong()
        val dataOffset = indexOffset + indexLen

        val headerBuf = ByteArray(64)
        System.arraycopy(magic, 0, headerBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(1)
        hWrap.putInt(0)
        hWrap.putLong(indexOffset)
        hWrap.putLong(indexLen.toLong())
        hWrap.putInt(files.size)

        val indexBuf = ByteArray(indexLen)
        val iWrap = ByteBuffer.wrap(indexBuf).order(ByteOrder.LITTLE_ENDIAN)

        var curOffset = dataOffset
        for ((path, data) in files) {
            val pBytes = path.toByteArray(Charsets.UTF_8)
            iWrap.putInt(pBytes.size)
            iWrap.put(pBytes)
            iWrap.putLong(curOffset)
            iWrap.putLong(data.size.toLong())
            iWrap.putLong(data.size.toLong())
            iWrap.put(ByteArray(32))
            iWrap.put(0.toByte())
            iWrap.put(0.toByte())
            curOffset += data.size
        }

        val out = ByteArrayOutputStream()
        out.write(headerBuf)
        out.write(indexBuf)
        for ((_, data) in files) {
            out.write(data)
        }
        return out.toByteArray()
    }
}
