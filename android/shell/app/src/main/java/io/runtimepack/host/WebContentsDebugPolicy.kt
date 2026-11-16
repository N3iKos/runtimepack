package io.runtimepack.host

import android.content.pm.ApplicationInfo

/**
 * Decides whether WebView remote debugging (chrome://inspect) may be enabled.
 *
 * [android.webkit.WebView.setWebContentsDebuggingEnabled] is a process-global
 * switch, so every host must set it explicitly instead of relying on defaults:
 * allowed only for beta builds that are also installed debuggable, denied for
 * release, non-debuggable and unknown/missing flavors.
 */
object WebContentsDebugPolicy {

    const val BETA_FLAVOR = "beta"

    fun shouldEnableDebugging(buildFlavor: String?, applicationFlags: Int): Boolean {
        if (buildFlavor != BETA_FLAVOR) return false
        return applicationFlags and ApplicationInfo.FLAG_DEBUGGABLE != 0
    }

    /**
     * Applies the policy through [setEnabled] and always invokes it, so a denied
     * host explicitly turns debugging off instead of leaving a stale process-wide
     * `true` behind.
     */
    fun applyTo(buildFlavor: String?, applicationFlags: Int, setEnabled: (Boolean) -> Unit): Boolean {
        val enabled = shouldEnableDebugging(buildFlavor, applicationFlags)
        setEnabled(enabled)
        return enabled
    }
}
