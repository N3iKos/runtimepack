package io.runtimepack.host

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.webkit.WebView

open class WebExternalUrlDispatcher(
    private val queueToUi: (() -> Unit) -> Boolean,
    private val isAlive: () -> Boolean = { true },
    private val launcher: (normalizedUrl: String, host: String) -> Unit
) {
    /**
     * Attempts to validate and queue an external URL for dispatch.
     *
     * @return true if URL is valid AND successfully queued for UI dispatch.
     *         false if URL is invalid, Activity is dead, or queueing failed.
     */
    fun dispatch(url: String): Boolean {
        if (!isAlive()) {
            return false
        }

        val normalizedUrl = WebExternalUrlPolicy.validateAndNormalize(url)
        if (normalizedUrl == null) {
            DiagnosticLogStore.recordError(
                message = "[${WebExternalUrlDiagnostics.CODE_INVALID_URL}] Invalid external URL rejected by policy",
                kind = "invalid_url",
                source = "URL_BRIDGE"
            )
            return false
        }

        val parsedHost = try {
            java.net.URI(normalizedUrl).host ?: "unknown"
        } catch (_: Exception) {
            "unknown"
        }

        val queued = queueToUi {
            if (!isAlive()) {
                DiagnosticLogStore.recordError(
                    message = "[${WebExternalUrlDiagnostics.CODE_DISPATCH_FAILED}] Activity destroyed before external URL dispatch executed for host: $parsedHost",
                    kind = "lifecycle_destroyed",
                    source = "URL_BRIDGE"
                )
                return@queueToUi
            }

            try {
                launcher(normalizedUrl, parsedHost)
                DiagnosticLogStore.recordLog(
                    level = "INFO",
                    message = "[CAP-WEB-030] Dispatched external URL to host: $parsedHost",
                    source = "URL_BRIDGE"
                )
            } catch (e: ActivityNotFoundException) {
                DiagnosticLogStore.recordError(
                    message = "[${WebExternalUrlDiagnostics.CODE_NO_HANDLER}] No browser activity found to handle external URL host: $parsedHost",
                    kind = "activity_not_found",
                    source = "URL_BRIDGE"
                )
            } catch (e: Exception) {
                DiagnosticLogStore.recordError(
                    message = "[${WebExternalUrlDiagnostics.CODE_DISPATCH_FAILED}] Failed to dispatch external URL host: $parsedHost (${e.javaClass.simpleName})",
                    kind = "url_dispatch_error",
                    source = "URL_BRIDGE"
                )
            }
        }

        return queued
    }

    companion object {
        fun forView(view: WebView): WebExternalUrlDispatcher {
            val context = view.context
            val activity = context as? Activity
            return WebExternalUrlDispatcher(
                queueToUi = { runnable -> view.post(runnable) },
                isAlive = {
                    if (activity != null) !activity.isFinishing && !activity.isDestroyed else true
                },
                launcher = { normalizedUrl, host ->
                    val parsedAndroid = Uri.parse(normalizedUrl)
                    if (!WebExternalUrlPolicy.verifyAndroidUriParity(
                            normalizedUrl,
                            parsedAndroid.scheme,
                            parsedAndroid.host,
                            parsedAndroid.port
                        )
                    ) {
                        DiagnosticLogStore.recordLog(
                            level = "WARN",
                            message = "[AND-SEC-002] URI parser differential detected; rejecting URL host: $host",
                            source = "URL_BRIDGE"
                        )
                        return@WebExternalUrlDispatcher
                    }

                    val intent = Intent(Intent.ACTION_VIEW, parsedAndroid).apply {
                        addCategory(Intent.CATEGORY_BROWSABLE)
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(intent)
                }
            )
        }
    }
}
