# Implementation Plan: Track 3 — Ren'Py Overlay Integration & 4 Pillars Universal RPG Maker VFS

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the transparent UI overlay, virtual gamepad, and precision touch routing into `RenpyActivity.kt`, and implement the 4 Pillars of Universal VFS for RPG Maker MV/MZ to eliminate `readdirSync` runtime crashes, O(N) asset interception bottlenecks, and false-positive diagnostic probe failures.

**Architecture:** 
1. **Ren'Py Overlay Subsystem**: Attach a transparent `overlayWebView` to `RenpyActivity`'s `mLayout`, register `RuntimePackVPad` with SDL key injection and dynamic `ControlHitbox` registration, and implement selective touch event filtering so taps on the visual novel surface reach SDL while button taps trigger virtual controls.
2. **RPG Maker Universal VFS Subsystem**: Connect `fs.readdirSync`, `fs.existsSync`, and `fs.statSync` in `runtimepack_web_shims.js` to native `GpackStreamReader` in-memory tables via `RuntimePackNative`, replace linear asset lookups in `VfsResourceInterceptor.kt` with O(1) lower-case HashMap lookups, make `AssetProbeRunner.kt` GPACK-aware, and fix compiler relative path truncation in `core/src/asset_audit/report.rs`.

**Tech Stack:** Kotlin (Android Shell, SDLActivity, WebView, VFS Interceptor), JavaScript (Web Shims, DOM VPad Bridge), Rust (Cargo workspace, `core/src/asset_audit/report.rs`), Android Gradle (API 34).

**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/design.md` §4 and §5.

## Global Constraints

- Source games in `Raw-Game/` remain strictly 100% immutable.
- All builds and packaging use `--flavor beta`.
- App-specific internal storage (`filesDir`) only; no broad external storage permissions (`MANAGE_EXTERNAL_STORAGE`).
- All changes must be backed by automated unit tests before any emulator verification.
- Zero placeholder steps; every task contains complete code and test commands.

---

### Task 1: Add Hitbox and Touch Filtering Models to RenpyActivity

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`

**Interfaces:**
- Consumes: `ControlHitbox` definition from `WebActivity.kt`.
- Produces: `RenpyActivity.Companion.shouldForwardTouchToGame` and `resolveGestureCapturedByGame` pure functions for Ren'Py SDL surface hit-testing.

- [ ] **Step 1: Write the failing unit tests for Ren'Py touch filtering**

Add the following tests to `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`:

```kotlin
    @Test
    fun testShouldForwardTouchToRenpyGameOutsideControls() {
        val hitboxes = listOf(
            ControlHitbox("btn-a", 800f, 400f, 900f, 500f)
        )
        // Touch outside any control -> should forward to Ren'Py game surface
        val forwardOutside = RenpyActivity.shouldForwardTouchToGame(
            x = 200f,
            y = 200f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            controlHitboxes = hitboxes
        )
        assertTrue(forwardOutside)

        // Touch inside btn-a -> should NOT forward to Ren'Py game surface
        val forwardInside = RenpyActivity.shouldForwardTouchToGame(
            x = 850f,
            y = 450f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = false,
            controlHitboxes = hitboxes
        )
        assertFalse(forwardInside)

        // When overlay modal is open -> NEVER forward to game
        val forwardModalOpen = RenpyActivity.shouldForwardTouchToGame(
            x = 200f,
            y = 200f,
            screenWidth = 1000f,
            screenHeight = 600f,
            density = 2.0f,
            isGamepadHidden = false,
            isOverlayModalOpen = true,
            controlHitboxes = hitboxes
        )
        assertFalse(forwardModalOpen)
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.testShouldForwardTouchToRenpyGameOutsideControls"
```
Expected: FAIL with compilation error (unresolved reference: `shouldForwardTouchToGame`).

- [ ] **Step 3: Implement pure touch hit-testing helper on RenpyActivity.Companion**

