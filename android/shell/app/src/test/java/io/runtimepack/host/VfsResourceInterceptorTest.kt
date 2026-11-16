package io.runtimepack.host

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

class VfsResourceInterceptorTest {

    @Test
    fun testDetectMimeTypeFromExtensions() {
        assertEquals("image/png", VfsResourceInterceptor.getMimeType("img/characters/Actor.png"))
        assertEquals("image/png", VfsResourceInterceptor.getMimeType("img/characters/Actor.PNG"))
        assertEquals("image/jpeg", VfsResourceInterceptor.getMimeType("img/pictures/Photo.jpg"))
        assertEquals("image/jpeg", VfsResourceInterceptor.getMimeType("img/pictures/Photo.jpeg"))
        assertEquals("image/webp", VfsResourceInterceptor.getMimeType("img/pictures/Photo.webp"))

        assertEquals("audio/ogg", VfsResourceInterceptor.getMimeType("audio/bgm/Theme.ogg"))
        assertEquals("audio/mpeg", VfsResourceInterceptor.getMimeType("audio/se/Attack.mp3"))
        assertEquals("audio/wav", VfsResourceInterceptor.getMimeType("audio/se/Hit.wav"))
        assertEquals("audio/mp4", VfsResourceInterceptor.getMimeType("audio/bgm/Theme.m4a"))

        assertEquals("application/javascript", VfsResourceInterceptor.getMimeType("js/rpg_core.js"))
        assertEquals("application/json", VfsResourceInterceptor.getMimeType("data/System.json"))
        assertEquals("text/css", VfsResourceInterceptor.getMimeType("css/game.css"))
        assertEquals("text/html", VfsResourceInterceptor.getMimeType("index.html"))
        assertEquals("text/html", VfsResourceInterceptor.getMimeType("manual.htm"))

        assertEquals("application/octet-stream", VfsResourceInterceptor.getMimeType("data/binary.bin"))
        assertEquals("application/octet-stream", VfsResourceInterceptor.getMimeType("unknown_extension"))

        assertEquals("image/png", VfsResourceInterceptor.getMimeType("img/test.png?v=1#header"))
    }

    @Test
    fun testSanitizeUrlToLogicalPath() {
        val path1 = VfsResourceInterceptor.urlToLogicalPath("https://game.local/img/characters/Actor.png")
        assertEquals("img/characters/Actor.png", path1)

        val path1Query = VfsResourceInterceptor.urlToLogicalPath("https://game.local/js/plugins.js?version=1.2#debug")
        assertEquals("js/plugins.js", path1Query)

        val path2 = VfsResourceInterceptor.urlToLogicalPath("file:///android_asset/game/data/System.json")
        assertEquals("data/System.json", path2)

        val path2Query = VfsResourceInterceptor.urlToLogicalPath("file:///android_asset/game/audio/bgm/Theme.ogg?v=2#track1")
        assertEquals("audio/bgm/Theme.ogg", path2Query)

        val pathLocalhost1 = VfsResourceInterceptor.urlToLogicalPath("http://localhost/index.html")
        assertEquals("index.html", pathLocalhost1)

        val pathLocalhost2 = VfsResourceInterceptor.urlToLogicalPath("http://localhost:8080/css/game.css?theme=dark")
        assertEquals("css/game.css", pathLocalhost2)

        val pathLocalhost3 = VfsResourceInterceptor.urlToLogicalPath("http://127.0.0.1:3000/data/Items.json")
        assertEquals("data/Items.json", pathLocalhost3)

        val pathDotDot = VfsResourceInterceptor.urlToLogicalPath("https://game.local/data/scenario/../system/_setting.ks")
        assertEquals("data/system/_setting.ks", pathDotDot)

        val externalLink = VfsResourceInterceptor.urlToLogicalPath("https://linktr.ee/Veldra.Works")
        assertNull(externalLink)

        val externalGoogle = VfsResourceInterceptor.urlToLogicalPath("https://www.google.com/search?q=test")
        assertNull(externalGoogle)
    }

    @Test
    fun testWebBackendNormalizeLogicalPath() {
        assertEquals("data/system/_setting.ks", WebBackend.normalizeLogicalPath("data/scenario/../system/_setting.ks"))
        assertEquals("data/system/_setting.ks", WebBackend.normalizeLogicalPath("data\\scenario\\..\\system\\_setting.ks"))
        assertEquals("data/system/_setting.ks", WebBackend.normalizeLogicalPath("/data/scenario/../system/_setting.ks"))
        assertEquals("img/characters/actor.png", WebBackend.normalizeLogicalPath("img/characters/Actor.png"))
        assertNull(WebBackend.normalizeLogicalPath("../../escape_root.txt"))
        assertNull(WebBackend.normalizeLogicalPath("data/../../escape_root.txt"))
    }

    @Test
    fun testMockStreamInterceptionRoundTrip() {
        val mockData = "console.log('mock script');".toByteArray(Charsets.UTF_8)
        val interceptor = VfsResourceInterceptor { path ->
            if (path == "js/mock.js") {
                ByteArrayInputStream(mockData)
            } else {
                null
            }
        }

        val response = interceptor.intercept("https://game.local/js/mock.js?v=42")
        assertNotNull(response)

        val missingResponse = interceptor.intercept("https://game.local/js/not_found.js")
        assertNull(missingResponse)

        val externalResponse = interceptor.intercept("https://linktr.ee/Veldra.Works")
        assertNull(externalResponse)
    }

