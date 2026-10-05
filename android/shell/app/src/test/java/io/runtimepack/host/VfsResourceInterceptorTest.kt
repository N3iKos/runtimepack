package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
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

        val externalLink = VfsResourceInterceptor.urlToLogicalPath("https://linktr.ee/Veldra.Works")
        assertNull(externalLink)

        val externalGoogle = VfsResourceInterceptor.urlToLogicalPath("https://www.google.com/search?q=test")
        assertNull(externalGoogle)
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
}
