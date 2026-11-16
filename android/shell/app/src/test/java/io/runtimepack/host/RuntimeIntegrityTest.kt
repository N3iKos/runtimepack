package io.runtimepack.host

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

class RuntimeIntegrityTest {

    private val GPACK_MAGIC = byteArrayOf(
        'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
        'K'.code.toByte(), 0, 1, 0
    )

    private data class PackItem(
        val path: String,
        val content: ByteArray
    )

    private fun createPackData(items: List<PackItem>): ByteArray {
        val flags = 0
        val headerSize = 64

        var indexBytesLen = 0
        for (item in items) {
            val pathLen = item.path.toByteArray(Charsets.UTF_8).size
            indexBytesLen += 4 + pathLen + 8 + 8 + 8 + 32 + 1 + 1
        }

        val indexOffset = headerSize.toLong()
        val indexLength = indexBytesLen.toLong()
        val dataStartOffset = indexOffset + indexLength

        val headerBuf = ByteArray(64)
        System.arraycopy(GPACK_MAGIC, 0, headerBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(1)
        hWrap.putInt(flags)
        hWrap.putLong(indexOffset)
        hWrap.putLong(indexLength)
        hWrap.putInt(items.size)

        val indexBuf = ByteArray(indexBytesLen)
        val iWrap = ByteBuffer.wrap(indexBuf).order(ByteOrder.LITTLE_ENDIAN)

        var curOffset = dataStartOffset
        val fileOffsets = mutableListOf<Long>()
        for (item in items) {
            fileOffsets.add(curOffset)
            curOffset += item.content.size
        }

        for ((i, item) in items.withIndex()) {
            val pBytes = item.path.toByteArray(Charsets.UTF_8)
            iWrap.putInt(pBytes.size)
            iWrap.put(pBytes)
            iWrap.putLong(fileOffsets[i])
            iWrap.putLong(item.content.size.toLong())
            iWrap.putLong(item.content.size.toLong())
            iWrap.put(ByteArray(32))
            iWrap.put(0.toByte())
            iWrap.put(0.toByte())
        }

        val out = ByteArrayOutputStream()
        out.write(headerBuf)
        out.write(indexBuf)
        for (item in items) {
            out.write(item.content)
        }
        return out.toByteArray()
    }

    @Before
    fun setUp() {
        OverlayAssetSource.gpackReader = null
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = ""
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = emptyList()
        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
    }

    @After
    fun tearDown() {
        OverlayAssetSource.gpackReader?.close()
        OverlayAssetSource.gpackReader = null
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = ""
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = emptyList()
        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
    }

    @Test
    fun testComputeHashSingleVector() {
        val files = listOf(
            "overlay/test.txt" to "hello".toByteArray(Charsets.UTF_8)
        )
        val hash = RuntimeIntegrity.computeHash(files)
        assertEquals("b3d9db26cfb28fa5d189b914eb404c61b995a743b9c2e583d155bc1dfcbfaa88", hash)
    }

    @Test
    fun testComputeHashMultipleSorted() {
        val filesReverse = listOf(
            "overlay/b.js" to "test".toByteArray(Charsets.UTF_8),
            "fonts/a.ttf" to "world".toByteArray(Charsets.UTF_8)
        )
        val hash = RuntimeIntegrity.computeHash(filesReverse)
        assertEquals("f5f5a7000e20110ccac51637f556b04558263117b9eb36b5db6cb50504bac4a0", hash)
    }

    @Test
    fun testComputeHashPathNormalization() {
        val files1 = listOf(
            "fonts/a.ttf" to "world".toByteArray(Charsets.UTF_8),
            "overlay/b.js" to "test".toByteArray(Charsets.UTF_8)
        )
        val files2 = listOf(
            "\\fonts\\a.ttf" to "world".toByteArray(Charsets.UTF_8),
            "/overlay/b.js" to "test".toByteArray(Charsets.UTF_8)
        )
        assertEquals(RuntimeIntegrity.computeHash(files1), RuntimeIntegrity.computeHash(files2))
    }

    @Test
    fun testComputeHashContentChange() {
        val files1 = listOf(
            "overlay/test.txt" to "hello".toByteArray(Charsets.UTF_8)
        )
        val files2 = listOf(
            "overlay/test.txt" to "hellp".toByteArray(Charsets.UTF_8)
        )
        assertNotEquals(RuntimeIntegrity.computeHash(files1), RuntimeIntegrity.computeHash(files2))
    }

    @Test
    fun testVerifyBetaFlavorAlwaysOk() {
        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = "some-expected-hash"

        val result = RuntimeIntegrity.verify(null)
        assertTrue(result is IntegrityResult.Ok)
    }

    @Test
    fun testVerifyReleaseFlavorMatchOk() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        val expectedHash = RuntimeIntegrity.computeHash(
            listOf("overlay/index.html" to overlayHtml)
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = expectedHash

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Ok but got $result", result is IntegrityResult.Ok)
        assertEquals(expectedHash, (result as IntegrityResult.Ok).hash)
    }

    @Test
    fun testVerifyReleaseFlavorMismatch() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = "0000000000000000000000000000000000000000000000000000000000000000"

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Mismatch but got $result", result is IntegrityResult.Mismatch)
        val mismatch = result as IntegrityResult.Mismatch
        assertEquals("0000000000000000000000000000000000000000000000000000000000000000", mismatch.expected)
    }