    @Test
    fun test404ImageReturnsTransparentPng() {
        AssetAuditService.reset()
        val interceptor = VfsResourceInterceptor { null }

        val pngResp = interceptor.intercept("https://game.local/img/characters/Missing.png")
        assertNotNull(pngResp)

        val events = AssetAuditService.getEvents(0L)
        org.junit.Assert.assertTrue("Events should record placeholder substitution", events.isNotEmpty())
        val fallbackEvent = events.first()
        assertEquals(RuntimeAuditStage.FILE_EXISTS, fallbackEvent.stage)
        assertEquals(RuntimeAuditOutcome.FAIL, fallbackEvent.outcome)
        assertEquals("PLACEHOLDER_SUBSTITUTED", fallbackEvent.fallback_action)
        assertEquals("Resource missing in VFS, transparent placeholder substituted", fallbackEvent.error_message)

        val jpgResp = interceptor.intercept("https://game.local/img/pictures/Missing.jpg")
        assertNotNull(jpgResp)

        val jpegResp = interceptor.intercept("https://game.local/img/pictures/Missing.jpeg")
        assertNotNull(jpegResp)

        val webpResp = interceptor.intercept("https://game.local/img/pictures/Missing.webp")
        assertNotNull(webpResp)

        // Verify TRANSPARENT_1X1_PNG constant has valid PNG header
        assertEquals(0x89.toByte(), VfsResourceInterceptor.TRANSPARENT_1X1_PNG[0])
        assertEquals('P'.code.toByte(), VfsResourceInterceptor.TRANSPARENT_1X1_PNG[1])
        assertEquals('N'.code.toByte(), VfsResourceInterceptor.TRANSPARENT_1X1_PNG[2])
        assertEquals('G'.code.toByte(), VfsResourceInterceptor.TRANSPARENT_1X1_PNG[3])
    }

    @Test
    fun test404AudioReturnsEmptyAudioResponse() {
        val interceptor = VfsResourceInterceptor { null }

        val oggResp = interceptor.intercept("https://game.local/audio/bgm/Missing.ogg")
        assertNotNull(oggResp)

        val m4aResp = interceptor.intercept("https://game.local/audio/bgm/Missing.m4a")
        assertNotNull(m4aResp)

        val mp3Resp = interceptor.intercept("https://game.local/audio/se/Missing.mp3")
        assertNotNull(mp3Resp)

        val wavResp = interceptor.intercept("https://game.local/audio/se/Missing.wav")
        assertNotNull(wavResp)
    }

    @Test
    fun testGpackStreamReaderInterceptionRoundTrip() {
        val gpackBytes = createTestGpack(
            listOf(
                "index.html" to "<!DOCTYPE html><html><body>Test</body></html>".toByteArray(Charsets.UTF_8),
                "data/System.json" to "{\"gameTitle\":\"Interceptor Game\"}".toByteArray(Charsets.UTF_8)
            )
        )

        GpackStreamReader.open(ByteArrayInputStream(gpackBytes), null).use { reader ->
            val interceptor = VfsResourceInterceptor(reader)

            val htmlResponse = interceptor.intercept("https://game.local/index.html")
            assertNotNull(htmlResponse)

            val jsonResponse = interceptor.intercept("http://localhost:8080/data/System.json?t=100")
            assertNotNull(jsonResponse)

            val missingAudioResponse = interceptor.intercept("https://game.local/audio/bgm/Title.ogg")
            assertNotNull(missingAudioResponse)

            val missingOtherResponse = interceptor.intercept("https://game.local/data/missing.bin")
            assertNull(missingOtherResponse)

            val externalResponse = interceptor.intercept("https://external.com/index.html")
            assertNull(externalResponse)
        }
    }

    private fun createTestGpack(files: List<Pair<String, ByteArray>>): ByteArray {
        val magic = byteArrayOf(
            'G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(),
            'K'.code.toByte(), 0, 1, 0
        )
        val headerSize = 64
        var indexLen = 0
        for ((path, _) in files) {
            val pBytes = path.toByteArray(Charsets.UTF_8)
            indexLen += 4 + pBytes.size + 8 + 8 + 8 + 32 + 1 + 1
        }
        val indexOffset = headerSize.toLong()
        val dataOffset = indexOffset + indexLen

        val headerBuf = ByteArray(64)
        System.arraycopy(magic, 0, headerBuf, 0, 8)
        val hWrap = ByteBuffer.wrap(headerBuf).order(ByteOrder.LITTLE_ENDIAN)
        hWrap.position(8)
        hWrap.putInt(1)
        hWrap.putInt(0)
        hWrap.putLong(indexOffset)
        hWrap.putLong(indexLen.toLong())
        hWrap.putInt(files.size)

        val indexBuf = ByteArray(indexLen)
        val iWrap = ByteBuffer.wrap(indexBuf).order(ByteOrder.LITTLE_ENDIAN)

        var curOffset = dataOffset
        for ((path, data) in files) {
            val pBytes = path.toByteArray(Charsets.UTF_8)
            iWrap.putInt(pBytes.size)
            iWrap.put(pBytes)
            iWrap.putLong(curOffset)
            iWrap.putLong(data.size.toLong())
            iWrap.putLong(data.size.toLong())
            iWrap.put(ByteArray(32))
            iWrap.put(0.toByte())
            iWrap.put(0.toByte())
            curOffset += data.size
        }

        val out = ByteArrayOutputStream()
        out.write(headerBuf)
        out.write(indexBuf)
        for ((_, data) in files) {
            out.write(data)
        }
        return out.toByteArray()
    }

