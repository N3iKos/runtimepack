package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class NativeSaveRepositoryTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testNamespaceAndPersistenceRoundTrip() {
        val root = tempFolder.newFolder("saves")
        val repo = NativeSaveRepository(root)

        assertTrue("put mz slot should succeed", repo.put("mz", "file1", "payload_mz_1"))
        assertTrue("put mv slot should succeed", repo.put("mv", "global", "payload_mv_global"))

        assertEquals("payload_mz_1", repo.get("mz", "file1"))
        assertEquals("payload_mv_global", repo.get("mv", "global"))

        assertTrue(repo.contains("mz", "file1"))
        assertTrue(repo.contains("mv", "global"))

        assertEquals(listOf("file1"), repo.keys("mz"))
        assertEquals(listOf("global"), repo.keys("mv"))

        assertTrue(repo.remove("mz", "file1"))
        assertFalse(repo.contains("mz", "file1"))
        assertNull(repo.get("mz", "file1"))
        assertTrue(repo.keys("mz").isEmpty())

        // Ensure MV slot was untouched by MZ operations
        assertEquals("payload_mv_global", repo.get("mv", "global"))
    }

    @Test
    fun testSlotValidationAndSecurity() {
        val root = tempFolder.newFolder("saves_sec")
        val repo = NativeSaveRepository(root)

        // Invalid namespaces
        assertFalse(repo.put("", "file1", "data"))
        assertFalse(repo.put("unknown", "file1", "data"))

        // Path traversal attempts
        assertFalse(repo.put("mz", "../escape", "data"))
        assertFalse(repo.put("mz", "sub/dir", "data"))
        assertFalse(repo.put("mz", "dir\\file", "data"))
        assertFalse(repo.put("mz", "", "data"))
        assertFalse(repo.put("mz", ".hidden", "data"))

        // Valid slot characters: letters, digits, underscore, dash, dot
        assertTrue(repo.put("mz", "file_1-save.bak", "valid_data"))
        assertEquals("valid_data", repo.get("mz", "file_1-save.bak"))
    }

    @Test
    fun testAtomicOverwriteLeavesPreviousIntactOnFailure() {
        val root = tempFolder.newFolder("saves_atomic")
        val repo = NativeSaveRepository(root)

        assertTrue(repo.put("mz", "file1", "initial_data"))
        assertEquals("initial_data", repo.get("mz", "file1"))

        assertTrue(repo.put("mz", "file1", "updated_data"))
        assertEquals("updated_data", repo.get("mz", "file1"))
    }

    @Test
    fun testWebBridgeSlotMethods() {
        val root = tempFolder.newFolder("bridge_saves")
        val repo = NativeSaveRepository(root)
        val dummySaveService = object : SaveService {
            override fun openRead(slot: String) = null
            override fun openWrite(slot: String) = java.io.ByteArrayOutputStream()
            override fun exists(slot: String) = false
            override fun list() = emptyList<String>()
            override fun delete(slot: String) = false
        }
        val bridge = WebBridge(dummySaveService, repo)

        assertTrue(bridge.saveSlot("mz", "global", "{\"title\":\"Forestia\"}"))
        assertTrue(bridge.slotExists("mz", "global"))
        assertFalse(bridge.slotExists("mz", "missing"))

        assertEquals("{\"title\":\"Forestia\"}", bridge.loadSlot("mz", "global"))
        assertNull(bridge.loadSlot("mz", "missing"))

        val listJson = bridge.listSlots("mz")
        assertTrue(listJson.contains("global"))
        assertEquals("[]", bridge.listSlots("mv"))

        assertTrue(bridge.deleteSlot("mz", "global"))
        assertFalse(bridge.slotExists("mz", "global"))
    }
}
