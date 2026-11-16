package io.runtimepack.host

import android.app.Activity
import android.content.Context
import android.view.MotionEvent
import io.runtimepack.host.WebActivity.Companion.ControlHitbox
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.libsdl.app.SDLActivity
import org.renpy.android.PythonSDLActivity
import java.io.File
import java.nio.file.Files

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

    // --- Android common-dir discovery (TeacherVN g02: Ren'Py 7.5.3) --------
    //
    // Ren'Py 7.x/8.0.x replace config.searchpath on Android and drop
    // config.commondir (renpy/main.py android_searchpath()). The extracted
    // gameRoot/renpy/common is therefore never searched and
    // load_module("_errorhandling") fails. Ren'Py 8.1+ builds the Android
    // search path from the entrypoint's predefined_searchpath(commondir),
    // which keeps the common dir. The snippets below are the relevant parts of
    // the entrypoints shipped in the g02 (7.5.3) and g01 (8.2.1) packs; the
    // helpers mirror the two upstream search-path algorithms so the tests
    // assert the resulting search path, not just the presence of an env var.
    // Provenance: g02 entrypoint sha256 6af25cf47b94af38a4d4d1ea20e48851b2a8f9e557455a3eab439c5e588831bf,
    // g01 entrypoint sha256 5b6b977565179d1f2b49ea3dc6e5433121f72be8e35202e8c80d68455532d7f9.

    private val legacyEntrypoint = """
android = ("ANDROID_PRIVATE" in os.environ)

if android:
    __main__ = sys.modules["__main__"]
    __main__.path_to_gamedir = path_to_gamedir
    __main__.path_to_renpy_base = path_to_renpy_base
    __main__.path_to_common = path_to_common
    __main__.path_to_saves = path_to_saves


def main():
    renpy_base = path_to_renpy_base()
    sys.path.append(renpy_base)
    import renpy.bootstrap
    renpy.bootstrap.bootstrap(renpy_base)
""".trimIndent()

    private val modernEntrypoint = """
def predefined_searchpath(commondir):
    import renpy

    searchpath = [ renpy.config.gamedir ]

    packs = [ "ANDROID_PACK_FF1", "ANDROID_PACK_FF2", "ANDROID_PACK_FF3", "ANDROID_PACK_FF4" ]

    for i in packs:
        if i not in os.environ:
            continue
        assets = os.environ[i]
        for i in [ "renpy/common", "game" ]:
            dn = os.path.join(assets, i)
            if os.path.isdir(dn):
                searchpath.append(dn)

    if commondir and os.path.isdir(commondir):
        searchpath.append(commondir)

    return searchpath


def main():
    renpy_base = path_to_renpy_base()
    import renpy.bootstrap
    renpy.__main__ = sys.modules[__name__]
    renpy.bootstrap.bootstrap(renpy_base)
""".trimIndent()

    // Engine modules (not entrypoints) are what actually build the Android
    // search path, so the host probes them first. Excerpts from the shipped
    // packs: g02 renpy/main.py sha256
    // 016447469c6f198e1f3f71b5fe40ca127b75e046f6be22e1d8bf933f04dc8c12
    // (def android_searchpath at line 303, reset at 374, call at 392);
    // g01 renpy/main.py sha256
    // e761c1fa90e9a9ac34f1f90e5c1138f5b22cdcde8c4ecbab8890a83a568fe3b8
    // (predefined_searchpath call at line 351).
    private val legacyEngineModule = """
def android_searchpath():
    renpy.config.searchpath = [ renpy.config.gamedir ]


def main():
    renpy.config.searchpath = [ renpy.config.gamedir ]
    if renpy.android:
        renpy.config.commondir = None
        android_searchpath()
""".trimIndent()

    private val modernEngineModule = """
def main():
    renpy.config.commondir = renpy.__main__.path_to_common(renpy.config.renpy_base)
    renpy.config.searchpath = renpy.__main__.predefined_searchpath(renpy.config.commondir)
""".trimIndent()

    private fun createEngineRoot(
        files: File,
        entrypointSource: String = legacyEntrypoint,
        engineModuleSource: String? = legacyEngineModule
    ): File {
        val root = RenpyActivity.gameRoot(files)
        root.mkdirs()
        File(root, "main.py").writeText(entrypointSource)
        File(root, "renpy/common").mkdirs()
        File(root, "renpy/common/_errorhandling.rpym").writeText("")
        if (engineModuleSource != null) {
            File(root, "renpy/main.py").writeText(engineModuleSource)
        }
        File(root, "lib").mkdirs()
        File(root, "game").mkdirs()
        File(root, "game/script.rpyc").writeText("")
        File(root, GpackExtractor.EXTRACTION_MARKER).writeText(GpackExtractor.EXTRACTION_VERSION)
        return root
    }

    private fun assetPackEntries(env: Map<String, String>): List<File> {
        val entries = mutableListOf<File>()
        listOf("ANDROID_PACK_FF1", "ANDROID_PACK_FF2", "ANDROID_PACK_FF3", "ANDROID_PACK_FF4").forEach { key ->
            val assets = env[key] ?: return@forEach
            listOf("renpy/common", "game").forEach { rel ->
                val dn = File(assets, rel)
                if (dn.isDirectory) entries.add(dn)
            }
        }
        return entries
    }

    /** Faithful port of renpy/main.py android_searchpath() (Ren'Py 7.x, 8.0.x). */
    private fun legacyAndroidSearchPath(gameDir: File, env: Map<String, String>): List<String> {
        val searchpath = mutableListOf(gameDir.absolutePath)
        val androidPublic = env["ANDROID_PUBLIC"]
        if (androidPublic != null) {
            val androidGame = File(androidPublic, "game")
            if (androidGame.exists()) searchpath.add(0, androidGame.absolutePath)
        }
        searchpath.addAll(assetPackEntries(env).map { it.absolutePath })
        return searchpath
    }

    /** Faithful port of main.py predefined_searchpath() (Ren'Py 8.1+). */
    private fun modernAndroidSearchPath(
        gameDir: File,
        commondir: File?,
        env: Map<String, String>
    ): List<String> {
        val searchpath = mutableListOf(gameDir.absolutePath)
        searchpath.addAll(assetPackEntries(env).map { it.absolutePath })
        if (commondir != null && commondir.isDirectory) searchpath.add(commondir.absolutePath)
        return searchpath
    }

    private object TestLinkOps : CommonLinkOps {
        override fun isSymlink(link: File): Boolean = Files.isSymbolicLink(link.toPath())

        override fun readLink(link: File): String? = try {
            Files.readSymbolicLink(link.toPath()).toString()
        } catch (e: Exception) {
            null
        }

        override fun move(from: File, to: File): Boolean = try {
            Files.move(from.toPath(), to.toPath())
            true
        } catch (e: Exception) {
            false
        }

        override fun symlink(target: String, link: String) {
            Files.createSymbolicLink(File(link).toPath(), File(target).toPath())
        }
    }

    private fun preparePack(files: File, root: File): Boolean =
        RenpyActivity.prepareCommonSearchPath(files, root, TestLinkOps)

    private fun staleSiblings(files: File): List<File> =
        RenpyActivity.assetPackRoot(files).listFiles()
            ?.filter { it.name.startsWith("${RenpyActivity.ENGINE_DIR_NAME}.stale") }
            ?: emptyList()

    // --- preparePython wiring (M3): the pure merge must carry the legacy env ---

    @Test
    fun pythonEnvAddsTheLegacyAssetPackToTheBaseEnvironment() {
        val files = tempFolder.newFolder("files_env_legacy")
        val root = createEngineRoot(files)
        val base = RenpyActivity.runtimeEnv(files, "/data/app/base.apk")
        val env = RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)

        assertEquals(7, env.size)
        assertEquals(base.size + 1, env.size)
        assertEquals(RenpyActivity.assetPackRoot(files).absolutePath, env["ANDROID_PACK_FF1"])
        base.forEach { (name, value) -> assertEquals(value, env[name]) }
    }

    @Test
    fun pythonEnvLeavesTheModernEnvironmentUnchanged() {
        val files = tempFolder.newFolder("files_env_modern")
        val root = createEngineRoot(files, modernEntrypoint, modernEngineModule)
        val env = RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)

        assertEquals(6, env.size)
        assertEquals(RenpyActivity.runtimeEnv(files, "/data/app/base.apk"), env)
        assertFalse(env.containsKey("ANDROID_PACK_FF1"))
    }

    @Test
    fun pythonEnvAddsNothingForAnUnknownWiring() {
        val files = tempFolder.newFolder("files_env_unknown")
        val root = createEngineRoot(files, "def main():\n    pass\n", engineModuleSource = null)

        assertEquals(
            RenpyActivity.runtimeEnv(files, "/data/app/base.apk"),
            RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)
        )
    }

    // --- resulting engine search path (asserted, not env-key counts) --------

    @Test
    fun legacySearchPathKeepsGameDirOnceAndAddsEngineCommon() {
        val files = tempFolder.newFolder("files_legacy")
        val root = createEngineRoot(files)
        // Full production env: ANDROID_PUBLIC is always exported by the host.
        val env = RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)
        assertTrue(env.containsKey("ANDROID_PUBLIC"))
        assertTrue(preparePack(files, root))

        val gameDir = File(root, "game")
        val packRoot = RenpyActivity.assetPackRoot(files)
        val packCommon = File(packRoot, "renpy/common")
        val searchpath = legacyAndroidSearchPath(gameDir, env)

        assertEquals(listOf(gameDir.absolutePath, packCommon.absolutePath), searchpath)
        assertEquals(1, searchpath.count { it == gameDir.absolutePath })
        assertTrue(File(searchpath.last(), "_errorhandling.rpym").isFile)
        assertFalse("asset pack must not re-add the game directory", File(packRoot, "game").exists())
    }

    @Test
    fun legacySearchPathPrependsPublicGameWhenItExists() {
        val files = tempFolder.newFolder("files_legacy_public")
        val root = createEngineRoot(files)
        val publicGame = File(RenpyActivity.publicDir(files), "game")
        publicGame.mkdirs()
        File(publicGame, "script.rpyc").writeText("")
        val env = RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)
        assertTrue(preparePack(files, root))

        val gameDir = File(root, "game")
        val packCommon = File(RenpyActivity.assetPackRoot(files), "renpy/common")

        assertEquals(
            listOf(publicGame.absolutePath, gameDir.absolutePath, packCommon.absolutePath),
            legacyAndroidSearchPath(gameDir, env)
        )
    }

    @Test
    fun modernSearchPathKeepsGameDirAndCommondirOnly() {
        val files = tempFolder.newFolder("files_modern")
        val root = createEngineRoot(files, modernEntrypoint, modernEngineModule)
        val env = RenpyActivity.pythonEnv(files, "/data/app/base.apk", root)
        val gameDir = File(root, "game")
        val commonDir = File(root, "renpy/common")

        assertEquals(
            listOf(gameDir.absolutePath, commonDir.absolutePath),
            modernAndroidSearchPath(gameDir, commonDir, env)
        )
    }

    // --- engine-module probe (M2) ------------------------------------------

    @Test
    fun engineProbePrefersTheEngineModuleOverEntrypointText() {
        val files = tempFolder.newFolder("probe_engine_legacy")
        // Entrypoint looks modern; the engine module still resets the path.
        val root = createEngineRoot(files, modernEntrypoint, legacyEngineModule)

        assertEquals(CommonDirWiring.LEGACY, RenpyActivity.engineCommonWiring(root))
        assertTrue(RenpyActivity.commonSearchPathEnv(files, root).containsKey("ANDROID_PACK_FF1"))
    }

    @Test
    fun engineProbeDetectsAModernEngineModule() {
        val files = tempFolder.newFolder("probe_engine_modern")
        val root = createEngineRoot(files, legacyEntrypoint, modernEngineModule)

        assertEquals(CommonDirWiring.MODERN, RenpyActivity.engineCommonWiring(root))
        assertTrue(RenpyActivity.commonSearchPathEnv(files, root).isEmpty())
    }

    @Test
    fun engineProbeFallsBackToTheEntrypointWhenTheEngineModuleIsMissing() {
        val files = tempFolder.newFolder("probe_fallback")
        val root = createEngineRoot(files, legacyEntrypoint, engineModuleSource = null)

        assertEquals(CommonDirWiring.LEGACY, RenpyActivity.engineCommonWiring(root))
    }

    @Test
    fun engineProbeRefusesAmbiguousEngineModules() {
        val files = tempFolder.newFolder("probe_ambiguous")
        val ambiguous = legacyEngineModule + "\n" + modernEngineModule
        val root = createEngineRoot(files, legacyEntrypoint, ambiguous)

        assertEquals(CommonDirWiring.UNKNOWN, RenpyActivity.engineCommonWiring(root))
        assertTrue(RenpyActivity.commonSearchPathEnv(files, root).isEmpty())
    }

    @Test
    fun engineProbeRefusesOversizedSourcesInsteadOfGuessing() {
        val files = tempFolder.newFolder("probe_oversized")
        val root = createEngineRoot(files, legacyEntrypoint, engineModuleSource = null)
        val huge = StringBuilder("def android_searchpath():\n")
        while (huge.length < RenpyActivity.MAX_COMMON_PROBE_BYTES + 4096) {
            huge.append("# padding padding padding padding\n")
        }
        File(root, "renpy/main.py").writeText(huge.toString())

        assertEquals(CommonDirWiring.UNKNOWN, RenpyActivity.engineCommonWiring(root))
        assertTrue(RenpyActivity.commonSearchPathEnv(files, root).isEmpty())
    }

    // --- pack-root link repair (M1): repair without deleting user data ------

    @Test
    fun prepareRepairsADanglingForeignLinkWithoutDataLoss() {
        val files = tempFolder.newFolder("files_dangling")
        val root = createEngineRoot(files)
        val packRoot = RenpyActivity.assetPackRoot(files)
        packRoot.mkdirs()
        val foreignTarget = File(tempFolder.root, "foreign-engine")
        val link = File(packRoot, RenpyActivity.ENGINE_DIR_NAME)
        Files.createSymbolicLink(link.toPath(), foreignTarget.toPath())
        assertFalse("precondition: foreign target does not exist", link.exists())

        assertTrue(preparePack(files, root))

        assertTrue(File(packRoot, "renpy/common/_errorhandling.rpym").isFile)
        val preserved = staleSiblings(files)
        assertEquals(1, preserved.size)
        assertEquals(
            foreignTarget.absolutePath,
            Files.readSymbolicLink(preserved[0].toPath()).toString()
        )
        assertEquals(
            File(root, RenpyActivity.ENGINE_DIR_NAME).absolutePath,
            Files.readSymbolicLink(link.toPath()).toString()
        )
    }

    @Test
    fun preparePreservesAMismatchedDirectoryInsteadOfDeletingIt() {
        val files = tempFolder.newFolder("files_mismatch")
        val root = createEngineRoot(files)
        val packRoot = RenpyActivity.assetPackRoot(files)
        val stale = File(packRoot, RenpyActivity.ENGINE_DIR_NAME)
        File(stale, "common").mkdirs()
        File(stale, "common/keep.txt").writeText("user data")

        assertTrue(preparePack(files, root))

        val preserved = staleSiblings(files)
        assertEquals(1, preserved.size)
        assertEquals("user data", File(preserved[0], "common/keep.txt").readText())
        assertTrue(File(packRoot, "renpy/common/_errorhandling.rpym").isFile)
    }

    @Test
    fun prepareIsIdempotentAndKeepsAValidLink() {
        val files = tempFolder.newFolder("files_repeat")
        val root = createEngineRoot(files)
        val packRoot = RenpyActivity.assetPackRoot(files)

        assertTrue(preparePack(files, root))
        val link = File(packRoot, RenpyActivity.ENGINE_DIR_NAME)
        val firstTarget = Files.readSymbolicLink(link.toPath()).toString()

        assertTrue(preparePack(files, root))

        assertEquals(firstTarget, Files.readSymbolicLink(link.toPath()).toString())
        assertEquals(0, staleSiblings(files).size)
        assertTrue(File(packRoot, "renpy/common/_errorhandling.rpym").isFile)
    }

    @Test
    fun prepareOnlyTouchesTheManagedPackRoot() {
        val files = tempFolder.newFolder("files_scope")
        val root = createEngineRoot(files)
        val unrelated = File(files, RenpyActivity.ENGINE_DIR_NAME)
        unrelated.mkdirs()
        File(unrelated, "keep.txt").writeText("not ours")

        assertTrue(preparePack(files, root))

        assertEquals("not ours", File(unrelated, "keep.txt").readText())
        assertFalse(Files.isSymbolicLink(unrelated.toPath()))
    }

    @Test
    fun prepareFailsWhenEngineCommonIsMissing() {
        val files = tempFolder.newFolder("files_no_common")
        val root = createEngineRoot(files)
        File(root, "renpy/common").deleteRecursively()

        assertFalse(preparePack(files, root))
        assertFalse(File(RenpyActivity.assetPackRoot(files), "renpy/common").isDirectory)
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

    // --- VPAD touch feedback parity -----------------------------------------

    private class TestableRenpyActivity : RenpyActivity() {
        val feedbackScripts = mutableListOf<String>()
        val runnableQueue = mutableListOf<Runnable>()
        val cancelEvents = mutableListOf<Int>()
        var surfaceDispatchCount = 0
        var surfaceCancelCount = 0
        var now: Long = 1_000L

        override fun evaluateFeedbackScript(script: String) {
            feedbackScripts.add(script)
        }

        override fun postFeedbackRunnable(action: Runnable, delayMs: Long) {
            runnableQueue.add(action)
        }

        override fun removeFeedbackRunnable(action: Runnable) {
            runnableQueue.remove(action)
        }

        override fun currentUptimeMs(): Long = now

        /** Mirrors production: work is queued for the UI thread, never run inline. */
        override fun dispatchUiRunnable(action: Runnable) {
            runnableQueue.add(action)
        }

        override fun dispatchCancelToSurface() {
            surfaceCancelCount++
            // Record the action rather than obtaining a MotionEvent: the host test
            // runtime stubs MotionEvent.obtain, so building one here would NPE.
            cancelEvents.add(MotionEvent.ACTION_CANCEL)
        }

        fun dispatch(action: Int, x: Float, y: Float, density: Float = 2.0f): Boolean =
            handleTouchDispatch(action, x, y, 1920f, 1080f, density) { surfaceDispatchCount++ }

        fun flushUiTasks() {
            val copy = ArrayList(runnableQueue)
            runnableQueue.clear()
            copy.forEach { it.run() }
        }

        public override fun onPause() {
            super.onPause()
        }

        public override fun onDestroy() {
            super.onDestroy()
        }

        fun flushRunnables() {
            val copy = ArrayList(runnableQueue)
            runnableQueue.clear()
            copy.forEach { it.run() }
        }
    }

    @Test
    fun forwardFeedbackSampleSpawnsRippleOnDownAndEndsTrailOnUp() {
        val activity = TestableRenpyActivity()
        activity.forwardFeedbackSample(MotionEvent.ACTION_DOWN, 200f, 300f)
        activity.forwardFeedbackSample(MotionEvent.ACTION_UP, 200f, 300f)

        assertEquals(2, activity.feedbackScripts.size)
        assertTrue(
            "DOWN must spawn a ripple at density-corrected CSS coordinates: ${activity.feedbackScripts[0]}",
            activity.feedbackScripts[0].contains("spawnTouchRipple(200, 300)")
        )
        assertTrue(
            "UP must end the trail: ${activity.feedbackScripts[1]}",
            activity.feedbackScripts[1].contains("endTouchTrail")
        )
    }

    @Test
    fun toolbarGeometryBridgeFieldIsVolatileForCrossThreadVisibility() {
        // Written on the WebView JS bridge thread, read on the UI thread during
        // dispatch: without @Volatile the UI thread may keep a stale list.
        val field = RenpyActivity::class.java.getDeclaredField("activeToolbarHitboxes")
        assertTrue(
            "activeToolbarHitboxes must be @Volatile",
            java.lang.reflect.Modifier.isVolatile(field.modifiers)
        )

        val controlField = RenpyActivity::class.java.getDeclaredField("activeControlHitboxes")
        assertTrue(
            "activeControlHitboxes must be @Volatile",
            java.lang.reflect.Modifier.isVolatile(controlField.modifiers)
        )

        // The bridge thread reads this flag to decide whether to enqueue a cancel,
        // while the UI thread owns the writes. Without @Volatile the bridge may see
        // a stale false and skip the enqueue entirely, leaving the I1 cancel undone.
        val capturedField = RenpyActivity::class.java.getDeclaredField("isTouchCapturedByGame")
        assertTrue(
            "isTouchCapturedByGame must be @Volatile (read from the JS bridge thread)",
            java.lang.reflect.Modifier.isVolatile(capturedField.modifiers)
        )
    }

    @Test
    fun gameOwnedGestureReachesSurfaceOnceAndMirrorsFeedback() {
        val activity = TestableRenpyActivity()
        assertTrue("a canvas DOWN belongs to the game", activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        assertTrue(activity.dispatch(MotionEvent.ACTION_MOVE, 920f, 520f))
        // The MOVE lands inside the 16ms window, so it is coalesced rather than
        // emitted: one sample per frame at most.
        assertEquals(1, activity.runnableQueue.size)
        activity.now += 20
        activity.flushRunnables()
        assertTrue(activity.dispatch(MotionEvent.ACTION_UP, 930f, 530f))

        assertEquals("each game-owned sample dispatches to SDL exactly once", 3, activity.surfaceDispatchCount)
        assertEquals(3, activity.feedbackScripts.size)
        assertTrue(activity.feedbackScripts[0].contains("spawnTouchRipple(450, 250)"))
        assertTrue(activity.feedbackScripts[1].contains("sampleTouchTrail(460, 260)"))
        assertTrue(activity.feedbackScripts[2].contains("endTouchTrail"))
    }

    @Test
    fun overlayOwnedGestureNeverReachesSurfaceOrSpawnsDuplicateEffects() {
        val activity = TestableRenpyActivity()
        activity.activeControlHitboxes = listOf(ControlHitbox("btn-a", 1600f, 800f, 1700f, 900f))

        assertFalse(
            "a tap on an overlay control is not game input",
            activity.dispatch(MotionEvent.ACTION_DOWN, 1650f, 850f)
        )
        assertEquals(0, activity.surfaceDispatchCount)
        assertEquals(0, activity.feedbackScripts.size)
    }

    @Test
    fun modalOwnedGestureNeverReachesSurfaceOrSpawnsEffects() {
        val activity = TestableRenpyActivity()
        activity.onOverlayModalOpenChanged(true)

        assertFalse(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        assertFalse(activity.dispatch(MotionEvent.ACTION_UP, 900f, 500f))
        assertEquals(0, activity.surfaceDispatchCount)
        assertEquals(
            "opening a modal emits one trail reset and never a ripple",
            listOf(1, 0),
            listOf(
                activity.feedbackScripts.count { it.contains("endTouchTrail") },
                activity.feedbackScripts.count { it.contains("spawnTouchRipple") }
            )
        )
    }

    @Test
    fun toolbarReportedGeometryOwnsTouchesBelowTheLegacyCutoff() {
        val activity = TestableRenpyActivity()
        activity.activeToolbarHitboxes = WebActivity.parseControlHitboxes(
            """[{"left":28.0,"top":532.0,"right":96.0,"bottom":600.0}]"""
        )

        assertFalse(
            "the ripple sub-button sits below 260dp and must stay with the overlay",
            activity.dispatch(MotionEvent.ACTION_DOWN, 60f, 560f)
        )
        assertEquals(0, activity.surfaceDispatchCount)

        assertTrue(
            "empty canvas inside the old block belongs to the game",
            activity.dispatch(MotionEvent.ACTION_DOWN, 50f, 400f)
        )
        assertEquals(1, activity.surfaceDispatchCount)
    }

    @Test
    fun forwardFeedbackSampleCoalescesMovesAndDropsTheQueueOnUp() {
        val activity = TestableRenpyActivity()
        activity.forwardFeedbackSample(MotionEvent.ACTION_DOWN, 100f, 100f)
        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 110f, 110f)
        activity.now += 4
        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 120f, 120f)
        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 130f, 130f)

        assertEquals("burst MOVE samples must coalesce into one pending runnable", 1, activity.runnableQueue.size)
        assertTrue(
            "burst MOVE samples must coalesce to the newest position",
            activity.pendingMoveCssSample == Pair(130, 130)
        )

        activity.forwardFeedbackSample(MotionEvent.ACTION_UP, 130f, 130f)
        assertEquals("UP must drop the pending MOVE sample", 0, activity.runnableQueue.size)
        assertNull(activity.pendingMoveCssSample)

        activity.flushRunnables()
        assertTrue(
            "no trail sample may be emitted after the gesture ended",
            activity.feedbackScripts.none { it.contains("sampleTouchTrail") }
        )
    }

    @Test
    fun forwardFeedbackSampleCancelsTrailOnCancel() {
        val activity = TestableRenpyActivity()
        activity.forwardFeedbackSample(MotionEvent.ACTION_CANCEL, 0f, 0f)
        assertEquals(1, activity.feedbackScripts.size)
        assertTrue(activity.feedbackScripts[0].contains("endTouchTrail"))
    }

    @Test
    fun pauseAndDestroyLeaveNoQueuedSamplesOrTrails() {
        val activity = TestableRenpyActivity()
        activity.forwardFeedbackSample(MotionEvent.ACTION_DOWN, 100f, 100f)
        activity.now += 1
        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 140f, 140f)
        assertEquals(1, activity.runnableQueue.size)

        activity.feedbackScripts.clear()
        activity.onPause()
        assertEquals("pause must drop queued MOVE samples", 0, activity.runnableQueue.size)
        assertNull(activity.pendingMoveCssSample)
        assertTrue(
            "pause must close the trail so it does not reappear on resume",
            activity.feedbackScripts.any { it.contains("endTouchTrail") }
        )

        activity.feedbackScripts.clear()
        activity.onDestroy()
        assertEquals("destroy must drop queued MOVE samples", 0, activity.runnableQueue.size)
        assertTrue(
            "destroy must close the trail",
            activity.feedbackScripts.any { it.contains("endTouchTrail") }
        )
    }

    @Test
    fun modalOpenDropsQueuedFeedbackButKeepsGameInputOwnership() {
        val activity = TestableRenpyActivity()
        activity.forwardFeedbackSample(MotionEvent.ACTION_DOWN, 100f, 100f)
        activity.now += 1
        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 140f, 140f)
        assertEquals(1, activity.runnableQueue.size)

        activity.onOverlayModalOpenChanged(true)
        assertEquals("opening a modal must drop queued feedback samples", 0, activity.runnableQueue.size)
        assertFalse(
            "an open modal keeps the gesture with the overlay",
            RenpyActivity.shouldForwardTouchToGame(
                960f, 540f, 1920f, 1080f, 2.0f,
                isGamepadHidden = false, isOverlayModalOpen = true
            )
        )
    }

    @Test
    fun modalOpeningDuringAGameGestureCancelsSdlExactlyOnceAndStopsFurtherDelivery() {
        val activity = TestableRenpyActivity()

        // Gesture starts on the game canvas.
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        assertEquals(1, activity.surfaceDispatchCount)
        assertTrue(activity.isTouchCapturedByGame)

        // A modal opens mid-gesture from the JS bridge thread. The bridge handler
        // must only queue UI work: no SDL touch may be dispatched off the UI thread.
        activity.onOverlayModalOpenChanged(true)
        assertEquals("no cancellation may run before the queued UI task", 0, activity.surfaceCancelCount)
        assertTrue(activity.isTouchCapturedByGame)

        activity.flushUiTasks()
        assertEquals("exactly one cancellation after the UI task runs", 1, activity.surfaceCancelCount)
        assertFalse("ownership must be dropped by the UI task", activity.isTouchCapturedByGame)

        val scriptsAfterCancel = activity.feedbackScripts.size
        val dispatchesAfterCancel = activity.surfaceDispatchCount

        // Later samples of the same gesture must not reach the game or the effects.
        assertFalse("MOVE after modal open must not be game input", activity.dispatch(MotionEvent.ACTION_MOVE, 950f, 550f))
        assertFalse("UP after modal open must not be game input", activity.dispatch(MotionEvent.ACTION_UP, 960f, 560f))
        assertEquals("no later MOVE/UP may reach SDL", dispatchesAfterCancel, activity.surfaceDispatchCount)
        assertEquals("no later MOVE/UP may reach the overlay effects", scriptsAfterCancel, activity.feedbackScripts.size)
        assertEquals("the gesture must not be cancelled a second time", 1, activity.surfaceCancelCount)
    }

    @Test
    fun modalOpeningFromBridgeQueuesOneCancelAndDoesNotCancelTwice() {
        val activity = TestableRenpyActivity()
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))

        // Two bridge calls before the UI thread drains its queue.
        activity.onOverlayModalOpenChanged(true)
        activity.onOverlayModalOpenChanged(true)
        assertEquals("no SDL work before the UI task", 0, activity.surfaceCancelCount)

        activity.flushUiTasks()
        assertEquals("repeated opens must still cancel only once", 1, activity.surfaceCancelCount)
        assertFalse(activity.isTouchCapturedByGame)
    }

    @Test
    fun gestureEndingBeforeTheQueuedCancelRunsIsNotCancelledAfterwards() {
        val activity = TestableRenpyActivity()
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        activity.onOverlayModalOpenChanged(true)
        assertEquals("no SDL work before the UI task", 0, activity.surfaceCancelCount)

        // The gesture finishes normally before the queued cancel runs.
        assertTrue(activity.dispatch(MotionEvent.ACTION_UP, 910f, 510f))
        assertFalse(activity.isTouchCapturedByGame)

        activity.flushUiTasks()
        assertEquals("an already-ended gesture must not be cancelled", 0, activity.surfaceCancelCount)
        assertEquals(0, activity.cancelEvents.size)
    }

    @Test
    fun queuedCancelDoesNotCancelAGestureStartedAfterTheOriginalEnded() {
        val activity = TestableRenpyActivity()

        // Original gesture is captured, then a modal opens and queues a cancel.
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        activity.onOverlayModalOpenChanged(true)
        assertEquals(0, activity.surfaceCancelCount)

        // Original gesture ends normally before the UI drains its queue.
        assertTrue(activity.dispatch(MotionEvent.ACTION_UP, 910f, 510f))
        assertFalse(activity.isTouchCapturedByGame)

        // Modal closes and a NEW gesture starts, still before the queue drains.
        activity.onOverlayModalOpenChanged(false)
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 800f, 400f))
        assertTrue(activity.isTouchCapturedByGame)

        val dispatchesBeforeFlush = activity.surfaceDispatchCount
        activity.flushUiTasks()

        assertEquals(
            "a queued cancel for the ended gesture must not cancel the new one",
            0,
            activity.surfaceCancelCount
        )
        assertTrue("the new gesture must still be owned by the game", activity.isTouchCapturedByGame)

        // The new gesture keeps routing to SDL and to the overlay effects.
        val scriptsBefore = activity.feedbackScripts.size
        assertTrue(activity.dispatch(MotionEvent.ACTION_MOVE, 820f, 420f))
        assertTrue(activity.dispatch(MotionEvent.ACTION_UP, 830f, 430f))
        assertEquals(dispatchesBeforeFlush + 2, activity.surfaceDispatchCount)
        assertTrue(activity.feedbackScripts.size > scriptsBefore)
        assertFalse(activity.isTouchCapturedByGame)
    }

    @Test
    fun repeatedModalOpensDoNotCancelAnAlreadyCancelledGesture() {
        val activity = TestableRenpyActivity()
        assertTrue(activity.dispatch(MotionEvent.ACTION_DOWN, 900f, 500f))
        activity.onOverlayModalOpenChanged(true)
        activity.flushUiTasks()
        activity.onOverlayModalOpenChanged(true)
        activity.onOverlayModalOpenChanged(false)
        activity.onOverlayModalOpenChanged(true)
        activity.flushUiTasks()
        assertEquals("an already-cancelled gesture must not be cancelled again", 1, activity.surfaceCancelCount)
    }

    @Test
    fun modalOpenWithNoGameGestureDoesNotCancelSdl() {
        val activity = TestableRenpyActivity()
        activity.onOverlayModalOpenChanged(true)
        activity.flushUiTasks()
        assertEquals(0, activity.surfaceCancelCount)
        assertFalse(activity.isTouchCapturedByGame)
    }

    @Test
    fun toolbarGeometryReportedByTheOverlayDrivesRenpyCapture() {
        val toolbar = WebActivity.parseControlHitboxes(
            """[{"left":28.0,"top":532.0,"right":96.0,"bottom":600.0}]"""
        )
        // The ripple sub-button sits below the legacy 260dp cutoff and must stay
        // with the overlay instead of being handed to the game.
        assertFalse(
            RenpyActivity.shouldForwardTouchToGame(
                60f, 560f, 1920f, 1080f, 2.0f,
                isGamepadHidden = false, isOverlayModalOpen = false,
                toolbarHitboxes = toolbar
            )
        )
        // Empty canvas inside the old block must reach the game.
        assertTrue(
            RenpyActivity.shouldForwardTouchToGame(
                50f, 400f, 1920f, 1080f, 2.0f,
                isGamepadHidden = false, isOverlayModalOpen = false,
                toolbarHitboxes = toolbar
            )
        )
    }

    @Test
    fun gestureOwnershipSurvivesCollapsingTheToolbarMidGesture() {
        val expanded = WebActivity.parseControlHitboxes(
            """[{"left":24.0,"top":20.0,"right":100.0,"bottom":96.0},{"left":28.0,"top":532.0,"right":96.0,"bottom":600.0}]"""
        )
        val captured = RenpyActivity.resolveGestureCapturedByGame(
            MotionEvent.ACTION_DOWN, 60f, 560f, 1920f, 1080f, 2.0f,
            isGamepadHidden = false, isOverlayModalOpen = false, wasCapturedByGame = false,
            toolbarHitboxes = expanded
        )
        assertFalse("a DOWN on the ripple sub-button stays with the overlay", captured)

        // Collapsed: geometry withdrawn, but the in-flight gesture keeps its owner.
        val collapsed = WebActivity.parseControlHitboxes(
            """[{"left":24.0,"top":20.0,"right":100.0,"bottom":96.0}]"""
        )
        val moved = RenpyActivity.resolveGestureCapturedByGame(
            MotionEvent.ACTION_MOVE, 60f, 560f, 1920f, 1080f, 2.0f,
            isGamepadHidden = false, isOverlayModalOpen = false, wasCapturedByGame = captured,
            toolbarHitboxes = collapsed
        )
        assertFalse("the overlay keeps the in-flight gesture after collapse", moved)
    }

}