    @Test
    fun testInternalRelativePathNormalization() {
        val normalPath = VfsResourceInterceptor.urlToLogicalPath("https://game.local/data/image/../fgimage/char.png")
        assertEquals("data/fgimage/char.png", normalPath)

        val localNormalPath = VfsResourceInterceptor.urlToLogicalPath("http://localhost:8080/data/image/../fgimage/char.png")
        assertEquals("data/fgimage/char.png", localNormalPath)

        val escapingPath1 = VfsResourceInterceptor.urlToLogicalPath("https://game.local/../secret.txt")
        assertNull(escapingPath1)

        val escapingPath2 = VfsResourceInterceptor.urlToLogicalPath("https://game.local/data/../../secret.txt")
        assertNull(escapingPath2)

        val escapingPath3 = VfsResourceInterceptor.urlToLogicalPath("file:///android_asset/game/data/../../secret.txt")
        assertNull(escapingPath3)

        val charPngData = "char_image_bytes".toByteArray(Charsets.UTF_8)
        val interceptor = VfsResourceInterceptor { path ->
            if (path == "data/fgimage/char.png") {
                ByteArrayInputStream(charPngData)
            } else {
                null
            }
        }

        val response = interceptor.intercept("https://game.local/data/image/../fgimage/char.png")
        assertNotNull("Should serve data/fgimage/char.png via collapsed relative path", response)

        val escapeResponse = interceptor.intercept("https://game.local/../secret.txt")
        assertNull("Escaping root should return null", escapeResponse)

        val escapeResponse2 = interceptor.intercept("https://game.local/data/../../secret.txt")
        assertNull("Escaping root with multiple traversals should return null", escapeResponse2)
    }

    @Test
    fun testSubfolderPluginResolutionFallback() {
        val pluginContent = "/*: @target MZ */ console.log('subfolder plugin loaded');".toByteArray(Charsets.UTF_8)
        val gpackBytes = createTestGpack(
            listOf(
                "index.html" to "<!DOCTYPE html><html><body>Test</body></html>".toByteArray(Charsets.UTF_8),
                "js/plugins/develop/PluginCommonBase.js" to pluginContent
            )
        )

        GpackStreamReader.open(ByteArrayInputStream(gpackBytes), null).use { reader ->
            val interceptor = VfsResourceInterceptor(reader)

            // When requested at standard root js/plugins/PluginCommonBase.js, should fallback to develop/PluginCommonBase.js
            val fallbackResponse = interceptor.intercept("https://game.local/js/plugins/PluginCommonBase.js")
            assertNotNull("Should resolve plugin in subfolder when requested at root", fallbackResponse)

            // Direct request to subfolder should also work
            val directResponse = interceptor.intercept("https://game.local/js/plugins/develop/PluginCommonBase.js")
            assertNotNull("Should resolve plugin at direct subfolder path", directResponse)

            // Non-existent plugin should return null
            val notFoundResponse = interceptor.intercept("https://game.local/js/plugins/NonExistent.js")
            assertNull("Non-existent plugin should return null", notFoundResponse)
        }
    }

    @Test
    fun testEncryptedTilesetCaseInsensitiveResolution() {
        val testTilesetData = "fake_encrypted_tileset_bytes".toByteArray(Charsets.UTF_8)
        val gpackBytes = createTestGpack(
            listOf(
                "index.html" to "<!DOCTYPE html><html><body>Test</body></html>".toByteArray(Charsets.UTF_8),
                "img/tilesets/mori_a2.rpgmvp" to testTilesetData
            )
        )

        GpackStreamReader.open(ByteArrayInputStream(gpackBytes), null).use { reader ->
            val interceptor = VfsResourceInterceptor(reader)

            // Direct request with case mismatch (mori_A2 vs mori_a2)
            val caseMismatchResponse = interceptor.intercept("https://game.local/img/tilesets/mori_A2.rpgmvp")
            assertNotNull("Should resolve mori_A2.rpgmvp when disk has mori_a2.rpgmvp", caseMismatchResponse)

            // Direct request with exact case
            val exactResponse = interceptor.intercept("https://game.local/img/tilesets/mori_a2.rpgmvp")
            assertNotNull("Should resolve mori_a2.rpgmvp directly", exactResponse)
        }

        // Test with custom streamProvider that only recognizes lowercase
        val streamInterceptor = VfsResourceInterceptor { path ->
            if (path == "img/tilesets/mori_a2.rpgmvp") {
                ByteArrayInputStream(testTilesetData)
            } else {
                null
            }
        }
        val streamResponse = streamInterceptor.intercept("https://game.local/img/tilesets/mori_A2.rpgmvp")
        assertNotNull("streamProvider should resolve mori_A2.rpgmvp via lowercase fallback", streamResponse)
    }

    @Test
    fun test404EncryptedAssetReturnsPlaceholder() {
        val interceptor = VfsResourceInterceptor { null }
        val imgResp = interceptor.intercept("https://game.local/img/tilesets/missing.rpgmvp")
        assertNotNull("Missing .rpgmvp should return transparent PNG placeholder", imgResp)
        val audioResp = interceptor.intercept("https://game.local/audio/se/missing.rpgmvo")
        assertNotNull("Missing .rpgmvo should return empty audio placeholder", audioResp)
    }