    @Test
    fun testVerifyReleaseFlavorGpackMissingFilesReturnsAnd402() {
        // Reader has other game files, but no overlay/web_shims/fonts
        val packBytes = createPackData(
            listOf(
                PackItem("data/System.json", "{}".toByteArray(Charsets.UTF_8))
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = "dummy"

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Unreadable but got $result", result is IntegrityResult.Unreadable)
        val unreadable = result as IntegrityResult.Unreadable
        assertEquals("AND-402", unreadable.code)
    }

    @Test
    fun testVerifyReleaseFlavorGpackReaderNullReturnsAnd403() {
        OverlayAssetSource.gpackReader = null

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = "dummy"

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Unreadable but got $result", result is IntegrityResult.Unreadable)
        val unreadable = result as IntegrityResult.Unreadable
        assertEquals("AND-403", unreadable.code)
    }

    @Test
    fun testVerifyReleaseFlavorEmptyExpectedHashReturnsAnd403() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = ""

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Unreadable but got $result", result is IntegrityResult.Unreadable)
        val unreadable = result as IntegrityResult.Unreadable
        assertEquals("AND-403", unreadable.code)
        assertEquals("Expected integrity hash is missing in release build", unreadable.detail)
    }

    @Test
    fun testVerifyReleaseFlavorCaseFoldingDoesNotIncludeGameFonts() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val templateFont = "template-font-bytes".toByteArray(Charsets.UTF_8)
        val gameFont = "game-font-bytes".toByteArray(Charsets.UTF_8)

        // RPG Maker VX Ace layout has "Fonts/..." (capital F), template has "fonts/..." (lowercase f)
        val packBytes = createPackData(
            listOf(
                PackItem("Fonts/VL-Gothic-Regular.ttf", gameFont),
                PackItem("Fonts/DenkOne-Regular.ttf", gameFont),
                PackItem("overlay/index.html", overlayHtml),
                PackItem("fonts/VL-Gothic-Regular.ttf", templateFont)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Expected hash computed only over template assets ("overlay/index.html" and "fonts/VL-Gothic-Regular.ttf")
        val expectedHash = RuntimeIntegrity.computeHash(
            listOf(
                "overlay/index.html" to overlayHtml,
                "fonts/VL-Gothic-Regular.ttf" to templateFont
            )
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = expectedHash

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Ok but got $result", result is IntegrityResult.Ok)
        assertEquals(expectedHash, (result as IntegrityResult.Ok).hash)
    }

    @Test
    fun testVerifyReleaseFlavorAuthoritativePathsExcludesLowercaseGameFonts() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val templateFont = "template-font-bytes".toByteArray(Charsets.UTF_8)
        val gameFont = "game-font-bytes".toByteArray(Charsets.UTF_8)

        // Game has lowercase "fonts/game-font.ttf" (e.g. MV/MZ convention)
        val packBytes = createPackData(
            listOf(
                PackItem("fonts/game-font.ttf", gameFont),
                PackItem("overlay/index.html", overlayHtml),
                PackItem("fonts/VL-Gothic-Regular.ttf", templateFont)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Authoritative paths list declared by generator: only template assets
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf(
            "overlay/index.html",
            "fonts/VL-Gothic-Regular.ttf"
        )

        val expectedHash = RuntimeIntegrity.computeHash(
            listOf(
                "overlay/index.html" to overlayHtml,
                "fonts/VL-Gothic-Regular.ttf" to templateFont
            )
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = expectedHash

        val result = RuntimeIntegrity.verify(null)
        assertTrue("Expected IntegrityResult.Ok but got $result", result is IntegrityResult.Ok)
        assertEquals(expectedHash, (result as IntegrityResult.Ok).hash)
    }

    private class TestContext(assets: Map<String, ByteArray> = emptyMap()) : android.content.ContextWrapper(null) {
        private val am = android.content.res.AssetManager().apply {
            for ((k, v) in assets) {
                addAsset(k, v)
            }
        }
        override fun getAssets(): android.content.res.AssetManager = am
    }

    @Test
    fun testVerifyBetaLooseAuthoritativeWithReaderAndGameFontCollision() {
        val gameFontBytes = "game-font-payload-vx-ace".toByteArray(Charsets.UTF_8)
        val templateFontBytes = "template-font-canonical-vl-gothic".toByteArray(Charsets.UTF_8)
        val overlayHtml = "<html><body>Beta Overlay</body></html>".toByteArray(Charsets.UTF_8)

        // Pack contains colliding game font and game data, but no overlay
        val packBytes = createPackData(
            listOf(
                PackItem("Fonts/VL-Gothic-Regular.ttf", gameFontBytes),
                PackItem("data/System.json", "{}".toByteArray(Charsets.UTF_8))
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Loose APK assets contain authoritative template font and overlay
        val context = TestContext(
            mapOf(
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes,
                "overlay/index.html" to overlayHtml
            )
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf(
            "fonts/VL-Gothic-Regular.ttf",
            "overlay/index.html"
        )

        val expectedHash = RuntimeIntegrity.computeHash(
            listOf(
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes,
                "overlay/index.html" to overlayHtml
            )
        )
        val packOnlyFontHash = RuntimeIntegrity.computeHash(
            listOf("fonts/VL-Gothic-Regular.ttf" to gameFontBytes)
        )

        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = expectedHash

        val result = RuntimeIntegrity.verify(context, expectedHash, "beta")
        assertTrue("Beta flavor verify must return IntegrityResult.Ok", result is IntegrityResult.Ok)
        val actualHash = (result as IntegrityResult.Ok).hash
        assertEquals("Beta loose assets must be authoritative over pack collision", expectedHash, actualHash)
        assertNotEquals("Must not match pack-only game font digest", packOnlyFontHash, actualHash)
    }

    @Test
    fun testVerifyBetaDifferentGameVsRuntimeFontBytesRuntimeWins() {
        val gameFontBytes = "game-font-different-bytes".toByteArray(Charsets.UTF_8)
        val runtimeFontBytes = "runtime-template-font-authoritative-bytes".toByteArray(Charsets.UTF_8)

        val packBytes = createPackData(
            listOf(
                PackItem("Fonts/VL-Gothic-Regular.ttf", gameFontBytes)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        val context = TestContext(
            mapOf("fonts/VL-Gothic-Regular.ttf" to runtimeFontBytes)
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf("fonts/VL-Gothic-Regular.ttf")

        val expectedTemplateHash = RuntimeIntegrity.computeHash(
            listOf("fonts/VL-Gothic-Regular.ttf" to runtimeFontBytes)
        )
        val packGameHash = RuntimeIntegrity.computeHash(
            listOf("fonts/VL-Gothic-Regular.ttf" to gameFontBytes)
        )

        val result = RuntimeIntegrity.verify(context, expectedTemplateHash, "beta")
        assertTrue(result is IntegrityResult.Ok)
        val actualHash = (result as IntegrityResult.Ok).hash
        assertEquals("Runtime font bytes must win over pack game font", expectedTemplateHash, actualHash)
        assertNotEquals(packGameHash, actualHash)
    }

    @Test
    fun testVerifyBetaMissingLooseAssetDoesNotSubstituteGamePack() {
        val gameFontBytes = "game-font-payload".toByteArray(Charsets.UTF_8)
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val templateFontBytes = "template-font-bytes".toByteArray(Charsets.UTF_8)

        val packBytes = createPackData(
            listOf(
                PackItem("Fonts/VL-Gothic-Regular.ttf", gameFontBytes)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Context only has overlay, missing loose template font
        val context = TestContext(
            mapOf("overlay/index.html" to overlayHtml)
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "beta"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf(
            "fonts/VL-Gothic-Regular.ttf",
            "overlay/index.html"
        )

        val fullExpectedHash = RuntimeIntegrity.computeHash(
            listOf(
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes,
                "overlay/index.html" to overlayHtml
            )
        )
        val overlayOnlyHash = RuntimeIntegrity.computeHash(
            listOf("overlay/index.html" to overlayHtml)
        )

        val result = RuntimeIntegrity.verify(context, fullExpectedHash, "beta")
        assertTrue("Beta policy must return Ok", result is IntegrityResult.Ok)
        val actualHash = (result as IntegrityResult.Ok).hash
        assertEquals("Must digest only readable loose assets without falling back to game pack", overlayOnlyHash, actualHash)
        assertNotEquals(fullExpectedHash, actualHash)
    }

    @Test
    fun testVerifyReleaseRejectsLooseAssetsWhenPackEntryMissing() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val templateFontBytes = "template-font-bytes".toByteArray(Charsets.UTF_8)

        // Pack has font, but missing overlay
        val packBytes = createPackData(
            listOf(
                PackItem("fonts/VL-Gothic-Regular.ttf", templateFontBytes)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Context has valid loose overlay and font
        val context = TestContext(
            mapOf(
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes,
                "overlay/index.html" to overlayHtml
            )
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf(
            "fonts/VL-Gothic-Regular.ttf",
            "overlay/index.html"
        )

        val fullExpectedHash = RuntimeIntegrity.computeHash(
            listOf(
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes,
                "overlay/index.html" to overlayHtml
            )
        )
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = fullExpectedHash

        val result = RuntimeIntegrity.verify(context, fullExpectedHash, "release")
        assertTrue("Release must fail closed (Mismatch) when pack entry is missing even if loose exists", result is IntegrityResult.Mismatch)
        val mismatch = result as IntegrityResult.Mismatch
        assertEquals(fullExpectedHash, mismatch.expected)
    }

    @Test
    fun testVerifyReleaseMissingPackContainerRejectsLooseRescue() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val context = TestContext(
            mapOf("overlay/index.html" to overlayHtml)
        )

        OverlayAssetSource.gpackReader = null
        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf("overlay/index.html")
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = "expected"

        val result = RuntimeIntegrity.verify(context, "expected", "release")
        assertTrue("Release must return AND-403 when reader is null even if loose exists", result is IntegrityResult.Unreadable)
        val unreadable = result as IntegrityResult.Unreadable
        assertEquals("AND-403", unreadable.code)
    }

    @Test
    fun testVerifyReleaseValidPackWithContextPresentSucceeds() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val templateFontBytes = "template-font-bytes".toByteArray(Charsets.UTF_8)

        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml),
                PackItem("fonts/VL-Gothic-Regular.ttf", templateFontBytes)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        val context = TestContext(
            mapOf(
                "overlay/index.html" to "different-loose-bytes".toByteArray(Charsets.UTF_8),
                "fonts/VL-Gothic-Regular.ttf" to "different-loose-font".toByteArray(Charsets.UTF_8)
            )
        )

        RuntimeBuildInfo.BUILD_FLAVOR = "release"
        RuntimeBuildInfo.RUNTIME_INTEGRITY_PATHS = listOf(
            "overlay/index.html",
            "fonts/VL-Gothic-Regular.ttf"
        )

        val expectedPackHash = RuntimeIntegrity.computeHash(
            listOf(
                "overlay/index.html" to overlayHtml,
                "fonts/VL-Gothic-Regular.ttf" to templateFontBytes
            )
        )
        RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256 = expectedPackHash

        val result = RuntimeIntegrity.verify(context, expectedPackHash, "release")
        assertTrue("Release must read from pack and succeed", result is IntegrityResult.Ok)
        assertEquals(expectedPackHash, (result as IntegrityResult.Ok).hash)
    }
}
