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
        assertEquals(1, failReport.stage_counts[RuntimeAuditStage.FILE_EXISTS.name])
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

    @Test
    fun testGetAuditReportReturnsJson() {
        assertEquals("{}", AssetAuditService.getAuditReport())

        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "audio/bgm/Theme.ogg",
            outcome = RuntimeAuditOutcome.PASS
        )

        val reportJson = AssetAuditService.getAuditReport()
        assertTrue("Report JSON should contain audit_id", reportJson.contains("\"audit_id\""))
        assertTrue("Report JSON should contain outcome PASS", reportJson.contains("\"outcome\":\"PASS\""))
        assertTrue("Report JSON should contain total_events: 1", reportJson.contains("\"total_events\":1"))
    }

    @Test
    fun testRecordEventInvalidatesCachedReport() {
        val rootDir = tempFolder.newFolder("cache_invalidation_test")
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_EXISTS,
            assetPath = "asset1.png",
            outcome = RuntimeAuditOutcome.PASS
        )

        // Persist report caches the current report
        AssetAuditService.persistReport(rootDir)
        val initialReport = AssetAuditService.getReport()
        assertNotNull(initialReport)
        assertEquals(1, initialReport?.total_events)

        // Record a second event - must invalidate cached report
        AssetAuditService.recordEvent(
            stage = RuntimeAuditStage.FILE_READABLE,
            assetPath = "asset2.png",
            outcome = RuntimeAuditOutcome.PASS
        )

        val updatedReport = AssetAuditService.getReport()
        assertNotNull(updatedReport)
        assertEquals("New event must invalidate cache and update total_events", 2, updatedReport?.total_events)
    }

    @Test
    fun testProblemSnapshotRetentionAcross6000SuccessEvents() {
        AssetAuditService.reset()

        // 1. Record 890 early failure/indeterminate problem events
        for (i in 1..890) {
            AssetAuditService.recordEvent(
                RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.DECODE_SUCCESS,
                    asset_path = "resources/Graphics/pic_$i.png",
                    outcome = RuntimeAuditOutcome.INDETERMINATE,
                    category = AssetAuditCategory.ENCODED_ASSET_UNVERIFIED,
                    error_message = "VNM prepared graphic requires GS.DataPreparer"
                )
            )
        }

        // 2. Flood with 6,000 passing success events
        for (i in 1..6000) {
            AssetAuditService.recordEvent(
                RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = "audio/se/sound_$i.ogg",
                    outcome = RuntimeAuditOutcome.PASS
                )
            )
        }

        // 3. Query getFailuresJson - all 890 early issues MUST be retained!
        val failuresJson = AssetAuditService.getFailuresJson(2000)
        assertTrue(failuresJson.contains("\"total_failures\":890"))
        assertTrue(failuresJson.contains("\"retained_count\":890"))
        assertTrue(failuresJson.contains("\"omitted_count\":0"))
        assertTrue(failuresJson.contains("\"truncated\":false"))
        assertTrue(failuresJson.contains("resources/Graphics/pic_1.png"))
        assertTrue(failuresJson.contains("resources/Graphics/pic_890.png"))
        assertFalse(failuresJson.contains("audio/se/sound_"))

        // 4. Test overflow beyond 2000 limit
        AssetAuditService.reset()
        for (i in 1..2100) {
            AssetAuditService.recordEvent(
                RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.DECODE_SUCCESS,
                    asset_path = "img/err_$i.png",
                    outcome = RuntimeAuditOutcome.FAIL,
                    category = AssetAuditCategory.DECODE_FAILURE
                )
            )
        }
        val overflowJson = AssetAuditService.getFailuresJson(2000)
        assertTrue(overflowJson.contains("\"total_failures\":2100"))
        assertTrue(overflowJson.contains("\"retained_count\":2000"))
        assertTrue(overflowJson.contains("\"omitted_count\":100"))
        assertTrue(overflowJson.contains("\"truncated\":true"))
    }

    @Test
    fun testRunIsolationAndStaleEventFiltering() {
        AssetAuditService.reset()
        val run1Id = AssetAuditService.currentRunId

        // Record event in run 1
        AssetAuditService.recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                stage = RuntimeAuditStage.FILE_EXISTS,
                asset_path = "run1_asset.png",
                outcome = RuntimeAuditOutcome.PASS,
                audit_id = run1Id
            )
        )

        // Reset to start run 2
        AssetAuditService.reset()
        val run2Id = AssetAuditService.currentRunId
        assertTrue(run1Id != run2Id)

        // Stale event from run 1 (including a stale DONE event) arrives
        AssetAuditService.recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                stage = RuntimeAuditStage.RUNTIME_USE_SUCCESS,
                asset_path = "all",
                outcome = RuntimeAuditOutcome.PASS,
                is_complete = true,
                error_message = "DONE from stale run 1",
                audit_id = run1Id
            )
        )

        AssetAuditService.recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                stage = RuntimeAuditStage.DECODE_SUCCESS,
                asset_path = "stale_run1_bad.png",
                outcome = RuntimeAuditOutcome.FAIL,
                audit_id = run1Id
            )
        )

        // Run 2 records an event
        AssetAuditService.recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                stage = RuntimeAuditStage.FILE_EXISTS,
                asset_path = "run2_asset.png",
                outcome = RuntimeAuditOutcome.PASS,
                audit_id = run2Id
            )
        )

        val run2Events = AssetAuditService.getEvents()
        assertEquals(1, run2Events.size)
        assertEquals("run2_asset.png", run2Events[0].asset_path)
        assertFalse(run2Events.any { it.audit_id == run1Id })
        assertFalse(run2Events.any { it.error_message?.contains("DONE from stale run 1") == true })
    }

    @Test
    fun testCurrentActiveRunIssuesNotMaskedByOldSnapshotOnDisk() {
        val rootDir = tempFolder.newFolder("active_vs_disk_test")
        val auditDir = File(rootDir, "runtime-state/audit")
        auditDir.mkdirs()

        // Create an old snapshot with 890 issues
        val oldSnapshot = StringBuilder()
        oldSnapshot.append("{\"schema_version\":1,\"audit_id\":\"old-run-890\",\"engine\":\"vnmaker\",\"source_hash\":\"hash123\",\"total_failures\":890,\"retained_count\":890,\"omitted_count\":0,\"truncated\":false,\"items\":[")
        val oldItems = (1..890).map { "{\"sequence_id\":$it,\"stage\":\"DECODE_SUCCESS\",\"asset_path\":\"old_pic_$it.png\",\"outcome\":\"FAIL\"}" }
        oldSnapshot.append(oldItems.joinToString(","))
        oldSnapshot.append("]}")
        File(auditDir, "problem-snapshot.json").writeText(oldSnapshot.toString(), Charsets.UTF_8)

        AssetAuditService.init(rootDir, "vnmaker", null, "hash123")
        // Start a new audit (transition to RUN_ACTIVE)
        AssetAuditService.reset()

        // Record only 1 current issue
        AssetAuditService.recordEvent(
            RuntimeAuditEvent(
                sequence_id = 0L,
                stage = RuntimeAuditStage.DECODE_SUCCESS,
                asset_path = "img/current_bad.png",
                outcome = RuntimeAuditOutcome.FAIL
            )
        )

        val failuresJson = AssetAuditService.getFailuresJson(2000)
        val root = MiniJson.parseObject(failuresJson)
        assertNotNull(root)
        assertEquals(1, root!!.getInt("total_failures"))
        assertEquals(1, root.getInt("retained_count"))
        assertEquals(0, root.getInt("omitted_count"))
        assertFalse(root.getBoolean("truncated") == true)
        assertEquals("img/current_bad.png", root.getArray("items")!!.objects()[0].getString("asset_path"))
    }

    @Test
    fun testNewAuditWithZeroIssuesReturnsZero() {
        val rootDir = tempFolder.newFolder("zero_issues_test")
        AssetAuditService.init(rootDir, "vnmaker", null, "hash123")
        AssetAuditService.reset()

        for (i in 1..10) {
            AssetAuditService.recordEvent(
                RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_READABLE,
                    asset_path = "sound_$i.ogg",
                    outcome = RuntimeAuditOutcome.PASS
                )
            )
        }

        val json = AssetAuditService.getFailuresJson(2000)
        val root = MiniJson.parseObject(json)
        assertNotNull(root)
        assertEquals(0, root!!.getInt("total_failures"))
        assertEquals(0, root.getInt("retained_count"))
        assertTrue(root.getArray("items")!!.items.isEmpty())
        assertEquals(AssetAuditService.currentRunId, root.getString("audit_id"))
    }

    @Test
    fun testRestartExactIdentityRestoresBoundedSnapshotAndTrimsWithLimit() {
        val rootDir = tempFolder.newFolder("restart_identity_test")
        val auditDir = File(rootDir, "runtime-state/audit")
        auditDir.mkdirs()

        val snapBuilder = StringBuilder()
        snapBuilder.append("{\"schema_version\":1,\"audit_id\":\"historical-run-890\",\"timestamp\":\"2026-10-04T12:00:00Z\",\"engine\":\"vnmaker\",\"source_hash\":\"exact_hash_456\",\"total_failures\":890,\"retained_count\":890,\"omitted_count\":0,\"truncated\":false,\"items\":[")
        val items = (1..890).map { "{\"sequence_id\":$it,\"stage\":\"DECODE_SUCCESS\",\"asset_path\":\"pic_$it.png\",\"outcome\":\"FAIL\"}" }
        snapBuilder.append(items.joinToString(","))
        snapBuilder.append("]}")
        File(auditDir, "problem-snapshot.json").writeText(snapBuilder.toString(), Charsets.UTF_8)

        // Initialize in PROCESS_RECREATED_IDLE state
        AssetAuditService.init(rootDir, "vnmaker", null, "exact_hash_456")
        AssetAuditService.setSessionState(AuditSessionState.PROCESS_RECREATED_IDLE)

        // 1. Exact match restores full 890
        val fullJson = AssetAuditService.getFailuresJson(2000)
        val fullRoot = MiniJson.parseObject(fullJson)
        assertNotNull(fullRoot)
        assertEquals(890, fullRoot!!.getInt("total_failures"))
        assertEquals(890, fullRoot.getInt("retained_count"))
        assertEquals(0, fullRoot.getInt("omitted_count"))
        assertEquals(true, fullRoot.getBoolean("historical"))
        assertEquals("historical-run-890", fullRoot.getString("audit_id"))
        assertEquals("2026-10-04T12:00:00Z", fullRoot.getString("timestamp"))

        // 2. Limit 10 correctly computes omitted_count = 880 and truncated = true
        val trimmedJson = AssetAuditService.getFailuresJson(10)
        val trimmedRoot = MiniJson.parseObject(trimmedJson)
        assertNotNull(trimmedRoot)
        assertEquals(890, trimmedRoot!!.getInt("total_failures"))
        assertEquals(10, trimmedRoot.getInt("retained_count"))
        assertEquals(880, trimmedRoot.getInt("omitted_count"))
        assertEquals(true, trimmedRoot.getBoolean("truncated"))
        assertEquals(true, trimmedRoot.getBoolean("historical"))
    }

    @Test
    fun testRestartMismatchedOrMissingProofYieldsUnavailable() {
        val rootDir = tempFolder.newFolder("mismatch_test")
        val auditDir = File(rootDir, "runtime-state/audit")
        auditDir.mkdirs()

        File(auditDir, "problem-snapshot.json").writeText(
            "{\"schema_version\":1,\"audit_id\":\"old-run\",\"engine\":\"vnmaker\",\"source_hash\":\"old_hash\",\"total_failures\":5,\"items\":[]}",
            Charsets.UTF_8
        )

        // 1. Mismatched source hash
        AssetAuditService.init(rootDir, "vnmaker", null, "different_hash")
        AssetAuditService.setSessionState(AuditSessionState.PROCESS_RECREATED_IDLE)
        val mismatchJson = AssetAuditService.getFailuresJson(2000)
        val mismatchRoot = MiniJson.parseObject(mismatchJson)
        assertEquals("UNAVAILABLE", mismatchRoot?.getString("status"))
        assertEquals(0, mismatchRoot?.getInt("total_failures"))

        // 2. Missing source hash (null)
        AssetAuditService.init(rootDir, "vnmaker", null, null)
        AssetAuditService.setSessionState(AuditSessionState.PROCESS_RECREATED_IDLE)
        val nullSourceJson = AssetAuditService.getFailuresJson(2000)
        val nullRoot = MiniJson.parseObject(nullSourceJson)
        assertEquals("UNAVAILABLE", nullRoot?.getString("status"))
    }

    @Test
    fun testOversizeSnapshotRejectsBoundedFailure() {
        val rootDir = tempFolder.newFolder("oversize_snapshot_test")
        val auditDir = File(rootDir, "runtime-state/audit")
        auditDir.mkdirs()

        // Create a file > 4 MiB (4.5 MiB)
        val hugeFile = File(auditDir, "problem-snapshot.json")
        val largeBytes = ByteArray(4 * 1024 * 1024 + 512 * 1024) { 'A'.code.toByte() }
        hugeFile.writeBytes(largeBytes)

        AssetAuditService.init(rootDir, "vnmaker", null, "some_hash")
        AssetAuditService.setSessionState(AuditSessionState.PROCESS_RECREATED_IDLE)
        val json = AssetAuditService.getFailuresJson(2000)
        val root = MiniJson.parseObject(json)
        assertEquals("UNAVAILABLE", root?.getString("status"))
        assertTrue(root?.getString("reason")?.contains("exceeds 4 MiB budget") == true)
    }

    @Test
    fun testOrchestratedResetBetweenRunnerCheckAndCommitNeverOverwritesNewRun() {
        AssetAuditService.reset()
        val runA = AssetAuditService.currentRunId

        // Simulate reset called before runner A commits
        AssetAuditService.reset()
        val runB = AssetAuditService.currentRunId
        assertTrue(runA != runB)

        var actionExecuted = false
        val result = AssetAuditService.withCurrentRun(runA, { false }) {
            actionExecuted = true
            "committed_A"
        }

        assertNull("Stale run A commit must return null", result)
        assertFalse("Stale run A action must not be executed", actionExecuted)
    }
}