    @Test
    fun testWebBridgeReaddirWithGpackReader() {
        val gpackBytes = createTestGpack(
            listOf(
                "data/System.json" to "{}".toByteArray(Charsets.UTF_8),
                "data/Map001.json" to "{}".toByteArray(Charsets.UTF_8),
                "img/pictures/cg1.png" to "png".toByteArray(Charsets.UTF_8)
            )
        )
        GpackStreamReader.open(ByteArrayInputStream(gpackBytes), null).use { reader ->
            val mockActivity = WebActivity()
            mockActivity.gpackReader = reader
            WebActivity.currentInstance = mockActivity
            try {
                val bridge = WebBridge(object : SaveService {
                    override fun openRead(slot: String): java.io.InputStream? = null
                    override fun openWrite(slot: String): java.io.OutputStream = java.io.ByteArrayOutputStream()
                    override fun exists(slot: String): Boolean = false
                    override fun list(): List<String> = emptyList()
                    override fun delete(slot: String): Boolean = false
                })
                val dataJson = bridge.readdir("data")
                org.junit.Assert.assertTrue("readdir('data') should include system.json", dataJson.contains("\"system.json\""))
                org.junit.Assert.assertTrue("readdir('data') should include map001.json", dataJson.contains("\"map001.json\""))
                org.junit.Assert.assertFalse("readdir('data') should not include cg1.png", dataJson.contains("cg1.png"))

                val picJson = bridge.readdir("img/pictures")
                org.junit.Assert.assertTrue("readdir('img/pictures') should include cg1.png", picJson.contains("\"cg1.png\""))

                val emptyJson = bridge.readdir("nonexistent")
                org.junit.Assert.assertEquals("[]", emptyJson)
            } finally {
                WebActivity.currentInstance = null
            }
        }
    }

    @Test
    fun testCaseInsensitiveHashResolutionWithoutLinearScan() {
        val interceptor = VfsResourceInterceptor(
            reader = null,
            rpgMakerKey = null,
            streamProvider = { path ->
                if (path.lowercase() == "audio/se/jump1.ogg") ByteArrayInputStream("sound".toByteArray()) else null
            }
        )
        val resp = interceptor.intercept("https://game.local/audio/se/Jump1.ogg")
        assertNotNull(resp)
        assertEquals("audio/ogg", resp?.mimeType)
    }

    @Test
    fun testIsVnmProfileNormalization() {
        val validProfiles = listOf("vnm", "VNM", "visual-novel-maker", "VisualNovelMaker", "vnmaker", "vnmaker-web", "VNM Web", "vnmaker_web")
        for (profile in validProfiles) {
            val interceptor = VfsResourceInterceptor(engineFamily = profile) { null }
            org.junit.Assert.assertTrue("Profile '$profile' should be identified as VNM", interceptor.isVnmProfile())
        }

        val invalidProfiles = listOf("rpgmaker-mz", "rpgmaker-mv", "renpy", "tyrano", "", null)
        for (profile in invalidProfiles) {
            val interceptor = VfsResourceInterceptor(engineFamily = profile) { null }
            org.junit.Assert.assertFalse("Profile '$profile' should NOT be identified as VNM", interceptor.isVnmProfile())
        }
    }

    @Test
    fun testAllowlistedWebShimsServedAndNamespaceEscapeBlocked() {
        val shimContent = "console.log('web shims');"
        val interceptor = VfsResourceInterceptor(engineFamily = "vnm") { path ->
            if (path == "web_shims/runtimepack_web_shims.js") {
                ByteArrayInputStream(shimContent.toByteArray(Charsets.UTF_8))
            } else if (path == "web_shims/unauthorized.js") {
                ByteArrayInputStream("secret".toByteArray(Charsets.UTF_8))
            } else {
                null
            }
        }

        val allowedResp = interceptor.intercept("https://game.local/web_shims/runtimepack_web_shims.js")
        assertNotNull(allowedResp)
        assertEquals("application/javascript", allowedResp?.mimeType)
        assertEquals("UTF-8", allowedResp?.encoding)
        val servedText = allowedResp?.data?.bufferedReader()?.readText() ?: ""
        assertEquals(shimContent, servedText)

        val forbiddenResp = interceptor.intercept("https://game.local/web_shims/unauthorized.js")
        assertNull(forbiddenResp)

        val guardOffResp = interceptor.intercept("https://game.local/web_shims/runtimepack_vnm_input_guard.js")
        assertNull(guardOffResp)

        val guardContent = "console.log('guard');"
        val guardInterceptor = VfsResourceInterceptor(engineFamily = "vnm", vnmInputGuard = "transition-aware") { path ->
            if (path == "web_shims/runtimepack_vnm_input_guard.js") {
                ByteArrayInputStream(guardContent.toByteArray(Charsets.UTF_8))
            } else {
                null
            }
        }
        val guardResp = guardInterceptor.intercept("https://game.local/web_shims/runtimepack_vnm_input_guard.js")
        assertNotNull(guardResp)
        assertEquals("application/javascript", guardResp?.mimeType)
        assertEquals("UTF-8", guardResp?.encoding)

        val nonVnmGuardInterceptor = VfsResourceInterceptor(engineFamily = "rpgmaker-mv", vnmInputGuard = "transition-aware") { path ->
            if (path == "web_shims/runtimepack_vnm_input_guard.js") {
                ByteArrayInputStream(guardContent.toByteArray(Charsets.UTF_8))
            } else {
                null
            }
        }
        assertNull(nonVnmGuardInterceptor.intercept("https://game.local/web_shims/runtimepack_vnm_input_guard.js"))
    }

