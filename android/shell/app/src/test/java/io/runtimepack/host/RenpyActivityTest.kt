package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class RenpyActivityTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun gameRootResolvesToGameSubdirectory() {
        val files = tempFolder.newFolder("files")
        assertEquals(File(files, "game"), RenpyActivity.gameRoot(files))
        assertEquals("game", RenpyActivity.gameRoot(files).name)
    }

    @Test
    fun saveDirIsSiblingOfGameRoot() {
        val files = tempFolder.newFolder("files")
        val saves = RenpyActivity.saveDir(files)
        val game = RenpyActivity.gameRoot(files)

        assertEquals(File(files, "saves"), saves)
        assertEquals(files, saves.parentFile)
        assertFalse(saves.absolutePath.startsWith(game.absolutePath + File.separator))
    }

    @Test
    fun runtimeEnvPointsPrivateAtGameRootAndPublicAtFilesDir() {
        val files = tempFolder.newFolder("files")
        val env = RenpyActivity.runtimeEnv(files, "/data/app/base.apk")

        assertEquals(RenpyActivity.gameRoot(files).absolutePath, env["ANDROID_PRIVATE"])
        assertEquals(files.absolutePath, env["ANDROID_PUBLIC"])
        assertEquals(RenpyActivity.legacyPublicDir(files).absolutePath, env["ANDROID_OLD_PUBLIC"])
        assertEquals("/data/app/base.apk", env["ANDROID_APK"])
        assertEquals(4, env.size)
    }

    @Test
    fun missingLayoutEntriesReportsAllWhenEmpty() {
        val root = tempFolder.newFolder("root")

        assertEquals(listOf("main.py", "renpy", "lib"), RenpyActivity.missingLayoutEntries(root))
        assertFalse(RenpyActivity.isGameMounted(root))
    }

    @Test
    fun isGameMountedTrueWhenEngineLayoutPresent() {
        val root = tempFolder.newFolder("root")
        File(root, "main.py").writeText("")
        File(root, "renpy").mkdirs()
        File(root, "lib").mkdirs()

        assertTrue(RenpyActivity.isGameMounted(root))
        assertTrue(RenpyActivity.missingLayoutEntries(root).isEmpty())
    }

    @Test
    fun missingLayoutEntriesReportsOnlyMissing() {
        val root = tempFolder.newFolder("root")
        File(root, "main.py").writeText("")

        assertEquals(listOf("renpy", "lib"), RenpyActivity.missingLayoutEntries(root))
    }

    @Test
    fun archivesInFindsRpaFilesOnly() {
        val root = tempFolder.newFolder("root")
        File(root, "images.rpa").writeText("")
        File(root, "audio.rpa").writeText("")
        File(root, "script.rpyc").writeText("")

        assertEquals(listOf("audio.rpa", "images.rpa"), RenpyActivity.archivesIn(root).map { it.name })
    }

    @Test
    fun archivesInReturnsEmptyForMissingDirectory() {
        val missing = File(tempFolder.root, "does-not-exist")
        assertTrue(RenpyActivity.archivesIn(missing).isEmpty())
    }

    @Test
    fun extractionMarkerLocatedAtGameRoot() {
        val root = tempFolder.newFolder("root")
        assertEquals(File(root, ".gpack_extracted"), RenpyActivity.extractionMarker(root))
    }
}
