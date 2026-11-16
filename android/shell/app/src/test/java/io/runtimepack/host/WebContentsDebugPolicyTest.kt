package io.runtimepack.host

import android.content.pm.ApplicationInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class WebContentsDebugPolicyTest {

    private val debuggableFlags = ApplicationInfo.FLAG_DEBUGGABLE
    private val plainFlags = 0

    @Test
    fun betaFlavorWithDebuggableAppEnablesDebugging() {
        assertTrue(WebContentsDebugPolicy.shouldEnableDebugging("beta", debuggableFlags))
    }

    @Test
    fun releaseFlavorWithDebuggableAppDeniesDebugging() {
        assertFalse(WebContentsDebugPolicy.shouldEnableDebugging("release", debuggableFlags))
    }

    @Test
    fun betaFlavorWithoutDebuggableAppDeniesDebugging() {
        assertFalse(WebContentsDebugPolicy.shouldEnableDebugging("beta", plainFlags))
    }

    @Test
    fun unknownOrMissingFlavorDeniesDebugging() {
        listOf(null, "", "Beta", "BETA", "beta ", "production", "unknown").forEach { flavor ->
            assertFalse("flavor=$flavor", WebContentsDebugPolicy.shouldEnableDebugging(flavor, debuggableFlags))
        }
    }

    @Test
    fun applyToAlwaysPassesExplicitDecisionToSetter() {
        val calls = mutableListOf<Boolean>()

        val enabled = WebContentsDebugPolicy.applyTo("beta", debuggableFlags) { calls.add(it) }
        assertTrue(enabled)
        assertEquals(listOf(true), calls)

        calls.clear()
        val denied = WebContentsDebugPolicy.applyTo("release", debuggableFlags) { calls.add(it) }
        assertFalse(denied)
        assertEquals("denied hosts must explicitly disable, not skip", listOf(false), calls)

        calls.clear()
        WebContentsDebugPolicy.applyTo(null, plainFlags) { calls.add(it) }
        assertEquals(listOf(false), calls)
    }

    @Test
    fun webActivityAppliesPolicyBeforeLoadingGame() {
        val source = webActivitySource()
        val applyIndex = source.indexOf("WebContentsDebugPolicy.applyTo(")
        assertTrue("WebActivity must route WebView debugging through the policy", applyIndex >= 0)
        assertTrue(
            "WebActivity must call WebView.setWebContentsDebuggingEnabled",
            source.contains("WebView.setWebContentsDebuggingEnabled")
        )
        assertTrue(
            "policy input must be the generated RuntimeBuildInfo.BUILD_FLAVOR",
            source.contains("RuntimeBuildInfo.BUILD_FLAVOR")
        )
        assertTrue(
            "policy input must be applicationInfo.flags",
            source.contains("applicationInfo.flags")
        )
        val loadIndex = source.indexOf("loadUrl(\"https://game.local/index.html\")")
        assertTrue("game load call must exist", loadIndex > 0)
        assertTrue("debug policy must be applied before game loading", applyIndex < loadIndex)
    }

    private fun webActivitySource(): String {
        val candidates = listOf(
            File("src/main/java/io/runtimepack/host/WebActivity.kt"),
            File("app/src/main/java/io/runtimepack/host/WebActivity.kt")
        )
        val file = candidates.firstOrNull { it.isFile }
            ?: throw AssertionError("WebActivity.kt not found from ${File(".").absolutePath}")
        return file.readText()
    }
}