    @Test
    fun testVnmTwoBoundaryHtmlTransform() {
        val interceptor = VfsResourceInterceptor(engineFamily = "vnmaker-web") { null }
        val rawHtml = """
            <!DOCTYPE html>
            <html>
            <head>
                <!--{PLATFORM_LIBS}-->
                <script type="text/javascript" src="data/lib/glMatrix.js"></script>
                <script type="text/javascript" src="data/ENGINE.js"></script>
                <script type="text/javascript" src="data/SCRIPTS.js"></script>
            </head>
            <body></body>
            </html>
        """.trimIndent()

        val transformed = interceptor.transformVnmHtml(rawHtml)
        org.junit.Assert.assertTrue(transformed.contains("<!-- RUNTIMEPACK_VNM_SHIMS -->"))
        org.junit.Assert.assertTrue(transformed.contains("web_shims/runtimepack_web_shims.js"))
        org.junit.Assert.assertTrue(transformed.contains("__runtimepackPollVnmStorage"))
        org.junit.Assert.assertTrue(transformed.contains("__runtimepackPollVnmMessageCompat"))

        val b1Idx = transformed.indexOf("web_shims/runtimepack_web_shims.js")
        val engineIdx = transformed.indexOf("data/ENGINE.js")
        val postEngineStorageIdx = transformed.indexOf("__runtimepackPollVnmStorage")
        val scriptsIdx = transformed.indexOf("data/SCRIPTS.js")
        val b2Idx = transformed.indexOf("__runtimepackPollVnmMessageCompat")

        org.junit.Assert.assertTrue(b1Idx < engineIdx)
        org.junit.Assert.assertTrue(engineIdx < postEngineStorageIdx)
        org.junit.Assert.assertTrue(postEngineStorageIdx < scriptsIdx)
        org.junit.Assert.assertTrue(scriptsIdx < b2Idx)
        org.junit.Assert.assertFalse(transformed.contains("web_shims/runtimepack_vnm_input_guard.js"))

        val guardTransformed = interceptor.transformVnmHtml(rawHtml, guardMode = "transition-aware")
        org.junit.Assert.assertTrue(guardTransformed.contains("window.__runtimepack_vnm_input_guard_mode = \"transition-aware\""))
        org.junit.Assert.assertTrue(guardTransformed.contains("web_shims/runtimepack_vnm_input_guard.js"))
        val guardScriptsIdx = guardTransformed.indexOf("data/SCRIPTS.js")
        val guardB2Idx = guardTransformed.indexOf("__runtimepackPollVnmMessageCompat")
        val guardIdx = guardTransformed.indexOf("web_shims/runtimepack_vnm_input_guard.js")
        org.junit.Assert.assertTrue(guardScriptsIdx < guardB2Idx)
        org.junit.Assert.assertTrue(guardB2Idx < guardIdx)
    }

