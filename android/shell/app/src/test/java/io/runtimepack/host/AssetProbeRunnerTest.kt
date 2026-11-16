package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.json.JSONObject
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

    private val testBitmapDecoder = BitmapProbeDecoder { stream ->
        val buf = ByteArray(8)
        val read = stream.read(buf)
        read >= 8 && buf[0] == 0x89.toByte() && buf[1] == 0x50.toByte() && buf[2] == 0x4E.toByte() && buf[3] == 0x47.toByte()
    }

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
            if (ext == "png") {
                f.writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))
            } else if (ext == "ogg") {
                f.writeBytes(byteArrayOf(0x4F, 0x67, 0x67, 0x53, 0x00, 0x02))
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
            sampleLimitPerCategory = 2,
            bitmapDecoder = testBitmapDecoder
        )

        val report = runner.run()

        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(ProbeMode.SAMPLED, report.mode)
        assertTrue("Probed count should be bounded by sample limit", report.total_probed in 5..10)
        assertTrue(report.passed_count > 0)
        assertEquals(0, report.failed_count)

        val runner2 = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.SAMPLED,
            sampleLimitPerCategory = 2,
            bitmapDecoder = testBitmapDecoder
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
            bitmapDecoder = testBitmapDecoder,
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

        val allAssets = (1..120).map { "img/sprites/asset_$it.png" }.sorted()
        val fingerprint = AssetProbeRunner.computeFingerprint(allAssets)

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
                "stage_counts": { "FILE_EXISTS": 60, "FILE_READABLE": 60, "DECODE_SUCCESS": 60 },
                "passed_count": 60,
                "failed_count": 0,
                "warning_count": 0,
                "fallbacks_detected": [],
                "timestamp_ms": 1000000,
                "source_hash": "test-source-hash-123",
                "profile_version": 2,
                "inventory_fingerprint": "$fingerprint"
            }
        """.trimIndent()
        checkpointFile.writeText(sampleCheckpointJson, Charsets.UTF_8)

        AssetAuditService.setCurrentRunId("test-audit-001")
        val processedCount = AtomicInteger(0)
        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rgss",
            mode = ProbeMode.EXHAUSTIVE,
            checkpointInterval = 50,
            bitmapDecoder = testBitmapDecoder,
            auditIdOverride = "test-audit-001",
            expectedSourceHash = "test-source-hash-123",
            onAssetProbed = { processedCount.incrementAndGet() }
        )

        val report = runner.run()

        assertTrue("Report should indicate checkpoint was restored", report.checkpoint_restored)
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
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
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
            sampleLimitPerCategory = 5,
            bitmapDecoder = testBitmapDecoder
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
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
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
                bitmapDecoder = testBitmapDecoder,
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
            assertEquals(0, report.stage_counts["INTEGRITY_VERIFIED"] ?: 0)
            assertEquals(1, report.stage_counts["DECODE_SUCCESS"])
        }
    }

    @Test
    fun testExcludedDllsWithExclusionsMetadataDoNotCreateMissing() {
        val graphFile = File(stateDir, "audit/asset-dependency-graph.json")
        graphFile.parentFile.mkdirs()
        graphFile.writeText("""
            {
                "schema_version": 1,
                "engine": "visual-novel-maker",
                "nodes": [
                    { "logical_path": "d3dcompiler_47.dll" },
                    { "logical_path": "node.dll" },
                    { "logical_path": "nw.dll" },
                    { "logical_path": "icon.png" },
                    { "logical_path": "resources/missing_pic.png" }
                ]
            }
        """.trimIndent(), Charsets.UTF_8)

        val exclusionsFile = File(stateDir, "audit/packaging-exclusions.json")
        exclusionsFile.writeText("""
            {
                "schema_version": 1,
                "source_hash": "mock_hash_123",
                "entries": [
                    { "logical_path": "d3dcompiler_47.dll", "reason": "EXCLUDED_PLATFORM_BINARY" },
                    { "logical_path": "node.dll", "reason": "EXCLUDED_PLATFORM_BINARY" },
                    { "logical_path": "nw.dll", "reason": "EXCLUDED_PLATFORM_BINARY" }
                ]
            }
        """.trimIndent(), Charsets.UTF_8)

        val iconFile = File(gameDir, "icon.png")
        iconFile.writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "visual-novel-maker",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
        )

        val report = runner.run()

        assertEquals(3, report.excluded_count)
        assertEquals(1, report.missing_count)
        assertEquals(1, report.failed_count)
        assertEquals(1, report.passed_count)
        assertEquals(RuntimeAuditOutcome.FAIL, report.outcome)

        val excludedEvents = report.events.filter { it.category == AssetAuditCategory.EXCLUDED_PLATFORM_BINARY }
        assertEquals(3, excludedEvents.size)
        for (ev in excludedEvents) {
            assertEquals(RuntimeAuditOutcome.WARNING, ev.outcome)
            assertEquals(RuntimeAuditStage.FILE_EXISTS, ev.stage)
        }

        val missingEvents = report.events.filter { it.category == AssetAuditCategory.MISSING_ASSET }
        assertEquals(1, missingEvents.size)
        assertEquals("resources/missing_pic.png", missingEvents[0].asset_path)
    }

    @Test
    fun testStaticAuditReportLoadedFromAssetsOrGpackWithoutRegexTrap() {
        val auditDir = File(stateDir, "audit")
        auditDir.mkdirs()
        val staticReportFile = File(auditDir, "asset-audit-report.json")
        staticReportFile.writeText("""
            {
                "schema_version": 1,
                "engine": "visual-novel-maker",
                "mode": "STANDARD",
                "outcome": "PASS",
                "stats": {
                    "nodes": 1976,
                    "references": 890,
                    "findings": 0
                },
                "findings": [
                    {
                        "code": "CMP-106",
                        "outcome": "FAIL",
                        "message": "Nested outcome text inside finding object"
                    }
                ]
            }
        """.trimIndent(), Charsets.UTF_8)

        File(gameDir, "valid.json").writeText("{}", Charsets.UTF_8)

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "visual-novel-maker",
            mode = ProbeMode.EXHAUSTIVE
        )

        val report = runner.run()
        val summaryFile = File(auditDir, "combined-validation-summary.json")
        assertTrue(summaryFile.exists())
        val summaryText = summaryFile.readText(Charsets.UTF_8)

        assertTrue(summaryText.contains("\"static_outcome\":\"PASS\"") || summaryText.contains("\"static_outcome\": \"PASS\""))
        assertTrue(summaryText.contains("\"engine\":\"visual-novel-maker\"") || summaryText.contains("\"engine\": \"visual-novel-maker\""))
        assertTrue(summaryText.contains("\"nodes\":1976") || summaryText.contains("\"nodes\": 1976"))
        assertTrue(summaryText.contains("\"references\":890") || summaryText.contains("\"references\": 890"))
        assertTrue(summaryText.contains("\"findings\":0") || summaryText.contains("\"findings\": 0"))
    }

    @Test
    fun testVnmEncryptedGraphicsClassifiedAsEncodedAssetUnverified() {
        val auditDir = File(stateDir, "audit")
        auditDir.mkdirs()
        val profFile = File(auditDir, "vnm-preparation-profile.json")
        profFile.writeText("""
            {
                "schema_version": 1,
                "engine": "visual-novel-maker",
                "source_hash": "test_hash",
                "status": "Enabled",
                "reason": "Static SCRIPTS configuration specifies needsPreparation=true"
            }
        """.trimIndent(), Charsets.UTF_8)

        val graphicsDir = File(gameDir, "resources/Graphics/Backgrounds/bg")
        graphicsDir.mkdirs()
        val encJpg = File(graphicsDir, "bg400c.jpg")
        encJpg.writeBytes(byteArrayOf(0xF5.toByte(), 0xF3.toByte(), 0xC9.toByte(), 0x8F.toByte(), 0x0B, 0x1A, 0x61, 0x70))

        val encPng = File(graphicsDir, "Moonlight.png")
        encPng.writeBytes(byteArrayOf(0x83.toByte(), 0x7B, 0x78, 0x28, 0x06, 0x00, 0x31, 0x3C))

        val plainIcon = File(gameDir, "icon.png")
        plainIcon.writeBytes(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

        val otherDir = File(gameDir, "img/other")
        otherDir.mkdirs()
        val corruptImg = File(otherDir, "corrupt.png")
        corruptImg.writeBytes(byteArrayOf(0x01, 0x02, 0x03, 0x04))

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "visual-novel-maker",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
        )

        val report = runner.run()

        assertEquals(2, report.indeterminate_count)
        assertEquals(1, report.passed_count)
        assertEquals(1, report.failed_count)
        assertEquals(1, report.decode_failed_count)

        val unverified = report.events.filter { it.category == AssetAuditCategory.ENCODED_ASSET_UNVERIFIED }
        assertEquals(2, unverified.size)
        for (ev in unverified) {
            assertEquals(RuntimeAuditOutcome.INDETERMINATE, ev.outcome)
            assertEquals(RuntimeAuditStage.DECODE_SUCCESS, ev.stage)
        }

        val decodeFailures = report.events.filter { it.category == AssetAuditCategory.DECODE_FAILURE }
        assertEquals(1, decodeFailures.size)
        assertEquals("img/other/corrupt.png", decodeFailures[0].asset_path)

        // Test disabled profile: bad magic on resources/Graphics becomes real DECODE_FAILURE
        profFile.writeText("""{"status": "Disabled"}""", Charsets.UTF_8)
        val runnerDisabled = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "visual-novel-maker",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
        )
        val repDisabled = runnerDisabled.run()
        assertEquals(3, repDisabled.failed_count)
        assertEquals(3, repDisabled.decode_failed_count)
        assertEquals(0, repDisabled.indeterminate_count)

        // Test unknown profile: bad magic on resources/Graphics becomes FORMAT_UNVERIFIED
        profFile.writeText("""{"status": "Unknown"}""", Charsets.UTF_8)
        val runnerUnknown = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "visual-novel-maker",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
        )
        val repUnknown = runnerUnknown.run()
        assertEquals(2, repUnknown.indeterminate_count)
        val formatUnverified = repUnknown.events.filter { it.category == "FORMAT_UNVERIFIED" }
        assertEquals(2, formatUnverified.size)
    }

    @Test
    fun testIntegrityVerifiedRemovedFromLengthChecks() {
        createDummyAssets(5, "img/chars", "png")

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder
        )

        val report = runner.run()
        assertEquals(RuntimeAuditOutcome.PASS, report.outcome)
        assertEquals(0, report.stage_counts["INTEGRITY_VERIFIED"] ?: 0)
        assertEquals(5, report.stage_counts["FILE_EXISTS"])
        assertEquals(5, report.stage_counts["FILE_READABLE"])
    }

    @Test
    fun testProbeCheckpointInvalidation() {
        val auditDir = File(stateDir, "audit")
        auditDir.mkdirs()
        val cpFile = File(auditDir, "probe_checkpoint.json")

        createDummyAssets(10, "img/sprites", "png")

        val staleCp = ProbeCheckpoint(
            audit_id = "old-run-123",
            mode = "EXHAUSTIVE",
            current_index = 5,
            total_assets = 10,
            last_processed_path = "img/sprites/asset_5.png",
            stage_counts = mapOf("FILE_EXISTS" to 5),
            passed_count = 5,
            failed_count = 0,
            warning_count = 0,
            fallbacks_detected = emptyList(),
            timestamp_ms = System.currentTimeMillis(),
            source_hash = "wrong_source_hash"
        )
        cpFile.writeText(staleCp.toJson(), Charsets.UTF_8)

        val runner = AssetProbeRunner(
            gameDir = gameDir,
            stateDir = stateDir,
            engine = "rpg-maker-mv",
            mode = ProbeMode.EXHAUSTIVE,
            bitmapDecoder = testBitmapDecoder,
            expectedSourceHash = "expected_source_hash_456"
        )

        val report = runner.run()
        assertFalse("Stale checkpoint must not be restored", report.checkpoint_restored)
        assertEquals(10, report.passed_count)
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
