package io.runtimepack.host

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

class OverlayAssetSourceTest {

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
    }

    @After
    fun tearDown() {
        OverlayAssetSource.gpackReader?.close()
        OverlayAssetSource.gpackReader = null
    }

    @Test
    fun testGuessMimeType() {
        assertEquals("text/html", OverlayAssetSource.guessMimeType("overlay/index.html"))
        assertEquals("text/html", OverlayAssetSource.guessMimeType("overlay/page.htm"))
        assertEquals("text/html", OverlayAssetSource.guessMimeType("OVERLAY/INDEX.HTML"))
        assertEquals("application/javascript", OverlayAssetSource.guessMimeType("overlay/vpad.js"))
        assertEquals("application/javascript", OverlayAssetSource.guessMimeType("web_shims/runtimepack_web_shims.mjs"))
        assertEquals("text/css", OverlayAssetSource.guessMimeType("overlay/vpad.css"))
        assertEquals("application/json", OverlayAssetSource.guessMimeType("config.json"))
        assertEquals("image/png", OverlayAssetSource.guessMimeType("overlay/icon.png"))
        assertEquals("image/jpeg", OverlayAssetSource.guessMimeType("overlay/photo.jpg"))
        assertEquals("image/jpeg", OverlayAssetSource.guessMimeType("overlay/photo.jpeg"))
        assertEquals("image/svg+xml", OverlayAssetSource.guessMimeType("overlay/vector.svg"))
        assertEquals("font/ttf", OverlayAssetSource.guessMimeType("fonts/VL-Gothic-Regular.ttf"))
        assertEquals("font/woff", OverlayAssetSource.guessMimeType("fonts/custom.woff"))
        assertEquals("font/woff2", OverlayAssetSource.guessMimeType("fonts/custom.woff2"))
        assertEquals("application/octet-stream", OverlayAssetSource.guessMimeType("unknown.bin"))
        assertEquals("application/octet-stream", OverlayAssetSource.guessMimeType("no_extension"))
    }

    @Test
    fun testNullGpackReaderFallback() {
        assertFalse(OverlayAssetSource.exists("overlay/index.html"))
        assertNull(OverlayAssetSource.open("overlay/index.html"))
        assertNull(OverlayAssetSource.read("overlay/index.html"))
    }

    @Test
    fun testReadAndOpenFromGpackReader() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val shimsJs = "console.log('web shims loaded');".toByteArray(Charsets.UTF_8)
        val fontData = byteArrayOf(0x00, 0x01, 0x00, 0x00, 0x00)

        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml),
                PackItem("web_shims/runtimepack_web_shims.js", shimsJs),
                PackItem("fonts/test.ttf", fontData)
            )
        )

        val reader = GpackStreamReader.open(ByteArrayInputStream(packBytes))
        OverlayAssetSource.gpackReader = reader

        // exists
        assertTrue(OverlayAssetSource.exists("overlay/index.html"))
        assertTrue(OverlayAssetSource.exists("\\overlay\\index.html"))
        assertTrue(OverlayAssetSource.exists("/overlay/index.html"))
        assertTrue(OverlayAssetSource.exists("web_shims/runtimepack_web_shims.js"))
        assertTrue(OverlayAssetSource.exists("fonts/test.ttf"))
        assertFalse(OverlayAssetSource.exists("overlay/missing.html"))

        // read
        val readHtml = OverlayAssetSource.read("overlay/index.html")
        assertNotNull(readHtml)
        assertArrayEquals(overlayHtml, readHtml)

        val readShims = OverlayAssetSource.read("\\web_shims\\runtimepack_web_shims.js")
        assertNotNull(readShims)
        assertArrayEquals(shimsJs, readShims)

        assertNull(OverlayAssetSource.read("overlay/missing.html"))

        // open
        val openFont = OverlayAssetSource.open("fonts/test.ttf")
        assertNotNull(openFont)
        val fontBytes = openFont!!.use { it.readBytes() }
        assertArrayEquals(fontData, fontBytes)

        assertNull(OverlayAssetSource.open("fonts/missing.ttf"))
    }

    @Test
    fun testShouldInterceptOverlayRequest() {
        val overlayHtml = "<html><body>Overlay</body></html>".toByteArray(Charsets.UTF_8)
        val fontData = byteArrayOf(0x00, 0x01, 0x00, 0x00)

        val packBytes = createPackData(
            listOf(
                PackItem("overlay/index.html", overlayHtml),
                PackItem("fonts/test.ttf", fontData)
            )
        )
        OverlayAssetSource.gpackReader = GpackStreamReader.open(ByteArrayInputStream(packBytes))

        // Intercepts file:///android_asset/overlay/index.html
        val responseHtml = OverlayAssetSource.shouldIntercept("file:///android_asset/overlay/index.html?v=1#hash")
        assertNotNull(responseHtml)
        assertEquals("text/html", responseHtml?.mimeType)
        assertEquals("utf-8", responseHtml?.encoding)
        assertArrayEquals(overlayHtml, responseHtml?.data?.use { it.readBytes() })

        // Intercepts file:///android_asset/fonts/test.ttf
        val responseFont = OverlayAssetSource.shouldIntercept("file:///android_asset/fonts/test.ttf")
        assertNotNull(responseFont)
        assertEquals("font/ttf", responseFont?.mimeType)
        assertArrayEquals(fontData, responseFont?.data?.use { it.readBytes() })

        // Non-overlay/fonts asset paths are ignored
        val responseOther = OverlayAssetSource.shouldIntercept("file:///android_asset/data/System.json")
        assertNull(responseOther)

        // Non-file scheme URLs are ignored
        val responseWeb = OverlayAssetSource.shouldIntercept("https://game.local/overlay/index.html")
        assertNull(responseWeb)

        // Missing overlay file returns null
        val responseMissing = OverlayAssetSource.shouldIntercept("file:///android_asset/overlay/missing.html")
        assertNull(responseMissing)
    }
}
