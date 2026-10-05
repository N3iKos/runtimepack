package io.runtimepack.host

import android.view.KeyEvent
import android.view.MotionEvent
import io.runtimepack.host.WebActivity.Companion.ControlHitbox
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class WebActivityTest {

    @Test
    fun testParseControlHitboxes() {
        val json = """[{"id":"btn_a","left":100.0,"top":200.0,"right":150.0,"bottom":250.0}]"""
        val hitboxes = WebActivity.parseControlHitboxes(json)
        assertEquals(1, hitboxes.size)
        assertEquals(100.0f, hitboxes[0].left)
        assertEquals(200.0f, hitboxes[0].top)
        assertEquals(150.0f, hitboxes[0].right)
        assertEquals(250.0f, hitboxes[0].bottom)

        assertTrue(WebActivity.parseControlHitboxes("").isEmpty())
        assertTrue(WebActivity.parseControlHitboxes("invalid json").isEmpty())
    }

    @Test
    fun testHitboxContainment() {
        val box = ControlHitbox("btn", 100f, 100f, 200f, 200f)
        assertTrue(box.containsWithMargin(150f, 150f, 0f, 1.0f))
        assertTrue(box.containsWithMargin(95f, 95f, 10f, 1.0f))
        assertFalse(box.containsWithMargin(250f, 250f, 10f, 1.0f))
    }

    @Test
    fun testGestureOwnershipThroughMoveUpAndCancel() {
        val hitboxes = listOf(ControlHitbox("dpad", 50f, 50f, 200f, 200f))

        val capturedDownOnHitbox = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_DOWN,
            x = 100f,
            y = 100f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            wasCapturedByGame = false,
            controlHitboxes = hitboxes
        )
        assertFalse(capturedDownOnHitbox)

        val updatedHitboxes = listOf(ControlHitbox("dpad", 300f, 300f, 450f, 450f))
        val capturedMoveOnHitbox = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_MOVE,
            x = 100f,
            y = 100f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            wasCapturedByGame = capturedDownOnHitbox,
            controlHitboxes = updatedHitboxes
        )
        assertFalse(capturedMoveOnHitbox)

        val capturedUpOnHitbox = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_UP,
            x = 100f,
            y = 100f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            wasCapturedByGame = capturedDownOnHitbox,
            controlHitboxes = updatedHitboxes
        )
        assertFalse(capturedUpOnHitbox)

        val capturedDownOnGame = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_DOWN,
            x = 800f,
            y = 400f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            wasCapturedByGame = false,
            controlHitboxes = hitboxes
        )
        assertTrue(capturedDownOnGame)

        val capturedMoveOnGame = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_MOVE,
            x = 100f,
            y = 100f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            wasCapturedByGame = capturedDownOnGame,
            controlHitboxes = hitboxes
        )
        assertTrue(capturedMoveOnGame)
    }

    @Test
    fun testModalAndGamepadHiddenOverrides() {
        val hitboxes = listOf(ControlHitbox("btn", 100f, 100f, 200f, 200f))

        val capturedWithModal = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_DOWN,
            x = 800f,
            y = 400f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = true,
            wasCapturedByGame = false,
            controlHitboxes = hitboxes
        )
        assertFalse(capturedWithModal)

        val capturedGamepadHidden = WebActivity.resolveGestureCapturedByGame(
            actionMasked = MotionEvent.ACTION_DOWN,
            x = 150f,
            y = 150f,
            screenWidth = 1280f,
            screenHeight = 720f,
            density = 1.0f,
            isGamepadHidden = true,
            isOverlayModalOpen = false,
            wasCapturedByGame = false,
            controlHitboxes = hitboxes
        )
        assertTrue(capturedGamepadHidden)
    }

    @Test
    fun testAndroidKeyToJsKeyCode() {
        assertEquals(65, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_A))
        assertEquals(90, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_Z))
        assertEquals(48, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_0))
        assertEquals(57, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_9))
        assertEquals(38, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_UP))
        assertEquals(40, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_DOWN))
        assertEquals(37, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_LEFT))
        assertEquals(39, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_DPAD_RIGHT))
        assertEquals(13, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_ENTER))
        assertEquals(27, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_ESCAPE))
        assertEquals(32, WebActivity.androidKeyToJsKeyCode(KeyEvent.KEYCODE_SPACE))
    }

    private class TestableWebActivity : WebActivity() {
        val runnableQueue = mutableListOf<Runnable>()
        var mockGameFocusCalls = 0
        var mockOverlayFocusCalls = 0
        var mockCancelCalls = 0
        val dispatchedKeys = mutableListOf<Pair<Int, String>>()
        val dispatchedGameTouchEvents = mutableListOf<MotionEvent>()

        override fun dispatchTouchEventToGame(event: MotionEvent): Boolean {
            dispatchedGameTouchEvents.add(event)
            return true
        }

        override fun postFeedbackRunnable(action: Runnable, delayMs: Long) {
            runnableQueue.add(action)
        }

        override fun dispatchUiRunnable(action: Runnable) {
            runnableQueue.add(action)
        }

        override fun dispatchTouchEvent(event: MotionEvent): Boolean {
            return try {
                super.dispatchTouchEvent(event)
            } catch (_: NullPointerException) {
                false
            }
        }

        override fun dispatchCancelToGame() {
            mockCancelCalls++
        }

        override fun dispatchKeyToGame(jsKeyCode: Int, type: String) {
            if (type == "keydown") {
                activePressedKeys.add(jsKeyCode)
            } else if (type == "keyup") {
                activePressedKeys.remove(jsKeyCode)
            }
            dispatchedKeys.add(Pair(jsKeyCode, type))
        }

        override fun performGameFocus() {
            mockGameFocusCalls++
        }

        override fun performOverlayFocus() {
            mockOverlayFocusCalls++
        }

        fun flushRunnables() {
            val copy = ArrayList(runnableQueue)
            runnableQueue.clear()
            copy.forEach { it.run() }
        }

        public override fun onPause() {
            super.onPause()
        }
    }

    @Test
    fun testProductionFocusSchedulingAndMonotonicGeneration() {
        val activity = TestableWebActivity()
        activity.isActivityForeground = true
        activity.isOverlayModalOpen = false
        activity.overlayTextFocused = false

        activity.requestGameFocusSafe()
        assertEquals(1, activity.runnableQueue.size)

        activity.onOverlayModalOpenChanged(true)
        assertEquals(1, activity.runnableQueue.size)

        activity.flushRunnables()
        assertEquals(0, activity.mockGameFocusCalls)
        assertEquals(0, activity.mockOverlayFocusCalls)

        activity.onOverlayModalOpenChanged(false)
        assertEquals(1, activity.runnableQueue.size)
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        activity.isActivityForeground = false
        activity.requestGameFocusSafe()
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        activity.isActivityForeground = true
        activity.onOverlayTextFocusChanged(true)
        activity.flushRunnables()
        assertEquals(1, activity.mockOverlayFocusCalls)

        activity.requestGameFocusSafe()
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        activity.onOverlayTextFocusChanged(false)
        activity.flushRunnables()
        assertEquals(2, activity.mockGameFocusCalls)
    }

    @Test
    fun testGamepadEyeHideAndReleaseKeys() {
        val activity = TestableWebActivity()
        activity.isActivityForeground = true
        activity.isOverlayModalOpen = false
        activity.overlayTextFocused = false

        activity.dispatchKeyToGame(90, "keydown")
        assertTrue(activity.activePressedKeys.contains(90))

        val bridge = activity.createVPadBridge()
        bridge.setGamepadHidden(true)
        assertTrue(activity.isGamepadHidden)
        assertFalse(activity.activePressedKeys.contains(90))
        assertTrue(activity.dispatchedKeys.contains(Pair(90, "keyup")))

        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        bridge.setGamepadHidden(false)
        assertFalse(activity.isGamepadHidden)
        activity.flushRunnables()
        assertEquals(2, activity.mockGameFocusCalls)
    }

    @Test
    fun testModalOpenReleasesHeldKeys() {
        val activity = TestableWebActivity()
        activity.isActivityForeground = true
        activity.isOverlayModalOpen = false
        activity.overlayTextFocused = false

        activity.dispatchKeyToGame(88, "keydown")
        assertTrue(activity.activePressedKeys.contains(88))

        activity.onOverlayModalOpenChanged(true)
        assertTrue(activity.isOverlayModalOpen)
        assertFalse(activity.activePressedKeys.contains(88))
        assertTrue(activity.dispatchedKeys.contains(Pair(88, "keyup")))
        assertEquals(0, activity.mockOverlayFocusCalls)

        activity.onOverlayModalOpenChanged(false)
        assertFalse(activity.isOverlayModalOpen)
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)
    }

    @Test
    fun testCapturedGestureCancelLifecycle() {
        val activity = TestableWebActivity()
        activity.isActivityForeground = true

        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertTrue(activity.isTouchCapturedByGame)
        assertEquals(1L, activity.gestureGeneration)

        activity.cancelCapturedGameGesture()
        activity.flushRunnables()
        assertEquals(1, activity.mockCancelCalls)
        assertFalse(activity.isTouchCapturedByGame)

        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertTrue(activity.isTouchCapturedByGame)
        assertEquals(2L, activity.gestureGeneration)

        activity.cancelCapturedGameGesture()
        activity.cancelCapturedGameGesture()
        assertEquals(1, activity.runnableQueue.size)
        activity.flushRunnables()
        assertEquals(2, activity.mockCancelCalls)
        assertFalse(activity.isTouchCapturedByGame)

        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertTrue(activity.isTouchCapturedByGame)
        assertEquals(3L, activity.gestureGeneration)

        activity.cancelCapturedGameGesture()
        assertEquals(1, activity.runnableQueue.size)
        activity.handleTouchEvent(MotionEvent.ACTION_UP, 500f, 300f)
        assertFalse(activity.isTouchCapturedByGame)
        activity.flushRunnables()
        assertEquals(2, activity.mockCancelCalls)

        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertTrue(activity.isTouchCapturedByGame)
        assertEquals(4L, activity.gestureGeneration)

        activity.cancelCapturedGameGesture()
        assertEquals(1, activity.runnableQueue.size)
        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertEquals(5L, activity.gestureGeneration)
        assertTrue(activity.isTouchCapturedByGame)

        activity.flushRunnables()
        assertEquals(2, activity.mockCancelCalls)
        assertTrue(activity.isTouchCapturedByGame)

        val bridge = activity.createVPadBridge()
        bridge.setGamepadHidden(true)
        activity.flushRunnables()
        assertEquals(3, activity.mockCancelCalls)
        assertFalse(activity.isTouchCapturedByGame)

        activity.handleTouchEvent(MotionEvent.ACTION_DOWN, 500f, 300f)
        assertTrue(activity.isTouchCapturedByGame)
        activity.onPause()
        activity.flushRunnables()
        assertEquals(4, activity.mockCancelCalls)
        assertFalse(activity.isTouchCapturedByGame)

        activity.onOverlayModalOpenChanged(true)
        activity.flushRunnables()
        assertEquals(4, activity.mockCancelCalls)

        bridge.setGamepadHidden(false)
        activity.flushRunnables()
        assertEquals(4, activity.mockCancelCalls)

        activity.onPause()
        activity.flushRunnables()
        assertEquals(4, activity.mockCancelCalls)
    }

    @Test
    fun testTextFocusExitSingleGenerationAndModalReopen() {
        val activity = TestableWebActivity()
        activity.isActivityForeground = true

        activity.onOverlayTextFocusChanged(true)
        assertTrue(activity.overlayTextFocused)
        activity.flushRunnables()
        assertEquals(1, activity.mockOverlayFocusCalls)

        activity.onOverlayTextFocusChanged(false)
        assertFalse(activity.overlayTextFocused)
        assertEquals(1, activity.runnableQueue.size)
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        activity.onOverlayTextFocusChanged(true)
        activity.flushRunnables()
        assertEquals(2, activity.mockOverlayFocusCalls)

        activity.onOverlayTextFocusChanged(false)
        assertEquals(1, activity.runnableQueue.size)

        activity.onOverlayModalOpenChanged(true)
        assertTrue(activity.isOverlayModalOpen)
        activity.flushRunnables()
        assertEquals(1, activity.mockGameFocusCalls)

        activity.onOverlayModalOpenChanged(false)
        assertFalse(activity.isOverlayModalOpen)
        activity.flushRunnables()
        assertEquals(2, activity.mockGameFocusCalls)
    }

    @Test
    fun testBuildKeyDispatchScriptBubblingAndStateInjection() {
        val activity = TestableWebActivity()
        val script = activity.buildKeyDispatchScript(65, "keydown")

        assertTrue(script.contains("document.dispatchEvent(evt)"))
        assertFalse(script.contains("window.dispatchEvent(evt)"))
        assertTrue(script.contains("bubbles: true"))
        assertTrue(script.contains("get: function() { return 65; }"))
        assertTrue(script.contains("window.Input._onKeyDown(evt)"))
        assertTrue(script.contains("window.RuntimePackInput.press(action)"))
    }

    @Test
    fun testGetRuntimeBuildInfoStructure() {
        val activity = WebActivity()
        val buildInfo = activity.getRuntimeBuildInfo()
        assertTrue(buildInfo.contains("\"runtime_integrity_sha256\""))
        assertTrue(buildInfo.contains("\"overlay_mode\""))
        assertTrue(buildInfo.contains("\"vnm_input_guard\""))
        val parsed = MiniJson.parseObject(buildInfo)
        org.junit.Assert.assertNotNull(parsed)
    }

    @Test
    fun testPassiveOverlayDirectTouchDispatch() {
        val activity = TestableWebActivity()
        activity.isPassiveOverlay = true
        activity.isOverlayModalOpen = true
        activity.overlayTextFocused = true
        activity.isGamepadHidden = true
        activity.activeControlHitboxes = listOf(ControlHitbox("blocker", 0f, 0f, 2000f, 2000f))

        val unsafeClass = Class.forName("sun.misc.Unsafe")
        val theUnsafeField = unsafeClass.getDeclaredField("theUnsafe")
        theUnsafeField.isAccessible = true
        val unsafe = theUnsafeField.get(null)
        val allocateMethod = unsafeClass.getMethod("allocateInstance", Class::class.java)

        val down = allocateMethod.invoke(unsafe, MotionEvent::class.java) as MotionEvent
        val move = allocateMethod.invoke(unsafe, MotionEvent::class.java) as MotionEvent
        val up = allocateMethod.invoke(unsafe, MotionEvent::class.java) as MotionEvent
        val cancel = allocateMethod.invoke(unsafe, MotionEvent::class.java) as MotionEvent

        assertTrue(activity.dispatchTouchEvent(down))
        assertTrue(activity.dispatchTouchEvent(move))
        assertTrue(activity.dispatchTouchEvent(up))
        assertTrue(activity.dispatchTouchEvent(cancel))

        assertEquals(4, activity.dispatchedGameTouchEvents.size)
        assertSame(down, activity.dispatchedGameTouchEvents[0])
        assertSame(move, activity.dispatchedGameTouchEvents[1])
        assertSame(up, activity.dispatchedGameTouchEvents[2])
        assertSame(cancel, activity.dispatchedGameTouchEvents[3])

        assertEquals(0, activity.runnableQueue.size)
        assertEquals(0, activity.dispatchedKeys.size)
        assertFalse(activity.isTouchCapturedByGame)
    }

    @Test
    fun testFullModeBoundedMoveFeedbackCoalescing() {
        val activity = TestableWebActivity()
        activity.isPassiveOverlay = false

        activity.forwardFeedbackSample(MotionEvent.ACTION_DOWN, 100f, 100f)
        assertEquals(0, activity.runnableQueue.size)

        activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, 105f, 105f)

        for (i in 1..10) {
            activity.forwardFeedbackSample(MotionEvent.ACTION_MOVE, (105 + i).toFloat(), (105 + i).toFloat())
        }

        assertTrue(activity.runnableQueue.size <= 1)
        assertEquals(Pair(115, 115), activity.pendingMoveCssSample)

        activity.forwardFeedbackSample(MotionEvent.ACTION_UP, 115f, 115f)
        assertNull(activity.pendingMoveFeedbackRunnable)
        assertNull(activity.pendingMoveCssSample)
    }

    @Test
    fun testHistoricalManifestMissingOverlayModeDefaultsToFull() {
        val jsonHistorical = """
            {
                "schema_version": 1,
                "app": { "name": "Test", "package": "com.test", "version_name": "1.0", "version_code": 1 },
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1, "capabilities": [] },
                "content": { "mode": "packed" }
            }
        """.trimIndent()

        val manifest = BuildManifestLoader.parse(jsonHistorical)
        assertEquals("full", manifest.overlayMode)

        val jsonPassive = """
            {
                "schema_version": 1,
                "app": { "name": "Test", "package": "com.test", "version_name": "1.0", "version_code": 1 },
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1, "capabilities": [] },
                "content": { "mode": "packed" },
                "overlay_mode": "passive-watermark"
            }
        """.trimIndent()

        val manifestPassive = BuildManifestLoader.parse(jsonPassive)
        assertEquals("passive-watermark", manifestPassive.overlayMode)
    }

    @Test
    fun testBuildManifestLoaderFailClosedOnMissingRequiredFields() {
        val missingApp = """
            {
                "schema_version": 1,
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1 },
                "content": { "mode": "packed" }
            }
        """.trimIndent()
        try {
            BuildManifestLoader.parse(missingApp)
            org.junit.Assert.fail("Expected IllegalArgumentException for missing 'app'")
        } catch (e: IllegalArgumentException) {
            org.junit.Assert.assertTrue(e.message?.contains("app") == true)
        }

        val missingRuntime = """
            {
                "schema_version": 1,
                "app": { "name": "Test", "package": "com.test", "version_name": "1.0", "version_code": 1 },
                "content": { "mode": "packed" }
            }
        """.trimIndent()
        try {
            BuildManifestLoader.parse(missingRuntime)
            org.junit.Assert.fail("Expected IllegalArgumentException for missing 'runtime'")
        } catch (e: IllegalArgumentException) {
            org.junit.Assert.assertTrue(e.message?.contains("runtime") == true)
        }

        val missingContent = """
            {
                "schema_version": 1,
                "app": { "name": "Test", "package": "com.test", "version_name": "1.0", "version_code": 1 },
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1 }
            }
        """.trimIndent()
        try {
            BuildManifestLoader.parse(missingContent)
            org.junit.Assert.fail("Expected IllegalArgumentException for missing 'content'")
        } catch (e: IllegalArgumentException) {
            org.junit.Assert.assertTrue(e.message?.contains("content") == true)
        }

        val missingAppName = """
            {
                "schema_version": 1,
                "app": { "package": "com.test", "version_name": "1.0", "version_code": 1 },
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1 },
                "content": { "mode": "packed" }
            }
        """.trimIndent()
        try {
            BuildManifestLoader.parse(missingAppName)
            org.junit.Assert.fail("Expected IllegalArgumentException for missing 'app.name'")
        } catch (e: IllegalArgumentException) {
            org.junit.Assert.assertTrue(e.message?.contains("app.name") == true)
        }

        val missingAppPkg = """
            {
                "schema_version": 1,
                "app": { "name": "Test", "version_name": "1.0", "version_code": 1 },
                "runtime": { "id": "web", "version": "1.0", "family": "Web", "runtime_api": 1 },
                "content": { "mode": "packed" }
            }
        """.trimIndent()
        try {
            BuildManifestLoader.parse(missingAppPkg)
            org.junit.Assert.fail("Expected IllegalArgumentException for missing 'app.package'")
        } catch (e: IllegalArgumentException) {
            org.junit.Assert.assertTrue(e.message?.contains("app.package") == true)
        }
    }

    @Test
    fun testAuthenticProductionVnmManifestFixture() {
        val productionManifestFixture = """
            {
              "schema_version": 1,
              "runtimepack_version": "0.1.0",
              "git_commit": null,
              "build_timestamp": "2026-10-05T01:13:05.803991700+00:00",
              "flavor": "beta",
              "audit": {
                "report_path": "asset-audit-report.json",
                "report_sha256": "0f500255189a11056d78720b28048debea6430e0288addb8cf7e2a1684ea3485",
                "mode": "STANDARD",
                "outcome": "INDETERMINATE",
                "overrides": [
                  {
                    "flag": "--accept-audit-unknown",
                    "original_outcome": "INDETERMINATE",
                    "accepted_finding_ids": [
                      "CMP-AUD-006-1",
                      "CMP-AUD-006-2"
                    ],
                    "rationale": "explicitly accepted incomplete or unknown static audit coverage"
                  }
                ]
              },
              "app": {
                "name": "Time for Punishment Class ~Taking Lessons for My Boyfriend~",
                "package": "io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend",
                "version_name": "1.0.0",
                "version_code": 1
              },
              "engine": {
                "id": "Visual Novel Maker",
                "confidence": "EXACT",
                "overridden": false,
                "detected_version": null
              },
              "runtime": {
                "id": "vnmaker-web",
                "version": "0.1.0",
                "family": "VNM Web",
                "runtime_api": 1,
                "capabilities": [
                  "fs.vfs.read",
                  "fs.save.write",
                  "input.keyboard",
                  "input.pointer",
                  "web.local_origin",
                  "web.javascript",
                  "web.webgl",
                  "web.webaudio"
                ]
              },
              "content": {
                "mode": "packed",
                "content_hash": "21079d20c44e2f38df7afc0eeca978efe148593e0539140b944ff40b5d60df60",
                "file_count": 1976,
                "total_bytes": 1385946515
              },
              "android": {
                "abis": [
                  "arm64-v8a",
                  "x86_64"
                ],
                "min_sdk": 26,
                "target_sdk": 34,
                "permissions": []
              },
              "provenance": {
                "source_hash": "dcfd5c07c2d19ebd55d806069cde077ec70b53f09344eb66440ada380c1b2207",
                "config_hash": "",
                "apk_hash": "227cbb029d04c3c2f58f99deec0cb6f6fb2322fce5dd30158c6500a86ccd59e3",
                "flavor": "beta"
              }
            }
        """.trimIndent()

        val manifest = BuildManifestLoader.parse(productionManifestFixture)
        assertEquals("Time for Punishment Class ~Taking Lessons for My Boyfriend~", manifest.app.name)
        assertEquals("io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend", manifest.app.packageName)
        assertEquals("1.0.0", manifest.app.versionName)
        assertEquals(1, manifest.app.versionCode)

        assertEquals("vnmaker-web", manifest.runtime.id)
        assertEquals("0.1.0", manifest.runtime.version)
        assertEquals("VNM Web", manifest.runtime.family)
        assertEquals(1, manifest.runtime.runtimeApi)
        assertEquals(8, manifest.runtime.capabilities.size)
        org.junit.Assert.assertTrue(manifest.runtime.capabilities.contains("web.webaudio"))

        org.junit.Assert.assertNotNull(manifest.engine)
        assertEquals("Visual Novel Maker", manifest.engine!!.id)
        assertEquals("EXACT", manifest.engine!!.confidence)
        assertEquals(false, manifest.engine!!.overridden)
        org.junit.Assert.assertNull(manifest.engine!!.detectedVersion)

        assertEquals("packed", manifest.contentMode)
        assertEquals("21079d20c44e2f38df7afc0eeca978efe148593e0539140b944ff40b5d60df60", manifest.contentHash)

        assertEquals("dcfd5c07c2d19ebd55d806069cde077ec70b53f09344eb66440ada380c1b2207", manifest.sourceHash)
        assertEquals("beta", manifest.flavor)
        assertEquals("2026-10-05T01:13:05.803991700+00:00", manifest.buildTimestamp)
        assertEquals("full", manifest.overlayMode)
    }
}
