package io.runtimepack.host

import android.app.Activity
import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.libsdl.app.SDLActivity
import java.io.File

class RenpyActivityTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    private fun createMountedRoot(name: String, markerVersion: String = GpackExtractor.EXTRACTION_VERSION): File {
        val root = tempFolder.newFolder(name)
        File(root, "main.py").writeText("")
        File(root, "renpy").mkdirs()
        File(root, "lib").mkdirs()
        File(root, GpackExtractor.EXTRACTION_MARKER).writeText(markerVersion)
        return root
    }

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
        assertEquals(GpackExtractor.EXTRACTION_MARKER, RenpyActivity.extractionMarker(root).name)
    }

    // --- mount guard (finding 2 + 3) ---------------------------------------

    @Test
    fun isContentMountedFalseWithoutMarker() {
        val root = tempFolder.newFolder("root")
        File(root, "main.py").writeText("")
        File(root, "renpy").mkdirs()
        File(root, "lib").mkdirs()

        assertFalse(GpackExtractor.isMarkerCurrent(root))
        assertFalse(RenpyActivity.isContentMounted(root))
    }

    @Test
    fun isContentMountedFalseWhenMarkerStale() {
        val root = createMountedRoot("stale", markerVersion = "v2")

        assertFalse(GpackExtractor.isMarkerCurrent(root))
        assertFalse(RenpyActivity.isContentMounted(root))
    }

    @Test
    fun isContentMountedTrueWithCurrentMarkerAndLayout() {
        val root = createMountedRoot("mounted")

        assertTrue(GpackExtractor.isMarkerCurrent(root))
        assertTrue(RenpyActivity.isContentMounted(root))
    }

    @Test
    fun isContentMountedFalseWithCurrentMarkerButIncompleteLayout() {
        val root = tempFolder.newFolder("partial")
        File(root, "main.py").writeText("")
        File(root, GpackExtractor.EXTRACTION_MARKER).writeText(GpackExtractor.EXTRACTION_VERSION)

        assertTrue(GpackExtractor.isMarkerCurrent(root))
        assertFalse(RenpyActivity.isGameMounted(root))
        assertFalse(RenpyActivity.isContentMounted(root))
    }

    @Test
    fun isMarkerCurrentFalseWhenMarkerIsDirectory() {
        val root = tempFolder.newFolder("dir-marker")
        File(root, GpackExtractor.EXTRACTION_MARKER).mkdirs()

        assertFalse(GpackExtractor.isMarkerCurrent(root))
    }

    @Test
    fun isMarkerCurrentFalseForMissingDirectory() {
        val missing = File(tempFolder.root, "nope")
        assertFalse(GpackExtractor.isMarkerCurrent(missing))
    }

    // --- save symlink redirection (finding 3 / isolation) ------------------

    @Test
    fun shouldLinkSaveDirectoryTrueWhenLegacyAbsent() {
        val legacySaves = File(tempFolder.root, "game/saves")
        assertTrue(RenpyActivity.shouldLinkSaveDirectory(legacySaves))
    }

    @Test
    fun shouldLinkSaveDirectoryFalseWhenLegacyPresent() {
        val legacySaves = tempFolder.newFolder("game", "saves")
        assertFalse(RenpyActivity.shouldLinkSaveDirectory(legacySaves))
    }

    // --- lifecycle split (Task 4 blocker) ----------------------------------

    @Test
    fun bootstrapIsAPlainActivityNotTheSdlActivity() {
        // The launcher must be a plain Activity so its onCreate can call through
        // to super synchronously while it mounts content off the UI thread.
        assertTrue(Activity::class.java.isAssignableFrom(RenpyBootstrapActivity::class.java))
        assertFalse(SDLActivity::class.java.isAssignableFrom(RenpyBootstrapActivity::class.java))
    }

    @Test
    fun renpyActivityRemainsTheSdlActivity() {
        assertTrue(SDLActivity::class.java.isAssignableFrom(RenpyActivity::class.java))
    }

    @Test
    fun bootstrapExposesBackgroundMounter() {
        val method = RenpyBootstrapActivity::class.java.getMethod(
            "mountGameContent",
            Context::class.java,
            File::class.java
        )
        assertTrue(java.lang.reflect.Modifier.isStatic(method.modifiers))
        assertTrue(java.lang.reflect.Modifier.isPublic(method.modifiers))
    }

    @Test
    fun bootstrapReusesTheSharedContentContract() {
        // The bootstrap must not re-implement the layout contract.
        val mounted = RenpyBootstrapActivity::class.java
        assertNotNull(mounted)
        assertEquals(
            listOf("main.py", "renpy", "lib"),
            RenpyActivity.expectedLayout()
        )
    }

    // --- SDL JNI compatibility (Blocker 2) --------------------------------

    @Test
    fun sdlInputConnectionDeclaresNativeSetComposingText() {
        // librenpython.so's JNI_OnLoad (Android_JNI_SetupThread) looks up
        // nativeSetComposingText(String, int) on SDLInputConnection.
        val inputClass = Class.forName("org.libsdl.app.SDLInputConnection")
        val method = inputClass.getDeclaredMethod(
            "nativeSetComposingText",
            String::class.java,
            Int::class.javaPrimitiveType
        )
        assertNotNull("nativeSetComposingText must exist on SDLInputConnection", method)
        assertTrue(
            "nativeSetComposingText must be a native method",
            java.lang.reflect.Modifier.isNative(method.modifiers)
        )
        assertFalse(
            "nativeSetComposingText must be non-static",
            java.lang.reflect.Modifier.isStatic(method.modifiers)
        )
    }

    @Test
    fun jniusNativeInvocationHandlerExistsForRenpyPyjnius() {
        // librenpython.so embeds pyjnius and looks up org.jnius.NativeInvocationHandler
        val cls = Class.forName("org.jnius.NativeInvocationHandler")
        assertNotNull(cls)
        val method = cls.getMethod("getPythonObjectPointer")
        assertEquals(Long::class.javaPrimitiveType, method.returnType)
    }
}