In `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`, add to `companion object`:

```kotlin
        @JvmStatic
        fun shouldForwardTouchToGame(
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            if (isOverlayModalOpen) {
                return false
            }
            return !WebActivity.isTouchOnActiveControls(
                x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen, controlHitboxes
            )
        }

        @JvmStatic
        fun resolveGestureCapturedByGame(
            actionMasked: Int,
            x: Float,
            y: Float,
            screenWidth: Float,
            screenHeight: Float,
            density: Float,
            isGamepadHidden: Boolean,
            isOverlayModalOpen: Boolean,
            wasCapturedByGame: Boolean,
            controlHitboxes: List<ControlHitbox> = emptyList()
        ): Boolean {
            return if (actionMasked == android.view.MotionEvent.ACTION_DOWN) {
                shouldForwardTouchToGame(
                    x, y, screenWidth, screenHeight, density, isGamepadHidden, isOverlayModalOpen, controlHitboxes
                )
            } else {
                wasCapturedByGame
            }
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.testShouldForwardTouchToRenpyGameOutsideControls"
```
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt
git commit -m "feat(renpy): add pure touch hit-testing and control filtering logic"
```

---

### Task 2: Implement Overlay Attachment and Touch Routing in RenpyActivity

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`

**Interfaces:**
- Consumes: `mLayout`, `mSurface`, `onNativeKeyDown`, `onNativeKeyUp` from SDLActivity.
- Produces: `overlayWebView` attached to `mLayout`, `RuntimePackVPad` JavascriptInterface, and `dispatchTouchEvent` selective forwarding.

- [ ] **Step 1: Write the failing unit test for VPad key mapping to SDL keycodes**

Add test to `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`:

```kotlin
    @Test
    fun testRenpyKeyMappingForGamepadButtons() {
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_UP, RenpyActivity.mapVPadButtonToKeyCode("up"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_DOWN, RenpyActivity.mapVPadButtonToKeyCode("down"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_LEFT, RenpyActivity.mapVPadButtonToKeyCode("left"))
        assertEquals(android.view.KeyEvent.KEYCODE_DPAD_RIGHT, RenpyActivity.mapVPadButtonToKeyCode("right"))
        assertEquals(android.view.KeyEvent.KEYCODE_BUTTON_A, RenpyActivity.mapVPadButtonToKeyCode("a"))
        assertEquals(android.view.KeyEvent.KEYCODE_BUTTON_B, RenpyActivity.mapVPadButtonToKeyCode("b"))
        assertEquals(android.view.KeyEvent.KEYCODE_MENU, RenpyActivity.mapVPadButtonToKeyCode("menu"))
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.testRenpyKeyMappingForGamepadButtons"
```
Expected: FAIL with compilation error (unresolved reference: `mapVPadButtonToKeyCode`).

- [ ] **Step 3: Implement key mapping, overlay attachment, and dispatchTouchEvent in RenpyActivity**

In `RenpyActivity.kt`, add:

