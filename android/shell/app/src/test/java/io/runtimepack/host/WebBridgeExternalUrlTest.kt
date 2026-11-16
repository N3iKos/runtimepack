package io.runtimepack.host

import android.content.ActivityNotFoundException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.InputStream
import java.io.OutputStream

class WebBridgeExternalUrlTest {

    private class MockSaveService : SaveService {
        override fun openRead(slot: String): InputStream? = null
        override fun openWrite(slot: String): OutputStream = object : OutputStream() {
            override fun write(b: Int) {}
        }
        override fun delete(slot: String): Boolean = true
        override fun exists(slot: String): Boolean = false
        override fun list(): List<String> = emptyList()
    }

    @Before
    fun setUp() {
        DiagnosticLogStore.resetAll()
    }

    @Test
    fun testWebBridgeDelegatesToDispatcher() {
        val saveService = MockSaveService()
        val recordedUrls = mutableListOf<String>()

        // 1. WebBridge with null dispatcher returns false
        val bridgeNoDispatcher = WebBridge(saveService = saveService, externalUrlDispatcher = null)
        assertFalse(bridgeNoDispatcher.openExternalUrl("https://example.com"))

        // 2. WebBridge with active dispatcher delegates properly
        val bridgeWithDispatcher = WebBridge(
            saveService = saveService,
            externalUrlDispatcher = { url: String ->
                recordedUrls.add(url)
                true
            }
        )
        assertTrue(bridgeWithDispatcher.openExternalUrl("https://example.com"))
        assertEquals(1, recordedUrls.size)
        assertEquals("https://example.com", recordedUrls[0])

        // 3. WebBridge returns dispatcher's false if rejected
        val bridgeRejecting = WebBridge(
            saveService = saveService,
            externalUrlDispatcher = { false }
        )
        assertFalse(bridgeRejecting.openExternalUrl("https://invalid.com"))
    }

    @Test
    fun testConcreteBridgeWiringEndToEnd() {
        val saveService = MockSaveService()
        val queuedRunnables = mutableListOf<() -> Unit>()
        val launchedUrls = mutableListOf<String>()

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { true },
            launcher = { url: String, _: String ->
                launchedUrls.add(url)
            }
        )

        // WebBridge wired with { dispatcher.dispatch(it) } exactly as in production
        val bridge = WebBridge(
            saveService = saveService,
            externalUrlDispatcher = { url: String -> dispatcher.dispatch(url) }
        )

        // 1. Valid URL accepted and queued
        assertTrue(bridge.openExternalUrl("https://example.com/patreon?tier=1"))
        assertEquals(1, queuedRunnables.size)
        assertEquals(0, launchedUrls.size)

        // Flush queued UI runnable
        queuedRunnables[0].invoke()
        assertEquals(1, launchedUrls.size)
        assertEquals("https://example.com/patreon?tier=1", launchedUrls[0])

        // 2. Invalid URL rejected immediately without queueing
        assertFalse(bridge.openExternalUrl("javascript:alert(1)"))
        assertEquals(1, queuedRunnables.size) // No new runnable queued
        assertEquals(1, launchedUrls.size)