    @Test
    fun testVnmHtmlDuplicateMarkerDoesNotReapply() {
        val interceptor = VfsResourceInterceptor(engineFamily = "vnm") { null }
        val rawHtml = """
            <!-- RUNTIMEPACK_VNM_SHIMS -->
            <script src="web_shims/runtimepack_web_shims.js"></script>
            <script src="data/SCRIPTS.js"></script>
        """.trimIndent()
        val transformed = interceptor.transformVnmHtml(rawHtml)
        assertEquals(rawHtml, transformed)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testVnmHtmlUnsupportedAsyncDeferRejection() {
        val interceptor = VfsResourceInterceptor(engineFamily = "vnm") { null }
        val rawHtml = """
            <!DOCTYPE html><html><head>
            <script src="data/ENGINE.js"></script>
            <script async src="data/SCRIPTS.js"></script>
            </head></html>
        """.trimIndent()
        interceptor.transformVnmHtml(rawHtml)
    }

    @Test(expected = IllegalArgumentException::class)
    fun testVnmHtmlMissingAnchorDiagnosticRejection() {
        val interceptor = VfsResourceInterceptor(engineFamily = "vnm") { null }
        val rawHtml = "<!DOCTYPE html><html><head><title>No scripts</title></head></html>"
        interceptor.transformVnmHtml(rawHtml)
    }

    @Test
    fun testVnmInterceptServesTransformedHtmlAtRootAndIndex() {
        val rawHtml = """
            <!DOCTYPE html>
            <html>
            <head>
                <script type="text/javascript" src="./data/ENGINE.js"></script>
                <script type="text/javascript" src='./data/SCRIPTS.js'></script>
            </head>
            <body></body>
            </html>
        """.trimIndent()

        val interceptor = VfsResourceInterceptor(
            engineFamily = "vnm",
            streamProvider = { path ->
                if (path == "index.html") ByteArrayInputStream(rawHtml.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val rootResp = interceptor.intercept("https://game.local/")
        assertNotNull(rootResp)
        assertEquals("text/html", rootResp?.mimeType)
        assertEquals("UTF-8", rootResp?.encoding)
        val rootText = rootResp?.data?.bufferedReader()?.readText() ?: ""
        org.junit.Assert.assertTrue(rootText.contains("web_shims/runtimepack_web_shims.js"))
        org.junit.Assert.assertTrue(rootText.contains("__runtimepackPollVnmMessageCompat"))

        val indexResp = interceptor.intercept("https://game.local/index.html")
        assertNotNull(indexResp)
        assertEquals("text/html", indexResp?.mimeType)
        val indexText = indexResp?.data?.bufferedReader()?.readText() ?: ""
        org.junit.Assert.assertTrue(indexText.contains("web_shims/runtimepack_web_shims.js"))
    }

    @Test
    fun testNonVnmInterceptServesUnmodifiedHtml() {
        val rawHtml = "<html><head><script src=\"js/rpg_core.js\"></script></head></html>"
        val interceptor = VfsResourceInterceptor(
            engineFamily = "rpgmaker-mz",
            streamProvider = { path ->
                if (path == "index.html") ByteArrayInputStream(rawHtml.toByteArray(Charsets.UTF_8)) else null
            }
        )
        val resp = interceptor.intercept("https://game.local/index.html")
        assertNotNull(resp)
        val text = resp?.data?.bufferedReader()?.readText() ?: ""
        assertEquals(rawHtml, text)
    }

    @Test
    fun testScriptCompletionHookAppendsTrailerStream() {
        val coreContent = "function Utils() { /* core */ }\n"
        val pluginsContent = "var \$plugins = [];\n"
        var rawStreamClosed = false

        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path ->
                when (path) {
                    "js/rmmz_core.js" -> object : ByteArrayInputStream(coreContent.toByteArray(Charsets.UTF_8)) {
                        override fun close() {
                            super.close()
                            rawStreamClosed = true
                        }
                    }
                    "js/plugins.js" -> ByteArrayInputStream(pluginsContent.toByteArray(Charsets.UTF_8))
                    else -> null
                }
            }
        )

        // 1. Hooked script receives original content + completion trailer
        val hookedResp = interceptor.intercept("https://game.local/js/rmmz_core.js")
        assertNotNull("Hooked script must be intercepted", hookedResp)
        assertEquals("application/javascript", hookedResp?.mimeType)
        assertEquals("UTF-8", hookedResp?.encoding)

        val servedContent = hookedResp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        org.junit.Assert.assertTrue("Raw stream must be closed after stream reader is closed", rawStreamClosed)
        org.junit.Assert.assertTrue("Served stream must contain original core content", servedContent.startsWith(coreContent))
        org.junit.Assert.assertTrue("Served stream must append completion trailer", servedContent.contains(VfsResourceInterceptor.SCRIPT_COMPLETION_TRAILER))
        org.junit.Assert.assertTrue("Trailer must invoke applyPlatformIdentity", servedContent.contains("window.RuntimePack.applyPlatformIdentity(u)"))
        org.junit.Assert.assertTrue("Trailer must guard ASI with prefix newline and semicolon", servedContent.contains("\n;\n(function() {"))

        // 2. Unhooked script must NOT receive trailer
        val unhookedResp = interceptor.intercept("https://game.local/js/plugins.js")
        assertNotNull(unhookedResp)
        val unhookedContent = unhookedResp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        assertEquals(pluginsContent, unhookedContent)
        org.junit.Assert.assertFalse("Unhooked script must not contain trailer", unhookedContent.contains("applyPlatformIdentity"))

        // 3. Basename-only mismatch at different directory does not trigger hook
        val diffPathResp = interceptor.intercept("https://game.local/js/other_core.js")
        assertNull(diffPathResp)
    }

    @Test
    fun testScriptCompletionHookMixedCaseRequestAppendsTrailer() {
        val coreContent = "function Utils() { /* core */ }\n"
        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path ->
                if (path.lowercase() == "js/rmmz_core.js") {
                    ByteArrayInputStream(coreContent.toByteArray(Charsets.UTF_8))
                } else if (path.lowercase() == "js/plugins.js") {
                    ByteArrayInputStream("var \$plugins = [];\n".toByteArray(Charsets.UTF_8))
                } else {
                    null
                }
            }
        )

        // 1. Mixed-case request JS/Rmmz_Core.js with declared lower hook MUST append trailer
        val mixedResp = interceptor.intercept("https://game.local/JS/Rmmz_Core.js")
        assertNotNull("Mixed-case request must be intercepted", mixedResp)
        assertEquals("application/javascript", mixedResp?.mimeType)
        val mixedContent = mixedResp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        org.junit.Assert.assertTrue("Mixed-case request must contain core content", mixedContent.startsWith(coreContent))
        org.junit.Assert.assertTrue("Mixed-case request must append completion trailer", mixedContent.contains("window.RuntimePack.applyPlatformIdentity(u)"))

        // 2. Unhooked mixed-case script must NOT receive trailer
        val unhookedMixedResp = interceptor.intercept("https://game.local/JS/Plugins.js")
        assertNotNull(unhookedMixedResp)
        val unhookedMixedContent = unhookedMixedResp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        org.junit.Assert.assertFalse("Unhooked mixed-case script must NOT receive trailer", unhookedMixedContent.contains("applyPlatformIdentity"))

        // 3. Basename-only match at different directory must NOT receive trailer
        val diffDirResp = interceptor.intercept("https://game.local/other/rmmz_core.js")
        assertNull("Different directory with same basename must not match hook", diffDirResp)
    }