```kotlin
    var overlayWebView: android.webkit.WebView? = null
    var isGamepadHidden: Boolean = false
    var isOverlayModalOpen: Boolean = false
    var isTouchCapturedByGame: Boolean = false
    var activeControlHitboxes: List<ControlHitbox> = emptyList()

    companion object {
        // ... (existing code) ...
        @JvmStatic
        fun mapVPadButtonToKeyCode(button: String): Int {
            return when (button.lowercase()) {
                "up" -> android.view.KeyEvent.KEYCODE_DPAD_UP
                "down" -> android.view.KeyEvent.KEYCODE_DPAD_DOWN
                "left" -> android.view.KeyEvent.KEYCODE_DPAD_LEFT
                "right" -> android.view.KeyEvent.KEYCODE_DPAD_RIGHT
                "a", "ok" -> android.view.KeyEvent.KEYCODE_BUTTON_A
                "b", "cancel" -> android.view.KeyEvent.KEYCODE_BUTTON_B
                "menu" -> android.view.KeyEvent.KEYCODE_MENU
                "dash", "shift" -> android.view.KeyEvent.KEYCODE_SHIFT_LEFT
                else -> android.view.KeyEvent.KEYCODE_UNKNOWN
            }
        }
    }

    @android.annotation.SuppressLint("SetJavaScriptEnabled")
    fun setupOverlayWebView() {
        if (mLayout == null) return
        try {
            val wv = android.webkit.WebView(this)
            wv.layoutParams = android.widget.RelativeLayout.LayoutParams(
                android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                android.view.ViewGroup.LayoutParams.MATCH_PARENT
            )
            wv.setBackgroundColor(android.graphics.Color.TRANSPARENT)
            wv.isVerticalScrollBarEnabled = false
            wv.isHorizontalScrollBarEnabled = false
            wv.overScrollMode = android.view.View.OVER_SCROLL_NEVER

            val settings = wv.settings
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = true
            settings.allowContentAccess = true

            wv.addJavascriptInterface(object : Any() {
                @JavascriptInterface
                fun openCommunityLink() {
                    this@RenpyActivity.openCommunityLink()
                }

                @JavascriptInterface
                fun updateControlHitboxes(json: String) {
                    activeControlHitboxes = WebActivity.parseControlHitboxes(json)
                }

                @JavascriptInterface
                fun setGamepadHidden(hidden: Boolean) {
                    isGamepadHidden = hidden
                }

                @JavascriptInterface
                fun setOverlayModalOpen(open: Boolean) {
                    isOverlayModalOpen = open
                }

                @JavascriptInterface
                fun sendKeyEvent(button: String, isDown: Boolean) {
                    val keyCode = mapVPadButtonToKeyCode(button)
                    if (keyCode != android.view.KeyEvent.KEYCODE_UNKNOWN) {
                        if (isDown) {
                            onNativeKeyDown(keyCode)
                        } else {
                            onNativeKeyUp(keyCode)
                        }
                    }
                }
            }, "RuntimePackVPad")

            wv.loadUrl("file:///android_asset/overlay/index.html")
            mLayout.addView(wv)
            overlayWebView = wv
            mSurface?.requestFocus()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to attach overlay: ${e.message}", e)
        }
    }

    override fun dispatchTouchEvent(event: android.view.MotionEvent): Boolean {
        val density = resources.displayMetrics.density
        val screenWidth = resources.displayMetrics.widthPixels.toFloat()
        val screenHeight = resources.displayMetrics.heightPixels.toFloat()

        val action = event.actionMasked
        isTouchCapturedByGame = resolveGestureCapturedByGame(
            action,
            event.x,
            event.y,
            screenWidth,
            screenHeight,
            density,
            isGamepadHidden,
            isOverlayModalOpen,
            isTouchCapturedByGame,
            activeControlHitboxes
        )

        if (isTouchCapturedByGame) {
            try {
                mSurface?.dispatchTouchEvent(event)
            } catch (ignored: Exception) {}
            if (action == android.view.MotionEvent.ACTION_UP || action == android.view.MotionEvent.ACTION_CANCEL) {
                isTouchCapturedByGame = false
            }
            return true
        }

        return super.dispatchTouchEvent(event)
    }

    override fun onPause() {
        super.onPause()
        overlayWebView?.onPause()
    }

    override fun onResume() {
        super.onResume()
        overlayWebView?.onResume()
    }

    override fun onDestroy() {
        super.onDestroy()
        overlayWebView?.destroy()
        overlayWebView = null
    }
```

And in `onStart()`, call `setupOverlayWebView()` if `overlayWebView == null`.

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.*"
```
Expected: All tests in `RenpyActivityTest` PASS.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt
git commit -m "feat(renpy): attach transparent overlay and route non-control touch to SDL surface"
```

---

