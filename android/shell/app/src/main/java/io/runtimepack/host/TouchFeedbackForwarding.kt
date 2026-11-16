package io.runtimepack.host

import android.view.MotionEvent

/**
 * Native host state needed to forward game-owned touch samples to the shared
 * overlay effects in `vpad.js` (`spawnTouchRipple`, `sampleTouchTrail`,
 * `endTouchTrail`).
 *
 * Both [WebActivity] and [RenpyActivity] implement this so the coalescing and
 * mode semantics live in one place instead of being duplicated per engine.
 */
interface TouchFeedbackHost {
    /** Runs a feedback script on the overlay WebView that owns the effects. */
    fun evaluateFeedbackScript(script: String)

    /** Schedules a coalesced MOVE sample; overridable so tests stay synchronous. */
    fun postFeedbackRunnable(action: Runnable, delayMs: Long)

    /** Cancels a previously scheduled coalesced sample. */
    fun removeFeedbackRunnable(action: Runnable)

    /** Monotonic clock used for MOVE throttling. */
    fun currentUptimeMs(): Long

    var lastMoveFeedbackDispatchTimeMs: Long
    var pendingMoveCssSample: Pair<Int, Int>?
    var pendingMoveFeedbackRunnable: Runnable?
    var moveFeedbackDispatchCount: Int
}

/**
 * Forwards touch samples captured by a game surface to the overlay effects.
 *
 * Semantics mirror the shipped overlay contract:
 * - `ACTION_DOWN` spawns one ripple and resets any pending MOVE sample,
 * - `ACTION_MOVE` is throttled to one sample per [MIN_MOVE_INTERVAL_MS] and
 *   coalesces the newest position in between,
 * - `ACTION_UP`/`ACTION_CANCEL` end the trail and drop queued samples.
 *
 * Coordinates are converted from physical pixels to CSS pixels, because the
 * effects position themselves against the overlay viewport.
 */
object TouchFeedbackForwarding {
    const val MIN_MOVE_INTERVAL_MS = 16L

    fun rippleScript(cssX: Int, cssY: Int): String =
        "if (window.spawnTouchRipple) { window.spawnTouchRipple($cssX, $cssY); }"

    fun trailScript(cssX: Int, cssY: Int): String =
        "if (window.sampleTouchTrail) { window.sampleTouchTrail($cssX, $cssY); }"

    fun endTrailScript(): String = "if (window.endTouchTrail) { window.endTouchTrail(); }"

    fun forward(host: TouchFeedbackHost, action: Int, x: Float, y: Float, density: Float) {
        val safeDensity = if (density > 0f && density.isFinite()) density else 1f
        val cssX = (x / safeDensity).toInt()
        val cssY = (y / safeDensity).toInt()

        when (action) {
            MotionEvent.ACTION_DOWN -> {
                clearPending(host)
                host.lastMoveFeedbackDispatchTimeMs = host.currentUptimeMs()
                host.evaluateFeedbackScript(rippleScript(cssX, cssY))
            }

            MotionEvent.ACTION_MOVE -> forwardMove(host, cssX, cssY)

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                clearPending(host)
                host.evaluateFeedbackScript(endTrailScript())
            }
        }
    }

    fun clearPending(host: TouchFeedbackHost) {
        val pending = host.pendingMoveFeedbackRunnable
        if (pending != null) {
            host.removeFeedbackRunnable(pending)
            host.pendingMoveFeedbackRunnable = null
        }
        host.pendingMoveCssSample = null
    }

    /** Drops queued samples and closes the trail so a paused/destroyed host leaves nothing behind. */
    fun reset(host: TouchFeedbackHost) {
        clearPending(host)
        host.evaluateFeedbackScript(endTrailScript())
    }

    private fun forwardMove(host: TouchFeedbackHost, cssX: Int, cssY: Int) {
        val now = host.currentUptimeMs()
        val elapsed = now - host.lastMoveFeedbackDispatchTimeMs
        if (elapsed >= MIN_MOVE_INTERVAL_MS && host.pendingMoveFeedbackRunnable == null) {
            host.lastMoveFeedbackDispatchTimeMs = now
            host.moveFeedbackDispatchCount++
            host.evaluateFeedbackScript(trailScript(cssX, cssY))
            return
        }

        host.pendingMoveCssSample = Pair(cssX, cssY)
        if (host.pendingMoveFeedbackRunnable != null) return

        val delay = (MIN_MOVE_INTERVAL_MS - elapsed).coerceAtLeast(1L)
        val runnable = Runnable {
            val sample = host.pendingMoveCssSample
            host.pendingMoveFeedbackRunnable = null
            host.pendingMoveCssSample = null
            if (sample != null) {
                host.lastMoveFeedbackDispatchTimeMs = host.currentUptimeMs()
                host.moveFeedbackDispatchCount++
                host.evaluateFeedbackScript(trailScript(sample.first, sample.second))
            }
        }
        host.pendingMoveFeedbackRunnable = runnable
        host.postFeedbackRunnable(runnable, delay)
    }
}
