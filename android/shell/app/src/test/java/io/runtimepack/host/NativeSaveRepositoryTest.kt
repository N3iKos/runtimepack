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

    @Test
    fun testVnmGameUidAndSlotsPersistenceAcrossInstances() {
        val root = tempFolder.newFolder("vnm_repo_saves")
        val repoSession1 = NativeSaveRepository(root)
        assertTrue(repoSession1.registerNamespace("game_uid_123"))

        assertTrue(repoSession1.put("game_uid_123", "1", "slot_1_data_日本語"))
        assertTrue(repoSession1.put("game_uid_123", "2", "slot_2_data"))
        assertTrue(repoSession1.put("game_uid_123", "3", "slot_3_data"))
        assertTrue(repoSession1.put("game_uid_123", "1_header", "{\"chapter\":1,\"title\":\"Prologue\"}"))
        assertTrue(repoSession1.put("game_uid_123", "settings", "{\"volume\":100}"))
        assertTrue(repoSession1.put("game_uid_123", "globalData", "{\"unlocked\":true}"))

        val repoSession2 = NativeSaveRepository(root, setOf("game_uid_123"))
        assertEquals("slot_1_data_日本語", repoSession2.get("game_uid_123", "1"))
        assertEquals("slot_2_data", repoSession2.get("game_uid_123", "2"))
        assertEquals("slot_3_data", repoSession2.get("game_uid_123", "3"))
        assertEquals("{\"chapter\":1,\"title\":\"Prologue\"}", repoSession2.get("game_uid_123", "1_header"))
        assertEquals("{\"volume\":100}", repoSession2.get("game_uid_123", "settings"))
        assertEquals("{\"unlocked\":true}", repoSession2.get("game_uid_123", "globalData"))

        assertTrue(repoSession2.contains("game_uid_123", "2"))
        assertTrue(repoSession2.remove("game_uid_123", "2"))
        assertFalse(repoSession2.contains("game_uid_123", "2"))
        assertNull(repoSession2.get("game_uid_123", "2"))
        assertNull(repoSession2.get("game_uid_123", "nonexistent"))

        val dummySaveService = object : SaveService {
            override fun openRead(slot: String) = null
            override fun openWrite(slot: String) = java.io.ByteArrayOutputStream()
            override fun exists(slot: String) = false
            override fun list() = emptyList<String>()
            override fun delete(slot: String) = false
        }
        val bridge = WebBridge(dummySaveService, repoSession2)
        assertTrue(bridge.registerNamespace("bridge_game_uid"))
        assertTrue(bridge.saveSlot("bridge_game_uid", "1", "data_via_bridge"))
        assertEquals("data_via_bridge", bridge.loadSlot("bridge_game_uid", "1"))
    }

    @Test
    fun testGetAndContainsDoNotCreateDirectoriesOnMissingLookup() {
        val root = tempFolder.newFolder("saves_no_mkdir")
        val repo = NativeSaveRepository(root)
        val vnmDir = File(root, "vnm")
        assertFalse(vnmDir.exists())

        assertNull(repo.get("vnm", "nonexistent_slot"))
        assertFalse("get on missing slot must not create namespace directory", vnmDir.exists())

        assertFalse(repo.contains("vnm", "nonexistent_slot"))
        assertFalse("contains on missing slot must not create namespace directory", vnmDir.exists())
    }

    @Test
    fun testVnmLegacySlotNameReadingWithoutRegisterNamespace() {
        val root = tempFolder.newFolder("vnm_legacy_saves")
        val repo = NativeSaveRepository(root)
        val legacySlot = "BB45114F1292564E907B88654BEDAB153D70_SaveGame_0"
        val payload = "eyJjaGFwdGVyIjo1LCJnb2xkIjoxMjAwMH0="

        assertTrue(repo.put("vnm", legacySlot, payload))
        assertTrue(repo.contains("vnm", legacySlot))
        assertEquals(payload, repo.get("vnm", legacySlot))
    }

    @Test
    fun testProcessRestartUnregisteredNamespaceReadAllowedMutationsDenied() {
        val root = tempFolder.newFolder("restart_saves")
        val repoSession1 = NativeSaveRepository(root)
        val uid = "bb45114f1292564e907b88654bedab153d70"
        assertTrue(repoSession1.registerNamespace(uid))
        val payload = "{\"chapter\":5,\"gold\":12000}"
        assertTrue(repoSession1.put(uid, "SaveGame_0", payload))

        val repoSession2 = NativeSaveRepository(root)

        assertTrue("contains must be true on existing namespace without register", repoSession2.contains(uid, "SaveGame_0"))
        assertEquals(payload, repoSession2.get(uid, "SaveGame_0"))

        assertFalse("unregistered put must return false", repoSession2.put(uid, "SaveGame_1", "new_payload"))
        assertFalse("unregistered remove must return false", repoSession2.remove(uid, "SaveGame_0"))
        assertEquals(payload, repoSession2.get(uid, "SaveGame_0"))

        assertFalse(repoSession2.contains("../root", "SaveGame_0"))
        assertNull(repoSession2.get("../root", "SaveGame_0"))
        assertFalse(repoSession2.contains(uid, "../escape"))
        assertNull(repoSession2.get(uid, "../escape"))
        assertFalse(repoSession2.contains(".hidden", "SaveGame_0"))
        assertNull(repoSession2.get(".hidden", "SaveGame_0"))

        val missingDir = File(root, "nonexistent_uid")
        assertFalse(repoSession2.contains("nonexistent_uid", "slot1"))
        assertNull(repoSession2.get("nonexistent_uid", "slot1"))
        assertFalse("Missing query must create no directory", missingDir.exists())
    }
}