### Task 3: Implement Pilar 1 — In-Memory Native VFS Bridge `readdirSync`

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Consumes: `gpackReader?.fileIndexMap` in `WebBackend.kt` / `WebActivity.kt`.
- Produces: `@JavascriptInterface fun readdir(relPath: String): String` on `RuntimePackNative`, and `fs.readdirSync(dir)` in `runtimepack_web_shims.js`.

- [ ] **Step 1: Write the failing unit test for `readdirSync` in `web_shims.test.js`**

Add to `android/shell/app/src/test/js/web_shims.test.js`:

```javascript
{
    const sb = createSandbox();
    sb.RuntimePackNative = {
        readdir: function(dir) {
            if (dir === "data" || dir === "data/") {
                return JSON.stringify(["System.json", "Map001.json", "Map002.json"]);
            }
            if (dir === "img/pictures") {
                return JSON.stringify(["cg1.png", "cg2.png"]);
            }
            return "[]";
        }
    };
    vm.runInContext(shimsCode, sb);

    const fs = vm.runInContext("require('fs')", sb);
    const dataFiles = fs.readdirSync("data");
    assert(Array.isArray(dataFiles), "readdirSync must return an array");
    assert.strictEqual(dataFiles.length, 3, "readdirSync('data') must return 3 files");
    assert.strictEqual(dataFiles[0], "System.json");

    const pictures = fs.readdirSync("img/pictures");
    assert.strictEqual(pictures.length, 2);
    assert.strictEqual(pictures[1], "cg2.png");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: FAIL with `AssertionError: readdirSync('data') must return 3 files` (returned `[]`).

- [ ] **Step 3: Implement native `readdir` bridge and web shims integration**

In `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt` (in `WebBridge`), add:
```kotlin
    @JavascriptInterface
    fun readdir(relPath: String): String {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val prefix = if (clean.isEmpty()) "" else "$clean/"
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory

        val packEntries = reader?.fileIndexMap?.keys
            ?.filter { it.startsWith(prefix) }
            ?.map { it.removePrefix(prefix).substringBefore('/') }
            ?.distinct()

        val diskEntries = gameDir?.let {
            val dir = if (clean.isEmpty()) it else File(it, clean)
            if (dir.exists() && dir.isDirectory) dir.list()?.toList() else null
        }

        val result = (packEntries ?: diskEntries ?: emptyList()).distinct()
        return org.json.JSONArray(result).toString()
    }
