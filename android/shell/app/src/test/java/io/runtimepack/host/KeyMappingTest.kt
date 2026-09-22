package io.runtimepack.host

import android.view.KeyEvent
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.concurrent.ConcurrentHashMap

class KeyMappingTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testMapJsKeyActionA() {
        val (androidKeys, vkKeys) = RgssActivity.mapJsKey(90)
        assertEquals(
            listOf(KeyEvent.KEYCODE_Z, KeyEvent.KEYCODE_ENTER),
            androidKeys
        )
        assertEquals(
            listOf(0x5A, 0x0D),
            vkKeys
        )
    }

    @Test
    fun testMapJsKeyCancelB() {
        val (androidKeys, vkKeys) = RgssActivity.mapJsKey(88)
        assertEquals(
            listOf(KeyEvent.KEYCODE_X, KeyEvent.KEYCODE_ESCAPE),
            androidKeys
        )
        assertEquals(
            listOf(0x58, 0x1B),
            vkKeys
        )
    }

    @Test
    fun testMapJsKeyDPad() {
        val up = RgssActivity.mapJsKey(38)
        assertEquals(listOf(KeyEvent.KEYCODE_DPAD_UP), up.first)
        assertEquals(listOf(0x26), up.second)

        val down = RgssActivity.mapJsKey(40)
        assertEquals(listOf(KeyEvent.KEYCODE_DPAD_DOWN), down.first)
        assertEquals(listOf(0x28), down.second)

        val left = RgssActivity.mapJsKey(37)
        assertEquals(listOf(KeyEvent.KEYCODE_DPAD_LEFT), left.first)
        assertEquals(listOf(0x25), left.second)

        val right = RgssActivity.mapJsKey(39)
        assertEquals(listOf(KeyEvent.KEYCODE_DPAD_RIGHT), right.first)
        assertEquals(listOf(0x27), right.second)
    }

    @Test
    fun testMapJsKeyModifiersAndControl() {
        val shift = RgssActivity.mapJsKey(16)
        assertEquals(listOf(KeyEvent.KEYCODE_SHIFT_LEFT), shift.first)
        assertEquals(listOf(0x10), shift.second)

        val ctrl = RgssActivity.mapJsKey(17)
        assertEquals(listOf(KeyEvent.KEYCODE_CTRL_LEFT), ctrl.first)
        assertEquals(listOf(0x11), ctrl.second)

        val esc = RgssActivity.mapJsKey(27)
        assertEquals(listOf(KeyEvent.KEYCODE_ESCAPE), esc.first)
        assertEquals(listOf(0x1B), esc.second)

        val enter = RgssActivity.mapJsKey(13)
        assertEquals(listOf(KeyEvent.KEYCODE_ENTER), enter.first)
        assertEquals(listOf(0x0D), enter.second)

        val space = RgssActivity.mapJsKey(32)
        assertEquals(listOf(KeyEvent.KEYCODE_SPACE), space.first)
        assertEquals(listOf(0x20), space.second)

        val q = RgssActivity.mapJsKey(81)
        assertEquals(listOf(KeyEvent.KEYCODE_Q), q.first)
        assertEquals(listOf(0x51), q.second)

        val e = RgssActivity.mapJsKey(69)
        assertEquals(listOf(KeyEvent.KEYCODE_E), e.first)
        assertEquals(listOf(0x45), e.second)
    }

    @Test
    fun testMapJsKeyAlt() {
        val alt = RgssActivity.mapJsKey(18)
        assertEquals(listOf(KeyEvent.KEYCODE_ALT_LEFT), alt.first)
        assertEquals(listOf(0x12), alt.second)
    }

    @Test
    fun testMapJsKeyDelete() {
        val del = RgssActivity.mapJsKey(46)
        assertEquals(listOf(KeyEvent.KEYCODE_FORWARD_DEL), del.first)
        assertEquals(listOf(0x2E), del.second)
    }

    @Test
    fun testMapJsKeyFunctionKeys() {
        for (i in 0..11) {
            val jsKey = 112 + i
            val (androidKeys, vkKeys) = RgssActivity.mapJsKey(jsKey)
            assertEquals(listOf(KeyEvent.KEYCODE_F1 + i), androidKeys)
            assertEquals(listOf(0x70 + i), vkKeys)
        }
    }

    @Test
    fun testMapJsKeyNumbers() {
        for (i in 0..9) {
            val jsKey = 48 + i
            val (androidKeys, vkKeys) = RgssActivity.mapJsKey(jsKey)
            assertEquals(listOf(KeyEvent.KEYCODE_0 + i), androidKeys)
            assertEquals(listOf(0x30 + i), vkKeys)
        }
    }

    @Test
    fun testMapJsKeyLettersFallback() {
        // Test letters like A (65), C (67), S (83), D (68), W (87)
        val testLetters = listOf(
            Triple(65, KeyEvent.KEYCODE_A, 0x41),
            Triple(67, KeyEvent.KEYCODE_C, 0x43),
            Triple(68, KeyEvent.KEYCODE_D, 0x44),
            Triple(83, KeyEvent.KEYCODE_S, 0x53),
            Triple(87, KeyEvent.KEYCODE_W, 0x57)
        )
        for ((jsKey, expectedAndroid, expectedVk) in testLetters) {
            val (androidKeys, vkKeys) = RgssActivity.mapJsKey(jsKey)
            assertEquals(listOf(expectedAndroid), androidKeys)
            assertEquals(listOf(expectedVk), vkKeys)
        }
    }

    @Test
    fun testMapJsKeyUnknownReturnsEmpty() {
        val (androidKeys, vkKeys) = RgssActivity.mapJsKey(9999)
        assertTrue(androidKeys.isEmpty())
        assertTrue(vkKeys.isEmpty())
    }

    @Test
    fun testVirtualKeyBufferStateUpdates() {
        val buffer = ByteArray(256)
        val (_, vkKeys) = RgssActivity.mapJsKey(90) // VK: 0x5A, 0x0D

        // Key Down: set mapped VK to 0x80
        for (vk in vkKeys) {
            buffer[vk] = 0x80.toByte()
        }
        assertEquals(0x80.toByte(), buffer[0x5A])
        assertEquals(0x80.toByte(), buffer[0x0D])
        assertEquals(0x00.toByte(), buffer[0x20])
        assertEquals(0x00.toByte(), buffer[0x00])
        assertEquals(0x00.toByte(), buffer[0x58])

        // Key Up: reset mapped VK to 0x00
        for (vk in vkKeys) {
            buffer[vk] = 0x00.toByte()
        }
        assertEquals(0x00.toByte(), buffer[0x5A])
        assertEquals(0x00.toByte(), buffer[0x0D])
        assertEquals(0x00.toByte(), buffer[0x20])
    }

    @Test
    fun testVirtualKeyBufferAtomicSerialization() {
        val targetDir = tempFolder.newFolder("rp_input_test")
        val buffer = ByteArray(256)

        // Set key 90 down (VK: 0x5A, 0x0D, 0x20)
        buffer[0x5A] = 0x80.toByte()
        buffer[0x0D] = 0x80.toByte()
        buffer[0x20] = 0x80.toByte()

        val success = RgssActivity.writeVirtualKeyBufferAtomic(buffer, targetDir)
        assertTrue(success)

        val stateFile = File(targetDir, RgssActivity.INPUT_STATE_FILE)
        assertTrue(stateFile.exists())
        assertEquals(256, stateFile.length())

        val readBytes = stateFile.readBytes()
        assertArrayEquals(buffer, readBytes)
        assertEquals(0x80.toByte(), readBytes[0x5A])
        assertEquals(0x80.toByte(), readBytes[0x0D])
        assertEquals(0x80.toByte(), readBytes[0x20])

        // Overwrite with key released
        buffer[0x5A] = 0x00.toByte()
        buffer[0x0D] = 0x00.toByte()
        buffer[0x20] = 0x00.toByte()

        val secondWrite = RgssActivity.writeVirtualKeyBufferAtomic(buffer, targetDir)
        assertTrue(secondWrite)

        val updatedBytes = stateFile.readBytes()
        assertArrayEquals(buffer, updatedBytes)
        assertEquals(0x00.toByte(), updatedBytes[0x5A])
        assertEquals(0x00.toByte(), updatedBytes[0x0D])
        assertEquals(0x00.toByte(), updatedBytes[0x20])

        // Ensure temp file was cleaned up
        val tmpFile = File(targetDir, "${RgssActivity.INPUT_STATE_FILE}.tmp")
        assertFalse(tmpFile.exists())
    }

    @Test
    fun testPressedJsKeysIdempotency() {
        val pressedKeys = ConcurrentHashMap.newKeySet<Int>()

        // First press should return true
        assertTrue(pressedKeys.add(90))
        // Duplicate press should return false (avoid double firing)
        assertFalse(pressedKeys.add(90))

        // First release should return true
        assertTrue(pressedKeys.remove(90))
        // Subsequent release should return false
        assertFalse(pressedKeys.remove(90))
    }

    @Test
    fun testMultiKeyBufferStateOverlap() {
        val buffer = ByteArray(256)
        val pressedKeys = ConcurrentHashMap.newKeySet<Int>()

        fun press(jsKey: Int) {
            if (!pressedKeys.add(jsKey)) return
            val (_, vks) = RgssActivity.mapJsKey(jsKey)
            for (vk in vks) {
                buffer[vk] = 0x80.toByte()
            }
        }

        fun release(jsKey: Int) {
            pressedKeys.remove(jsKey)
            val (_, vks) = RgssActivity.mapJsKey(jsKey)
            for (vk in vks) {
                buffer[vk] = 0x00.toByte()
            }
            // Retain any keys that are still pressed
            for (remaining in pressedKeys) {
                val (_, remainingVks) = RgssActivity.mapJsKey(remaining)
                for (vk in remainingVks) {
                    buffer[vk] = 0x80.toByte()
                }
            }
        }

        // Press 90 (Action: 0x5A, 0x0D, 0x20)
        press(90)
        assertEquals(0x80.toByte(), buffer[0x0D])
        assertEquals(0x80.toByte(), buffer[0x5A])

        // Press 13 (Enter: 0x0D)
        press(13)
        assertEquals(0x80.toByte(), buffer[0x0D])

        // Release 90 -> 0x5A is 0x00, but 0x0D is still 0x80 because 13 is held
        release(90)
        assertEquals(0x00.toByte(), buffer[0x5A])
        assertEquals(0x00.toByte(), buffer[0x20])
        assertEquals(0x80.toByte(), buffer[0x0D])

        // Release 13 -> now 0x0D is 0x00
        release(13)
        assertEquals(0x00.toByte(), buffer[0x0D])
    }

    private class TestInputRgssActivity(private val workingDir: File) : RgssActivity() {
        override fun getInputStateDirectory(): File = workingDir
        override fun dispatchNativeKeyDown(androidKey: Int) {}
        override fun dispatchNativeKeyUp(androidKey: Int) {}
    }

    @Test
    fun testAltAndDirectionCoexistInVirtualKeyBuffer() {
        val targetDir = tempFolder.newFolder("rp_alt_dpad_test")
        val activity = TestInputRgssActivity(targetDir)
        val stateFile = File(targetDir, RgssActivity.INPUT_STATE_FILE)

        // 1. Press ALT (JS 18 -> VK 0x12)
        activity.onKeyDown(18)
        assertEquals(0x80.toByte(), activity.virtualKeyBuffer[0x12])
        assertTrue(stateFile.exists())
        assertEquals(0x80.toByte(), stateFile.readBytes()[0x12])

        // 2. Press UP (JS 38 -> VK 0x26) while holding ALT
        activity.onKeyDown(38)
        assertEquals(0x80.toByte(), activity.virtualKeyBuffer[0x12])
        assertEquals(0x80.toByte(), activity.virtualKeyBuffer[0x26])
        val bytesBoth = stateFile.readBytes()
        assertEquals(0x80.toByte(), bytesBoth[0x12])
        assertEquals(0x80.toByte(), bytesBoth[0x26])

        // 3. Release UP (JS 38) -> UP is cleared, ALT remains pressed (0x80)
        activity.onKeyUp(38)
        assertEquals(0x80.toByte(), activity.virtualKeyBuffer[0x12])
        assertEquals(0x00.toByte(), activity.virtualKeyBuffer[0x26])
        val bytesUpReleased = stateFile.readBytes()
        assertEquals(0x80.toByte(), bytesUpReleased[0x12])
        assertEquals(0x00.toByte(), bytesUpReleased[0x26])

        // 4. Release ALT (JS 18) -> ALT is finally cleared (0x00)
        activity.onKeyUp(18)
        assertEquals(0x00.toByte(), activity.virtualKeyBuffer[0x12])
        assertEquals(0x00.toByte(), activity.virtualKeyBuffer[0x26])
        val bytesAllReleased = stateFile.readBytes()
        assertEquals(0x00.toByte(), bytesAllReleased[0x12])
        assertEquals(0x00.toByte(), bytesAllReleased[0x26])
    }

    @Test
    fun testMapAndroidKeyToVkFullCoverage() {
        assertEquals(0x41, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_A))
        assertEquals(0x5A, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_Z))
        assertEquals(0x30, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_0))
        assertEquals(0x39, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_9))
        assertEquals(0x60, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_NUMPAD_0))
        assertEquals(0x69, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_NUMPAD_9))
        assertEquals(0x0D, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_ENTER))
        assertEquals(0x20, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_SPACE))
        assertEquals(0x1B, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_ESCAPE))
        assertEquals(0x10, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_SHIFT_LEFT))
        assertEquals(0x11, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_CTRL_LEFT))
        assertEquals(0x12, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_ALT_LEFT))
        assertEquals(0x09, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_TAB))
        assertEquals(0x08, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_DEL))
        assertEquals(0x2E, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_FORWARD_DEL))
        assertEquals(0x70, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_F1))
        assertEquals(0x7B, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_F12))
        assertEquals(0xBA, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_SEMICOLON))
        assertEquals(0xBF, RgssActivity.mapAndroidKeyToVk(KeyEvent.KEYCODE_SLASH))
    }

    @Test
    fun testExpandedJsKeyMapping() {
        // Numpad 5: 101 -> VK 0x65
        val (numAndroid, numVk) = RgssActivity.mapJsKey(101)
        assertEquals(listOf(KeyEvent.KEYCODE_NUMPAD_5), numAndroid)
        assertEquals(listOf(0x65), numVk)

        // Semicolon: 186 -> VK 0xBA
        val (semiAndroid, semiVk) = RgssActivity.mapJsKey(186)
        assertEquals(listOf(KeyEvent.KEYCODE_SEMICOLON), semiAndroid)
        assertEquals(listOf(0xBA), semiVk)

        // PageDown: 34 -> VK 0x22
        val (pgAndroid, pgVk) = RgssActivity.mapJsKey(34)
        assertEquals(listOf(KeyEvent.KEYCODE_PAGE_DOWN), pgAndroid)
        assertEquals(listOf(0x22), pgVk)
    }

    @Test
    fun testWebActivityAndroidKeyToJsKeyCode() {
        assertEquals(65, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_A))
        assertEquals(90, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_Z))
        assertEquals(48, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_0))
        assertEquals(57, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_9))
        assertEquals(96, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_NUMPAD_0))
        assertEquals(105, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_NUMPAD_9))
        assertEquals(112, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_F1))
        assertEquals(123, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_F12))
        assertEquals(38, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_UP))
        assertEquals(40, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_DOWN))
        assertEquals(37, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_LEFT))
        assertEquals(39, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_RIGHT))
        assertEquals(13, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_ENTER))
        assertEquals(13, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_CENTER))
        assertEquals(27, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_ESCAPE))
        assertEquals(27, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_BACK))
        assertEquals(32, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_SPACE))
        assertEquals(8, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DEL))
        assertEquals(9, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_TAB))
        assertEquals(16, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_SHIFT_LEFT))
        assertEquals(17, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_CTRL_LEFT))
        assertEquals(18, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_ALT_LEFT))
        assertEquals(33, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_PAGE_UP))
        assertEquals(34, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_PAGE_DOWN))
        assertEquals(36, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_MOVE_HOME))
        assertEquals(35, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_MOVE_END))
    }

    @Test
    fun testWebShimsScriptInitialState() {
        val activity = WebActivity()
        assertEquals("", activity.webShimsScript)
        activity.webShimsScript = "test script"
        assertEquals("test script", activity.webShimsScript)
    }
}
