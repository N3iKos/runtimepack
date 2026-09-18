package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class CheatBridgeTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    private class TestRgssActivity(private val workingDir: File) : RgssActivity() {
        override fun getInputStateDirectory(): File = workingDir
        override fun dispatchNativeKeyDown(androidKey: Int) {}
        override fun dispatchNativeKeyUp(androidKey: Int) {}
    }

    @Test
    fun testPostCheatCmdWritesAtomicFile() {
        val testDir = tempFolder.newFolder("cheat_test_1")
        val activity = TestRgssActivity(testDir)

        val cmdJson = """{"action":"set_variable","id":10,"value":99}"""
        val ok = activity.postCheatCmd(cmdJson)
        assertTrue(ok)

        val targetFile = File(testDir, RgssActivity.CHEAT_CMD_FILE)
        assertTrue(targetFile.exists())
        assertEquals(cmdJson, targetFile.readText())

        val tmpFile = File(testDir, "${RgssActivity.CHEAT_CMD_FILE}.tmp")
        assertFalse(tmpFile.exists())
    }

    @Test
    fun testGetCheatStateDefaultAndPopulated() {
        val testDir = tempFolder.newFolder("cheat_test_2")
        val activity = TestRgssActivity(testDir)

        assertEquals("{}", activity.getCheatState())

        val stateFile = File(testDir, RgssActivity.CHEAT_STATE_FILE)
        val stateJson = """{"gold":5000,"variables":{"1":42},"switches":{"2":true}}"""
        stateFile.writeText(stateJson)

        assertEquals(stateJson, activity.getCheatState())
    }

    @Test
    fun testGetEvalResultReadsAndDeletes() {
        val testDir = tempFolder.newFolder("cheat_test_3")
        val activity = TestRgssActivity(testDir)

        assertEquals("", activity.getEvalResult())

        val evalFile = File(testDir, RgssActivity.CHEAT_EVAL_FILE)
        evalFile.writeText("12345")

        assertEquals("12345", activity.getEvalResult())
        assertFalse(evalFile.exists())
        assertEquals("", activity.getEvalResult())
    }
}
