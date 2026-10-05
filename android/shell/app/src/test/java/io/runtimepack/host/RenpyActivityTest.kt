package io.runtimepack.host

import android.app.Activity
import android.content.Context
import io.runtimepack.host.WebActivity.Companion.ControlHitbox
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.libsdl.app.SDLActivity
import org.renpy.android.PythonSDLActivity
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
    fun runtimeEnvConfiguresIsolatedPaths() {
        val files = tempFolder.newFolder("files")
        val env = RenpyActivity.runtimeEnv(files, "/data/app/base.apk")

        assertEquals(RenpyActivity.gameRoot(files).absolutePath, env["ANDROID_PRIVATE"])
        assertEquals(RenpyActivity.publicDir(files).absolutePath, env["ANDROID_PUBLIC"])
        assertEquals(RenpyActivity.legacyPublicDir(files).absolutePath, env["ANDROID_OLD_PUBLIC"])
        assertEquals("/data/app/base.apk", env["ANDROID_APK"])
        assertEquals(RenpyActivity.gameRoot(files).absolutePath, env["ANDROID_ARGUMENT"])
        assertEquals(RenpyActivity.saveDir(files).absolutePath, env["RENPY_PATH_TO_SAVES"])
        assertEquals(6, env.size)
    }

    @Test
    fun androidPublicIsIsolatedSubfolderNotFilesDirRoot() {
        // Ren'Py appends os.path.join(ANDROID_PUBLIC, "game") to its searchpath.
        // If ANDROID_PUBLIC was filesDir, filesDir/game would be added a second
        // time (it is already in searchpath via ANDROID_PRIVATE), causing every
        // script to load twice and crashing store.build.renpy.
        val files = tempFolder.newFolder("files_iso")
        val env = RenpyActivity.runtimeEnv(files, "/data/app/base.apk")
        val androidPublic = env["ANDROID_PUBLIC"]!!

        org.junit.Assert.assertNotEquals(files.absolutePath, androidPublic)
        assertEquals(File(files, "public").absolutePath, androidPublic)
        assertTrue(androidPublic.startsWith(files.absolutePath + File.separator))
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

    // --- PyJNIus PythonSDLActivity bridge (Blocker 3) --------------------

    @Test
    fun renpyActivityExtendsPythonSDLActivity() {
        assertTrue(
            "RenpyActivity must extend PythonSDLActivity for pyjnius compatibility",
            PythonSDLActivity::class.java.isAssignableFrom(RenpyActivity::class.java)
        )
        assertTrue(
            "PythonSDLActivity must extend SDLActivity",
            SDLActivity::class.java.isAssignableFrom(PythonSDLActivity::class.java)
        )
    }

    @Test
    fun pythonSDLActivityClassIsLoadableAndHasExpectedMembers() {
        // Ren'Py Python code executes: autoclass("org.renpy.android.PythonSDLActivity").mActivity
        val cls = Class.forName("org.renpy.android.PythonSDLActivity")
        assertNotNull(cls)

        val mActivityField = cls.getField("mActivity")
        assertTrue(java.lang.reflect.Modifier.isStatic(mActivityField.modifiers))
        assertTrue(java.lang.reflect.Modifier.isPublic(mActivityField.modifiers))

        // Bridge methods called by lib/python3.9/android/__init__.py and renpy/main.py
        assertNotNull(cls.getMethod("vibrate", Double::class.javaPrimitiveType))
        assertNotNull(cls.getMethod("getDPI"))
        assertNotNull(cls.getMethod("openUrl", String::class.java))
        assertNotNull(cls.getMethod("setWakeLock", Boolean::class.javaPrimitiveType))
        assertNotNull(cls.getMethod("isChromebook"))
        assertNotNull(cls.getMethod("toast", String::class.java))
        assertNotNull(cls.getMethod("toastError", String::class.java))
        assertNotNull(cls.getMethod("hidePresplash"))
        assertNotNull(
            cls.getDeclaredMethod(
                "nativeSetEnv",
                String::class.java,
                String::class.java
            )
        )
    }

    @Test
    fun renpyAndroidPackageClassesExist() {
        assertNotNull(Class.forName("org.renpy.android.Constants"))
        assertNotNull(Class.forName("org.renpy.android.StoreInterface"))
        assertNotNull(Class.forName("org.renpy.android.ResourceManager"))
        assertNotNull(Class.forName("org.renpy.android.Action"))
    }

    @Test
    fun testShouldForwardTouchToRenpyGameOutsideControls() {
        val hitboxes = listOf(
            ControlHitbox("btn-a", 800f, 400f, 900f, 500f)
        )
        // Touch outside any control -> should forward to Ren'Py game surface
        val forwardOutside = RenpyActivity.shouldForwardTouchToGame(
            x = 200f,
            y = 200f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            controlHitboxes = hitboxes
        )
        assertTrue(forwardOutside)

        // Touch inside btn-a -> should NOT forward to Ren'Py game surface
        val forwardInside = RenpyActivity.shouldForwardTouchToGame(
            x = 850f,
            y = 450f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            controlHitboxes = hitboxes
        )
        assertFalse(forwardInside)

        // When overlay modal is open -> NEVER forward to game
        val forwardModalOpen = RenpyActivity.shouldForwardTouchToGame(
            x = 200f,
            y = 200f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = true,
            controlHitboxes = hitboxes
        )
        assertFalse(forwardModalOpen)
    }

    @Test
    fun testRenpyKeyMappingForGamepadButtons() {
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_UP, RenpyActivity.mapVPadButtonToKeyCode("up"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_DOWN, RenpyActivity.mapVPadButtonToKeyCode("down"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_LEFT, RenpyActivity.mapVPadButtonToKeyCode("left"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_RIGHT, RenpyActivity.mapVPadButtonToKeyCode("right"))
        assertEquals(android.view.KeyEvent.KEYCODE_BUTTON_A, RenpyActivity.mapVPadButtonToKeyCode("a"))
        assertEquals(android.view.KeyEvent.KEYCODE_BUTTON_B, RenpyActivity.mapVPadButtonToKeyCode("b"))
        assertEquals(android.view.KeyEvent.KEYCODE_MENU, RenpyActivity.mapVPadButtonToKeyCode("menu"))
        assertEquals(android.view.KeyEvent.KEYCODE_SHIFT_LEFT, RenpyActivity.mapVPadButtonToKeyCode("dash"))
    }
}