```

In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
Update `readdirSync`:
```javascript
        readdirSync: function(dirPath) {
            var clean = String(dirPath || "").replace(/\\/g, "/").replace(/^\.?\//, "").replace(/\/$/, "");
            if (typeof root.window !== "undefined" && root.window.RuntimePackNative && typeof root.window.RuntimePackNative.readdir === "function") {
                try {
                    var res = root.window.RuntimePackNative.readdir(clean);
                    var parsed = JSON.parse(res);
                    if (Array.isArray(parsed) && parsed.length > 0) {
                        return parsed;
                    }
                } catch (e) {}
            }
            if (clean === "data" || clean === "") {
                var mapInfosStr = syncFetch("data/MapInfos.json");
                if (mapInfosStr) {
                    var list = ["Actors.json", "Classes.json", "CommonEvents.json", "Enemies.json", "Items.json", "MapInfos.json", "Skills.json", "States.json", "System.json", "Tilesets.json", "Troops.json", "Weapons.json", "Armors.json", "Animations.json"];
                    try {
                        var infos = JSON.parse(mapInfosStr);
                        if (Array.isArray(infos)) {
                            infos.forEach(function(m) {
                                if (m && typeof m.id === "number") {
                                    list.push("Map" + String(m.id).padStart(3, "0") + ".json");
                                }
                            });
                        }
                    } catch (e) {}
                    return list;
                }
            }
            return [];
        },
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "feat(vfs): implement Pilar 1 in-memory native readdirSync bridge with MapInfos fallback"
```

---

### Task 4: Implement Pilar 2 — Fast Stat and Exists Sync Bridge

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Consumes: `gpackReader?.hasFile()` and `gpackReader?.getEntry()?.size`.
- Produces: `RuntimePackNative.fileExists(path)` and `RuntimePackNative.fileSize(path)`, wired into `fs.existsSync` and `fs.statSync`.

- [ ] **Step 1: Write the failing unit test for `existsSync` and `statSync` without syncFetch**

Add to `android/shell/app/src/test/js/web_shims.test.js`:

```javascript
{
    const sb = createSandbox();
    let syncFetchCalled = false;
    sb.RuntimePackNative = {
        fileExists: function(path) { return path === "audio/bgm/Theme.ogg"; },
        fileSize: function(path) { return path === "audio/bgm/Theme.ogg" ? 5242880 : 0; }
    };
    vm.runInContext(shimsCode, sb);

    const fs = vm.runInContext("require('fs')", sb);
    assert.strictEqual(fs.existsSync("audio/bgm/Theme.ogg"), true);
    assert.strictEqual(fs.existsSync("audio/bgm/Missing.ogg"), false);

    const stat = fs.statSync("audio/bgm/Theme.ogg");
    assert.strictEqual(stat.isFile(), true);
    assert.strictEqual(stat.isDirectory(), false);
    assert.strictEqual(stat.size, 5242880);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: FAIL (attempted XHR syncFetch or threw error in Node environment without XMLHttpRequest).

- [ ] **Step 3: Implement native bridge and update `syncExists` and `statSync`**

In `WebBackend.kt` (`WebBridge`), add:
```kotlin
    @JavascriptInterface
    fun fileExists(relPath: String): Boolean {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.hasFile(clean) ?: File(gameDir, clean).exists()
    }

    @JavascriptInterface
    fun fileSize(relPath: String): Long {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.getEntry(clean)?.size ?: File(gameDir, clean).length()
    }
```

In `runtimepack_web_shims.js`:
Update `syncExists(p)`:
```javascript
    function syncExists(p) {
        if (!p) return false;
        var cleanPath = String(p).replace(/\\/g, "/").replace(/^\.?\//, "");
        if (typeof root.window !== "undefined" && root.window.RuntimePackNative && typeof root.window.RuntimePackNative.fileExists === "function") {
            try {
                return root.window.RuntimePackNative.fileExists(cleanPath);
            } catch (e) {}
        }
        // ... (existing XHR fallback) ...
    }
```
And in `fsShim`:
```javascript
        statSync: function(p) {
            var cleanPath = String(p || "").replace(/\\/g, "/").replace(/^\.?\//, "");
            var exists = syncExists(cleanPath);
            var size = 0;
            if (exists && typeof root.window !== "undefined" && root.window.RuntimePackNative && typeof root.window.RuntimePackNative.fileSize === "function") {
                try {
                    size = root.window.RuntimePackNative.fileSize(cleanPath);
                } catch (e) {}
            } else if (exists) {
                size = (syncFetch(p) || "").length;
            }
            return {
                isFile: function() { return exists; },
                isDirectory: function() { return false; },
                size: size
            };
        },
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "feat(vfs): implement Pilar 2 fast statSync and existsSync native bridges"
```

---

### Task 5: Implement Pilar 3 — O(1) Case-Insensitive Hash Map Resolver

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Consumes: `reader.getEntry()` from `GpackStreamReader.kt`.
- Produces: O(1) resolution in `intercept(url)` eliminating linear scans.

- [ ] **Step 1: Write the failing unit test for O(1) case-insensitive lookup in VfsResourceInterceptor**

Add to `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`:

```kotlin
    @Test
    fun testCaseInsensitiveHashResolutionWithoutLinearScan() {
        // Construct a mock stream provider and verify mixed-case paths resolve directly
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
```

- [ ] **Step 2: Run test to verify it passes/fails**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.VfsResourceInterceptorTest.testCaseInsensitiveHashResolutionWithoutLinearScan"
```

- [ ] **Step 3: Replace linear `firstOrNull` in `VfsResourceInterceptor.kt` with O(1) reader lookups**

In `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
1. Build `pluginFilenameMap: Map<String, String>` lazily from `reader.entries`:
```kotlin
    private val pluginFilenameMap: Map<String, String> by lazy {
        val map = HashMap<String, String>()
        reader?.entries?.forEach { entry ->
            val norm = entry.logicalPath.replace('\\', '/').removePrefix("www/")
            if (norm.startsWith("js/plugins/") && norm.endsWith(".js")) {
                val filename = norm.substringAfterLast('/').lowercase()
                map.putIfAbsent(filename, entry.logicalPath)
            }
        }
        map
    }
```
2. In `intercept(url)`:
Replace lines 172-179:
```kotlin
        if (stream == null && reader != null) {
            val entry = reader.getEntry(logicalPath)
            if (entry != null) {
                stream = streamProvider(entry.logicalPath)
            }
        }
```
Replace lines 198-208:
```kotlin
        if (stream == null && (logicalPath.startsWith("js/plugins/") || logicalPath.startsWith("www/js/plugins/")) && logicalPath.endsWith(".js")) {
            val filename = logicalPath.substringAfterLast('/').lowercase()
            val matchedLogicalPath = pluginFilenameMap[filename]
            if (matchedLogicalPath != null) {
                stream = streamProvider(matchedLogicalPath)
            }
        }
```
Replace lines 228-235:
```kotlin
        if (encStream == null && reader != null) {
            val entry = reader.getEntry(encPath)
            if (entry != null) {
                encStream = streamProvider(entry.logicalPath)
            }
        }
```

- [ ] **Step 4: Run test suite to verify regression passes**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.VfsResourceInterceptorTest.*"
```
Expected: PASS 100%.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
git commit -m "perf(vfs): replace O(N) linear scans with O(1) hash map resolver and pre-indexed plugin table"
```

---

### Task 6: Implement Pilar 4 — VFS-Aware Diagnostics & Path Sanitizer Fix

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Modify: `core/src/asset_audit/report.rs`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
- Test: Rust unit tests in `core/src/asset_audit/report.rs`

**Interfaces:**
- Consumes: `gpackReader?.hasFile()` and `gpackReader?.openStream()`.
- Produces: Correct `FILE_EXISTS` probing for packed APKs, and accurate script paths without stripped `"js/"` prefixes.

- [ ] **Step 1: Write failing Rust unit test for `sanitize_path_str` on `js/plugins/inazuma/Activity.js`**

In `core/src/asset_audit/report.rs`, add at the bottom under `#[cfg(test)]`:

```rust
    #[test]
    fn test_sanitize_path_str_preserves_js_plugins_prefix() {
        let input = "js/plugins/inazuma/Activity.js";
        let sanitized = sanitize_path_str(input);
        assert_eq!(sanitized, "js/plugins/inazuma/Activity.js");

        let deep_input = "www/js/plugins/inazuma/core/audio/WebAudioEx.js";
        let deep_sanitized = sanitize_path_str(deep_input);
        assert_eq!(deep_sanitized, "www/js/plugins/inazuma/core/audio/WebAudioEx.js");
    }
```

- [ ] **Step 2: Run cargo test to verify it fails**

Run:
```powershell
cargo test -p runtimepack-core test_sanitize_path_str_preserves_js_plugins_prefix
```
Expected: FAIL with `left == "plugins/inazuma/Activity.js", right == "js/plugins/inazuma/Activity.js"`.

- [ ] **Step 3: Fix `sanitize_path_str` in `core/src/asset_audit/report.rs`**

Replace lines 148-154 in `core/src/asset_audit/report.rs`:
```rust
    let segments: Vec<&str> = path_no_home.split('/').filter(|s| !s.is_empty()).collect();
    if let Some(pos) = segments.iter().position(|segment| {
        let lower = segment.to_ascii_lowercase();
        WELL_KNOWN_DIRS.contains(&lower.as_str())
    }) {
        return segments[pos..].join("/");
    }
```
*(Changing `rposition` to `position` ensures that the outermost game directory such as `www/` or `js/` is preserved rather than chopped by a nested match like `plugins/` or `audio/`).*

- [ ] **Step 4: Update `AssetProbeRunner.kt` to query `gpackReader`**

In `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`:
Pass `gpackReader: GpackStreamReader? = WebActivity.currentInstance?.gpackReader` to `AssetProbeRunner`.
At lines 114-142:
```kotlin
            val relPath = assetsToProbe[i]
            val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader
            val hasPackFile = reader?.hasFile(relPath) == true
            val file = File(gameDir, relPath)
            val exists = hasPackFile || file.exists()

            // Stage 1: FILE_EXISTS
            if (!exists) {
                val ev = RuntimeAuditEvent(
                    sequence_id = 0L,
                    stage = RuntimeAuditStage.FILE_EXISTS,
                    asset_path = relPath,
                    outcome = RuntimeAuditOutcome.FAIL,
                    error_message = "File not found: $relPath"
                )
                // ...
                continue
            }
            stageCounts["FILE_EXISTS"] = (stageCounts["FILE_EXISTS"] ?: 0) + 1

            // Stage 2: FILE_READABLE
            val fis: InputStream = try {
                val s = if (hasPackFile) {
                    reader?.openStream(relPath) ?: FileInputStream(file)
                } else {
                    FileInputStream(file)
                }
                stageCounts["FILE_READABLE"] = (stageCounts["FILE_READABLE"] ?: 0) + 1
                s
            } catch (e: Exception) {
                // ...
```

- [ ] **Step 5: Run tests to verify everything passes**

Run:
```powershell
cargo test -p runtimepack-core test_sanitize_path_str_preserves_js_plugins_prefix
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest
```
Expected: PASS 100%.

- [ ] **Step 6: Commit**

```powershell
git add core/src/asset_audit/report.rs android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt
git commit -m "fix(audit): make AssetProbeRunner GPACK-aware and fix path truncation in sanitize_path_str"
```

---

### Task 7: Full Verification on Rebuilt Forestia and Mizuki APKs

**Files:**
- Output: `dist/forestia/Forestia.apk`
- Output: `dist/mizuki_and_me/MizukiAndMe.apk`

- [ ] **Step 1: Rebuild Forestia APK**
  Run:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
  Verify Gradle exit 0.

- [ ] **Step 2: Verify Forestia on MuMu emulator**
  Run:
  ```powershell
  adb -s 127.0.0.1:16384 install -r dist/forestia/Forestia.apk
  ```
  Verify:
  1. Boot game, start New Game, walk into rooms / schedule triggers. Confirm zero `TypeError: Cannot read properties of undefined (reading 'getRoadRegion')`.
  2. Open Diagnostic Suite Beta from overlay, run probe. Confirm 0 false-positive "File not found" errors and correct `js/plugins/` paths in activity log.

- [ ] **Step 3: Rebuild Mizuki And Me APK**
  Run:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\Mizuki And Me" -o dist/mizuki_and_me --package com.cherrykiss.mizukiandme --name "Mizuki And Me" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
  Verify Gradle exit 0.

- [ ] **Step 4: Verify Mizuki And Me on MuMu emulator**
  Run:
  ```powershell
  adb -s 127.0.0.1:16384 install -r dist/mizuki_and_me/MizukiAndMe.apk
  ```
  Verify:
  1. Overlay topbar and virtual gamepad render cleanly over Ren'Py visual novel canvas.
  2. Tapping dialogue advances text directly.
  3. Tapping virtual gamepad buttons sends key events without swallowing dialogue taps.
  4. Community link opens properly.