    @Test
    fun testWebHtmlTransformInjectsWebShimsWhenHooksActive() {
        val rawHtml = """
            <!DOCTYPE html>
            <html>
            <head>
                <script type="text/javascript" src="js/main.js"></script>
            </head>
            <body></body>
            </html>
        """.trimIndent()

        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path ->
                if (path == "index.html") ByteArrayInputStream(rawHtml.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val resp = interceptor.intercept("https://game.local/index.html")
        assertNotNull("index.html must be intercepted", resp)
        assertEquals("text/html", resp?.mimeType)
        assertEquals("UTF-8", resp?.encoding)

        val transformed = resp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        org.junit.Assert.assertTrue("Transformed HTML must contain web_shims tag", transformed.contains("web_shims/runtimepack_web_shims.js"))
        val shimIdx = transformed.indexOf("web_shims/runtimepack_web_shims.js")
        val mainIdx = transformed.indexOf("js/main.js")
        org.junit.Assert.assertTrue("web_shims must precede js/main.js in document order", shimIdx < mainIdx)

        // Duplicate marker should not add redundant script tag
        val retransformed = interceptor.transformWebHtml(transformed)
        assertEquals("Re-transforming should be idempotent", transformed, retransformed)
    }

    @Test
    fun testCoreCompletionTrailerCarriesIdentityHandoffContract() {
        val coreContent = "function Utils() { /* core */ }\n"
        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rpg_core.js"),
            streamProvider = { path ->
                if (path == "js/rpg_core.js") ByteArrayInputStream(coreContent.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val resp = interceptor.intercept("https://game.local/js/rpg_core.js")
        assertNotNull("Core hook must be intercepted", resp)
        val served = resp?.data?.bufferedReader()?.use { it.readText() } ?: ""

        org.junit.Assert.assertTrue("Served stream must keep core content intact", served.startsWith(coreContent))
        org.junit.Assert.assertTrue("Core content must be followed by the trailer body", served.contains("(function() {"))
        org.junit.Assert.assertTrue(
            "Trailer must keep the canonical applier call",
            served.contains("window.RuntimePack.applyPlatformIdentity(u)")
        )
        org.junit.Assert.assertTrue(
            "Trailer must retain the Utils handoff for the shim parsed later",
            served.contains("__runtimepack_pendingIdentity")
        )
        org.junit.Assert.assertTrue(
            "Trailer must install a configurable isNwjs accessor when the applier is absent",
            served.contains("Object.defineProperty(u, \"isNwjs\"")
        )
        org.junit.Assert.assertTrue("Trailer fallback accessor must stay configurable", served.contains("configurable: true"))
        org.junit.Assert.assertTrue(
            "Trailer inline pin must mark the canonical false-fn",
            served.contains("__rp_patched")
        )
        org.junit.Assert.assertFalse(
            "Trailer must not claim shim state",
            served.contains("platformIdentityActive = true")
        )
        org.junit.Assert.assertTrue("Trailer must guard ASI", served.contains("\n;\n(function() {"))
        org.junit.Assert.assertTrue(
            "Trailer must keep the explicit missing-Utils diagnostic",
            served.contains("Utils is undefined in core script completion hook")
        )
    }

    @Test
    fun testCoreCompletionTrailerStillAppendsForMzHook() {
        val coreContent = "var Utils = { isNwjs: function() { return true; } };\n"
        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path ->
                if (path == "js/rmmz_core.js") ByteArrayInputStream(coreContent.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val resp = interceptor.intercept("https://game.local/js/rmmz_core.js")
        assertNotNull("MZ core hook must be intercepted", resp)
        val served = resp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        org.junit.Assert.assertTrue("MZ core content must be preserved", served.startsWith(coreContent))
        org.junit.Assert.assertTrue(
            "MZ trailer must keep the canonical applier call",
            served.contains("window.RuntimePack.applyPlatformIdentity(u)")
        )
        org.junit.Assert.assertTrue(
            "MZ trailer must retain the Utils handoff",
            served.contains("__runtimepack_pendingIdentity")
        )
    }

    @Test
    fun testMvWebHtmlTransformInjectsSingleShimBeforeMain() {
        val rawHtml = """
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <title>B-Prison</title>
                <script type="text/javascript" src="js/libs/pixi.js"></script>
                <script type="text/javascript" src="js/libs/pixi-tilemap.js"></script>
                <script type="text/javascript" src="js/libs/pixi-picture.js"></script>
                <script type="text/javascript" src="js/libs/fpsmeter.js"></script>
                <script type="text/javascript" src="js/libs/lz-string.js"></script>
                <script type="text/javascript" src="js/libs/iphone-inline-video.browser.js"></script>
                <script type="text/javascript" src="js/rpg_core.js"></script>
                <script type="text/javascript" src="js/rpg_managers.js"></script>
                <script type="text/javascript" src="js/rpg_objects.js"></script>
                <script type="text/javascript" src="js/rpg_scenes.js"></script>
                <script type="text/javascript" src="js/rpg_sprites.js"></script>
                <script type="text/javascript" src="js/rpg_windows.js"></script>
                <script type="text/javascript" src="js/plugins.js"></script>
                <script type="text/javascript" src="js/main.js"></script>
            </head>
            <body></body>
            </html>
        """.trimIndent()

        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rpg_core.js"),
            streamProvider = { path ->
                if (path == "index.html") ByteArrayInputStream(rawHtml.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val resp = interceptor.intercept("https://game.local/index.html")
        assertNotNull("index.html must be served through the interceptor", resp)
        val transformed = resp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        val shimMarker = "web_shims/runtimepack_web_shims.js"
        assertEquals("Exactly one shim tag must be injected", 1, transformed.split(shimMarker).size - 1)
        org.junit.Assert.assertTrue(
            "Shim must still precede js/main.js",
            transformed.indexOf(shimMarker) < transformed.indexOf("js/main.js")
        )
        assertEquals("Re-transforming must be idempotent", transformed, interceptor.transformWebHtml(transformed))
        assertEquals("Idempotent re-transform must keep exactly one shim tag", 1, interceptor.transformWebHtml(transformed).split(shimMarker).size - 1)
    }

    @Test
    fun testMzWebHtmlTransformInjectsSingleShimBeforeMain() {
        val rawHtml = """
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <title>Game</title>
                <script type="text/javascript" src="js/libs/pixi.js"></script>
                <script type="text/javascript" src="js/rmmz_core.js"></script>
                <script type="text/javascript" src="js/rmmz_managers.js"></script>
                <script type="text/javascript" src="js/rmmz_objects.js"></script>
                <script type="text/javascript" src="js/rmmz_scenes.js"></script>
                <script type="text/javascript" src="js/rmmz_sprites.js"></script>
                <script type="text/javascript" src="js/rmmz_windows.js"></script>
                <script type="text/javascript" src="js/plugins.js"></script>
                <script type="text/javascript" src="js/main.js"></script>
            </head>
            <body></body>
            </html>
        """.trimIndent()

        val interceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path ->
                if (path == "index.html") ByteArrayInputStream(rawHtml.toByteArray(Charsets.UTF_8)) else null
            }
        )

        val resp = interceptor.intercept("https://game.local/index.html")
        assertNotNull("MZ index.html must be served through the interceptor", resp)
        val transformed = resp?.data?.bufferedReader()?.use { it.readText() } ?: ""
        val shimMarker = "web_shims/runtimepack_web_shims.js"
        assertEquals("Exactly one shim tag must be injected", 1, transformed.split(shimMarker).size - 1)
        org.junit.Assert.assertTrue(
            "Shim must still precede js/main.js",
            transformed.indexOf(shimMarker) < transformed.indexOf("js/main.js")
        )
        assertEquals("Re-transforming must be idempotent", transformed, interceptor.transformWebHtml(transformed))
    }

    @Test
    fun testAtSignAndPercent40DirectAndEncryptedFallback() {
        val mioEncryptedBytes = byteArrayOf(0x52, 0x50, 0x47, 0x4D, 0x11, 0x22, 0x33, 0x44)
        val literalPercentBytes = byteArrayOf(0x05, 0x06, 0x07, 0x08)
        val heroPlusBytes = byteArrayOf(0x09, 0x0A, 0x0B, 0x0C)

        val interceptor = VfsResourceInterceptor { path ->
            when (path) {
                "img/pictures/mio_tachie_@_@.png_" -> ByteArrayInputStream(mioEncryptedBytes)
                "img/pictures/literal_%40.png" -> ByteArrayInputStream(literalPercentBytes)
                "img/pictures/hero+party.png" -> ByteArrayInputStream(heroPlusBytes)
                else -> null
            }
        }

        // 1. Direct request for .png_ with %40 must resolve to literal @ .png_ and return exact provider bytes
        val directResp = interceptor.intercept("https://game.local/img/pictures/mio_tachie_%40_%40.png_")
        assertNotNull("Direct %40 .png_ request must resolve to literal @ on disk", directResp)
        assertEquals("image/png", directResp?.mimeType)
        val directData = directResp?.data?.readBytes()
        assertArrayEquals("Must serve exact encrypted provider bytes, NOT placeholder", mioEncryptedBytes, directData)

        // 2. Direct request with literal @ must also resolve directly
        val directAtResp = interceptor.intercept("https://game.local/img/pictures/mio_tachie_@_@.png_")
        assertNotNull("Direct @ .png_ request must resolve", directAtResp)
        assertArrayEquals(mioEncryptedBytes, directAtResp?.data?.readBytes())

        // 3. Plain .png request with %40 must fallback to literal @ .png_ and return exact provider bytes
        val fallbackResp = interceptor.intercept("https://game.local/img/pictures/mio_tachie_%40_%40.png")
        assertNotNull("Plain %40 .png request must fallback to literal @ .png_", fallbackResp)
        assertEquals("image/png", fallbackResp?.mimeType)
        val fallbackData = fallbackResp?.data?.readBytes()
        assertArrayEquals("Fallback must serve exact provider bytes of .png_, NOT placeholder", mioEncryptedBytes, fallbackData)

        // 4. Literal %40 filename on disk should be accessible via %2540 (single decode only, no double decode to @)
        val literalResp = interceptor.intercept("https://game.local/img/pictures/literal_%2540.png")
        assertNotNull("Single decode %2540 -> %40 should resolve literal %40 file", literalResp)
        assertArrayEquals(literalPercentBytes, literalResp?.data?.readBytes())

        // 5. Filenames with literal '+' must be preserved and NOT converted to space
        val plusResp = interceptor.intercept("https://game.local/img/pictures/hero+party.png")
        assertNotNull("Filename with literal '+' must not turn into space", plusResp)
        assertArrayEquals(heroPlusBytes, plusResp?.data?.readBytes())
    }

    @Test
    fun testPercentDecodingAndTraversalSecurity() {
        val interceptor = VfsResourceInterceptor { null }

        // Traversal attempts via percent-encoding must fail-closed (null)
        assertNull(interceptor.urlToLogicalPath("https://game.local/%2e%2e/secret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%2e%2e/%2e%2e/secret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%5csecret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%00secret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%2fsecret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%GGsecret.txt"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/data/%2"))
        assertNull(interceptor.urlToLogicalPath("https://game.local/c:/secret.txt"))
    }

    @Test
    fun testHtmlTransformationClosesRawStream() {
        var vnmStreamClosed = false
        var webStreamClosed = false

        val vnmHtml = "<!DOCTYPE html><html><head><script src='data/ENGINE.js'></script><script src='data/SCRIPTS.js'></script></head><body></body></html>"
        val webHtml = "<!DOCTYPE html><html><head><script src='js/main.js'></script></head><body></body></html>"

        val vnmInterceptor = VfsResourceInterceptor(engineFamily = "vnm") { path ->
            if (path == "index.html") {
                object : ByteArrayInputStream(vnmHtml.toByteArray(Charsets.UTF_8)) {
                    override fun close() {
                        super.close()
                        vnmStreamClosed = true
                    }
                }
            } else null
        }

        val vnmResp = vnmInterceptor.intercept("https://game.local/index.html")
        assertNotNull(vnmResp)
        assertTrue("VNM transform site must close rawStream", vnmStreamClosed)

        val webInterceptor = VfsResourceInterceptor(
            scriptCompletionHooks = listOf("js/rmmz_core.js"),
            streamProvider = { path: String ->
                if (path == "index.html") {
                    object : ByteArrayInputStream(webHtml.toByteArray(Charsets.UTF_8)) {
                        override fun close() {
                            super.close()
                            webStreamClosed = true
                        }
                    }
                } else null
            }
        )

        val webResp = webInterceptor.intercept("https://game.local/index.html")
        assertNotNull(webResp)
        assertTrue("Web transform site must close rawStream", webStreamClosed)
    }
}