        // 3. Dangerous credentials URL rejected
        assertFalse(bridge.openExternalUrl("http://user:pass@evil.com"))
        assertEquals(1, queuedRunnables.size)
    }

    @Test
    fun testSharedNavigationPolicyOriginIsolation() {
        // Canonical game origin
        assertTrue(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", -1))
        assertTrue(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("http", "game.local", 80))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "evil.com", 443))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 8080))

        // Credential injection on game origin rejected
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://user@game.local/"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://user:pass@game.local/"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://user%40name@game.local/"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://game.local%40evil.com/"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://game.local%2540evil.com/"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://game.local\\evil/"))

        // Path and query with @ and %40 allowed on canonical game origin (RFC 3986 S44)
        assertTrue(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", -1, "https://game.local/img/pictures/mio_tachie_%40_%40.png_"))
        assertTrue(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", -1, "https://game.local/img/pictures/mio_tachie_@_@.png_"))
        assertTrue(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://game.local/data/actor@2.json?tag=@main&version=1"))

        // Caller-supplied argument mismatch / rawUrl spoofing rejected
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://evil.com/path"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", -1, "http://game.local/path"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", -1, "https://game.local:8080/path"))
        assertFalse(SharedNavigationPolicy.isCanonicalGameUri("https", "game.local", 443, "https://game.local/%5cpath"))

        // Legacy asset paths
        assertTrue(SharedNavigationPolicy.isAllowedAssetUri("file:///android_asset/game/index.html"))
        assertTrue(SharedNavigationPolicy.isAllowedAssetUri("file:///android_asset/game/fonts/font.ttf"))
        assertFalse(SharedNavigationPolicy.isAllowedAssetUri("file:///android_asset/game/../other/secret.txt"))
        assertFalse(SharedNavigationPolicy.isAllowedAssetUri("file:///android_asset/game/%2e%2e/secret.txt"))
        assertFalse(SharedNavigationPolicy.isAllowedAssetUri("file:///android_asset/other/index.html"))
        assertFalse(SharedNavigationPolicy.isAllowedAssetUri("https://game.local/index.html"))
    }

    @Test
    fun testDispatcherValidUrlQueuedAndLaunched() {
        val queuedRunnables = mutableListOf<() -> Unit>()
        val launchedUrls = mutableListOf<String>()

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { true },
            launcher = { url: String, _: String ->
                launchedUrls.add(url)
            }
        )

        val accepted = dispatcher.dispatch("https://example.com/test?q=1")
        assertTrue(accepted)
        assertEquals(1, queuedRunnables.size)
        assertEquals(0, launchedUrls.size)

        // Execute queued UI runnable
        queuedRunnables[0].invoke()
        assertEquals(1, launchedUrls.size)
        assertEquals("https://example.com/test?q=1", launchedUrls[0])
    }

    @Test
    fun testDispatcherRejectsUnsafeUrlsWithAnd501() {
        val queuedRunnables = mutableListOf<() -> Unit>()

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { true },
            launcher = { _: String, _: String -> }
        )

        assertFalse(dispatcher.dispatch("javascript:alert(1)"))
        assertFalse(dispatcher.dispatch("file:///etc/passwd"))
        assertFalse(dispatcher.dispatch("http://user:pass@evil.com/"))
        assertEquals(0, queuedRunnables.size)

        val errorsJson = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(errorsJson.contains(WebExternalUrlDiagnostics.CODE_INVALID_URL))
    }

    @Test
    fun testDispatcherLifecycleChecks() {
        var alive = false
        val queuedRunnables = mutableListOf<() -> Unit>()
        var launched = false

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { alive },
            launcher = { _: String, _: String -> launched = true }
        )

        // 1. Activity dead before queue: dispatch returns false immediately
        assertFalse(dispatcher.dispatch("https://example.com"))
        assertEquals(0, queuedRunnables.size)

        // 2. Activity alive during queue, but dies before UI runnable executes
        alive = true
        assertTrue(dispatcher.dispatch("https://example.com"))
        assertEquals(1, queuedRunnables.size)

        // Activity dies
        alive = false
        queuedRunnables[0].invoke()
        assertFalse("Launcher must not be called when activity died before execution", launched)

        val errorsJson = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(errorsJson.contains(WebExternalUrlDiagnostics.CODE_DISPATCH_FAILED))
    }

    @Test
    fun testDispatcherHandlesActivityNotFoundWithAnd502() {
        val queuedRunnables = mutableListOf<() -> Unit>()

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { true },
            launcher = { _: String, _: String ->
                throw ActivityNotFoundException("No browser")
            }
        )

        assertTrue(dispatcher.dispatch("https://example.com"))
        queuedRunnables[0].invoke()

        val errorsJson = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(errorsJson.contains(WebExternalUrlDiagnostics.CODE_NO_HANDLER))
    }

    @Test
    fun testDispatcherHandlesSecurityExceptionWithAnd503() {
        val queuedRunnables = mutableListOf<() -> Unit>()

        val dispatcher = WebExternalUrlDispatcher(
            queueToUi = { r: () -> Unit ->
                queuedRunnables.add(r)
                true
            },
            isAlive = { true },
            launcher = { _: String, _: String ->
                throw SecurityException("Permission denied")
            }
        )

        assertTrue(dispatcher.dispatch("https://example.com"))
        queuedRunnables[0].invoke()

        val errorsJson = DiagnosticLogStore.getDiagnosticErrorsJson()
        assertTrue(errorsJson.contains(WebExternalUrlDiagnostics.CODE_DISPATCH_FAILED))
    }
}
