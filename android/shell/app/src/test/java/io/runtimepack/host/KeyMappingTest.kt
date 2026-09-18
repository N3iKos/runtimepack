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
            listOf(KeyEvent.KEYCODE_Z, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_SPACE),
            androidKeys
        )
        assertEquals(
            listOf(0x5A, 0x0D, 0x20),
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
        val (_, vkKeys) = RgssActivity.mapJsKey(90) // VK: 0x5A, 0x0D, 0x20

        // Key Down: set mapped VK to 0x80
        for (vk in vkKeys) {
            buffer[vk] = 0x80.toByte()
        }
        assertEquals(0x80.toByte(), buffer[0x5A])
        assertEquals(0x80.toByte(), buffer[0x0D])
        assertEquals(0x80.toByte(), buffer[0x20])
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
}
