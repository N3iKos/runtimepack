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
}
