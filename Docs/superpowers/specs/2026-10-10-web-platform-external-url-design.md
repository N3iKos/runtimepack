# Technical Design Specification: Web Platform Early Identity Virtualization & Bounded External URL Dispatch

- **Document ID:** SPEC-2026-10-10-WEB-PLATFORM-EXTERNAL-URL
- **Status:** DRAFT (FOR CODE REVIEW & SUPERVISOR APPROVAL BEFORE PRODUCTION IMPLEMENTATION)
- **Target Branch / Worktree:** `fix/mvmz-logical-root-20261009` (`.worktrees/fix-mvmz-logical-root`)
- **Engine Scope:** RPG Maker MV, RPG Maker MZ (Web Runtime Backend, `web-rpgmaker`), Universal Web Engine Adapter
- **Affected Crates & Modules:**
  - `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
  - `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt` (`WebBridge`)
  - `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt` (`shouldOverrideUrlLoading`, lifecycle dispatch)
  - `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt` (VFS trailer stream fence, strict origin bounds)
  - `android/shell/app/src/main/java/io/runtimepack/host/WebExternalUrlPolicy.kt` (New pure JVM class)
  - `compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json` (New profile)
  - `Docs/superpowers/compat/COMPAT-REGISTRY.md` (`CAP-WEB-029`, `CAP-WEB-030`)

---

## 1. Executive Summary & Root Cause Analysis (RCA)

### 1.1 Problem A: Synthetic Node Environment Triggers False Desktop Identity in RPG Maker MZ/MV

#### Root Cause Analysis (RCA)
1. To support desktop plugins in mobile WebView environments, `runtimepack_web_shims.js` injects synthetic mocks for Node.js built-ins:
   `window.require` (supporting `path`, `fs`, `Buffer`, `crypto`, `nw.gui`, `greenworks`) and `window.process` (`CAP-WEB-009`).
2. In official RPG Maker MZ (`rmmz_core.js`, lines 242–244) and MV (`rpg_core.js`), platform detection is defined as:
   ```javascript
   Utils.isNwjs = function() {
       return typeof require === "function" && typeof process === "object";
   };
   ```
3. Because `require` is a function and `process` is an object, `Utils.isNwjs()` unconditionally returns `true` inside Android WebView!
4. This false desktop identity triggers multiple severe runtime regressions:
   - **Boot Crash / Interruption in Multilingual & Platform Plugins:** In `shiroin_multilingualSupport.js` (lines 494–540), `Scene_Boot.prototype.detectLaunchEnvironment` checks:
     ```javascript
     if (!Utils.isNwjs || !Utils.isNwjs()) return;
     const os = require("os");
     const tempRoot = toNorm(os.tmpdir());
     ```
     Because `Utils.isNwjs()` is `true`, execution enters the desktop branch and attempts to call `require("os").tmpdir()`. Since the `os` module mock is absent or returns an empty object, calling `os.tmpdir()` throws a `TypeError: os.tmpdir is not a function`, terminating or interrupting `Scene_Boot.start()`.
   - **Boot Continuity & "Continue" Button Freeze:** When `Scene_Boot.start()` is aborted by unhandled promise rejections or script exceptions, `SceneManager._started` is already set to `true`. Resuming via `SceneManager.resume()` skips downstream initialization phases (such as title screen menu generation and save slot status evaluation), causing the "Continue" command or start transitions to freeze or skip event chains.
   - **Save Route Diversion:** In `rmmz_managers.js` (lines 545–547, 612–617):
     ```javascript
     StorageManager.isLocalMode = function() {
         return Utils.isNwjs();
     };
     ```
     When `isLocalMode()` evaluates to `true`, `StorageManager` executes `saveToLocalFile` (writing to desktop paths like `save/`) instead of `saveToForage` (browser storage). While `CAP-WEB-008` dynamically overrides `isLocalMode` to `false` via late polling, any plugin or manager evaluation occurring before or during early script loading executes desktop filesystem branches.
   - **External URL Opener Incompatibility:** In plugins like `QJ-MPMZLib-AnsuzEffects.js` (lines 76–81), the code branches based on platform detection:
     ```javascript
     if (Utils.isMobileDevice()) {
         cordova.InAppBrowser.open(url, '_system');
     } else {
         require('nw.gui').Shell.openExternal(url);
     }
     ```
     When `Utils.isNwjs()` returns `true`, desktop mode is assumed, calling `require('nw.gui').Shell.openExternal(url)`. Because `nw.gui` does not expose `Shell.openExternal`, this throws `TypeError: Cannot read properties of undefined (reading 'openExternal')`.

#### Invalidation of Naive Explorer Proposal
The prior explorer suggested placing `Object.defineProperty(window, 'Utils', { ... setter ... })`.
Through synthetic VM and browser DOM script execution spikes in Node/V8, this proposal has been proven **critically flawed if used in isolation**:
1. **Identifier Re-declaration Syntax Error:** In V8/Chromium, if `window.Utils` is defined with a non-configurable descriptor (`configurable: false`), parsing a global top-level script containing `function Utils() {}` immediately throws a fatal exception:
   `SyntaxError: Identifier 'Utils' has already been declared`.
   This completely crashes engine bootstrap before `rmmz_core.js` executes.
2. **Assignment Overwrite Race:** If `window.Utils` is configurable, the setter is invoked when `function Utils() {}` is hoisted or evaluated. However, in `rmmz_core.js`, the assignment of `Utils.isNwjs`:
   `Utils.isNwjs = function() { return typeof require === "function" && typeof process === "object"; };`
   executes **after** `Utils` has passed into the setter. A naive setter that merely sets `v.isNwjs = function() { return false; }` will be immediately overwritten by the engine's subsequent assignment unless `isNwjs` itself is defined as an accessor property with a no-op setter.
3. **Strict Mode Incompatibility:** If the setter attempts `Object.defineProperty(v, 'isNwjs', { writable: false, value: ... })`, the engine's subsequent assignment `Utils.isNwjs = ...` in strict mode throws `TypeError: Cannot assign to read only property 'isNwjs' of function`.

---

### 1.2 Problem B: Missing External URL Service & Native WebView Navigation Security

#### Root Cause Analysis (RCA)
1. Mobile plugins in RPG Maker MZ (such as `QJ-MPMZLib-AnsuzEffects.js`) attempt to open official websites or sponsor links using Apache Cordova's InAppBrowser plugin:
   `cordova.InAppBrowser.open(url, '_system');`
2. RuntimePack is a dedicated native compatibility container, not an Apache Cordova container. `window.cordova` is undefined.
3. Invoking `openOfficialWebsite` in the game UI triggers:
   `ReferenceError: cordova is not defined`.
4. The caller in `QJ-MPMZLib-AnsuzEffects.js` ignores the return value of `openOfficialWebsite`.
5. Existing Android `WebActivity.kt` exposes `openUrl(url: String)` on `RuntimePackVPad` (overlay WebView bridge), but `RuntimePackVPad` is strictly bound to `overlayWebView`. The game's `gameWebView` only has `RuntimePackNative` (`WebBridge`) and `RuntimePackWeb`.
6. Neither bridge on `gameWebView` currently provides an external URL navigation method.
7. **Native WebView Security Gap (Reviewer Finding):**
   - Currently, `gameWebView` in `WebActivity.kt` does not override `shouldOverrideUrlLoading`.
   - If a link or redirect to an external HTTP URL is triggered, Android WebView navigates `gameWebView` in-place to the external page.
   - Crucially, `RuntimePackNative` and `RuntimePackWeb` (`@JavascriptInterface`) remain injected into whatever page is loaded in `gameWebView`!
   - This exposes native save repositories, storage RPCs, and diagnostic methods to untrusted external domains.
   - Additionally, third-party iframes embedded in game pages could attempt unprompted redirects or background navigations.

---

## 2. Normative Architecture & Contracts

### 2.1 Capability Taxonomy & Registration
- **`CAP-WEB-029`**: RPG Maker MV / MZ platform identity virtualization (`Utils.isNwjs` synchronous false enforcement).
- **`CAP-WEB-030`**: Bounded external URL dispatch service (`openExternalUrl` native bridge).

Both entries succeed `CAP-WEB-028` in `Docs/superpowers/compat/COMPAT-REGISTRY.md`.

---

### 2.2 Synchronous 4-Layer Platform Identity Architecture (`CAP-WEB-029`)

To guarantee that `Utils.isNwjs()` is synchronously `false` before any plugin or scene executes, without breaking engine loading or strict mode assignments, RuntimePack implements four complementary defense-in-depth layers:

```text
Layer 1: HTML Document Order (VfsResourceInterceptor)
  index.html -> injects <script src="web_shims/runtimepack_web_shims.js"> as first <head> child
       │
       ▼
Layer 2: VFS Core Trailer Injection (Script Completion Fence)
  VfsResourceInterceptor intercepts rmmz_core.js / rpg_core.js:
    Streams SequenceInputStream(rawCoreStream, trailerStream)
    trailerStream appends:
      \n;\nif (typeof window !== "undefined" && window.RuntimePack && window.RuntimePack.applyPlatformIdentity && typeof Utils !== "undefined") {
        window.RuntimePack.applyPlatformIdentity(Utils);
      }
    -> Executes in the EXACT SAME script evaluation as core before script onload / managers / plugins!
       │
       ▼
Layer 3: Accessor Descriptor Trap (runtimepack_web_shims.js)
  window.Utils defined with getter & setter:
    when Utils assigned: binds Object.defineProperty(Utils, 'isNwjs', { get: () => () => false, set: () => {} })
       │
       ▼
Layer 4: Late-Binding Polling Fallback (pollPatches)
  Asserts RuntimePack.applyPlatformIdentity(root.Utils) every 10ms for unbundled or dynamically evaluated modules
```

#### Layer 1: Synchronous Injection in `VfsResourceInterceptor.kt`
`VfsResourceInterceptor.kt` intercepts `index.html` for MV and MZ (`web-rpgmaker`, `rpgmaker-mv`, `rpgmaker-mz`), injecting:
```html
<script type="text/javascript" src="web_shims/runtimepack_web_shims.js"></script>
```
before `<script type="text/javascript" src="js/main.js"></script>`.
This guarantees that all web shims, Node mocks, and identity helpers are registered before `main.js` creates or appends any engine script tags.

#### Layer 2: VFS Core Trailer Injection (Deterministic Script Completion Fence)
In `VfsResourceInterceptor.kt`:
When the WebView requests `js/rmmz_core.js` or `js/rpg_core.js` (or paths matching build-time configured core script fence targets):
Instead of serving raw bytes alone, `VfsResourceInterceptor` wraps the response in a `SequenceInputStream`:
```kotlin
val rawStream = streamProvider(logicalPath) ?: return null
val trailerBytes = """
;
(function() {
    if (typeof Utils !== "undefined" && Utils) {
        if (typeof window !== "undefined" && window.RuntimePack && typeof window.RuntimePack.applyPlatformIdentity === "function") {
            window.RuntimePack.applyPlatformIdentity(Utils);
        } else {
            try {
                Object.defineProperty(Utils, "isNwjs", {
                    configurable: true,
                    enumerable: true,
                    get: function() { return function() { return false; }; },
                    set: function() {}
                });
            } catch (_) {
                Utils.isNwjs = function() { return false; };
            }
        }
    }
})();
""".toByteArray(Charsets.UTF_8)

val compositeStream = SequenceInputStream(rawStream, ByteArrayInputStream(trailerBytes))
return createResponse("application/javascript", "UTF-8", compositeStream)
```
- **Why this is deterministic & unshakeable:**
  - `rmmz_core.js` is evaluated from top to bottom by the JavaScript engine.
  - At line 242 of `rmmz_core.js`, `Utils.isNwjs = function() { ... };` executes.
  - At the very last line of `rmmz_core.js`, the trailer fence executes **in the exact same evaluation context**.
  - The leading `\n;\n` guarantees safety even if the preceding line contains a single-line comment `//` or lacks a trailing semicolon.
  - Before the browser fires the `load` event for the script element, and before `rmmz_managers.js` or any plugin can ever be parsed, `Utils.isNwjs` is already locked to `false`!
  - Source game files and GPACK container remain 100% immutable; zero binary patching required on core game code.

#### Layer 3: Accessor Property Trap on `window.Utils`
In `runtimepack_web_shims.js`:
```javascript
(function(root) {
    root.RuntimePack = root.RuntimePack || {};

    root.RuntimePack.applyPlatformIdentity = function(target) {
        if (!target) return;
        try {
            Object.defineProperty(target, "isNwjs", {
                configurable: true,
                enumerable: true,
                get: function() {
                    return function() { return false; };
                },
                set: function(_fn) {
                    // Swallow engine or plugin assignment to prevent overwriting with true
                }
            });
        } catch (_) {
            target.isNwjs = function() { return false; };
        }
    };

    var _utilsInstance = root.Utils;
    if (_utilsInstance) {
        root.RuntimePack.applyPlatformIdentity(_utilsInstance);
    }

    try {
        Object.defineProperty(root, "Utils", {
            configurable: true,
            enumerable: true,
            get: function() {
                return _utilsInstance;
            },
            set: function(val) {
                _utilsInstance = val;
                if (val && (typeof val === "function" || typeof val === "object")) {
                    root.RuntimePack.applyPlatformIdentity(val);
                }
            }
        });
    } catch (_) {
        // Fallback for non-configurable environments
    }
})(typeof window !== "undefined" ? window : globalThis);
```
- **Why this works:**
  - `configurable: true` prevents `SyntaxError: Identifier 'Utils' has already been declared`.
  - When `function Utils() {}` is evaluated in `rmmz_core.js`, the setter executes and installs the `isNwjs` accessor.
  - The `isNwjs` property has `get: () => () => false` and a no-op `set`.
  - When `rmmz_core.js` executes `Utils.isNwjs = function() { ... }`, the setter absorbs the function without error (even in strict mode), preserving `Utils.isNwjs() === false`.

#### Layer 4: Fallback Late Polling in `pollPatches`
Add `if (root.Utils) { root.RuntimePack.applyPlatformIdentity(root.Utils); }` into `pollPatches()` to guarantee persistence across potential dynamic eval or engine reloads.

---

### 2.3 Native WebView Security Architecture & Navigation Policy

To resolve the reviewer's security finding and eliminate privilege leak to external sites, `WebActivity.kt` and `WebBackend.kt` enforce strict origin isolation and navigation override rules:

#### 2.3.1 Canonical Game Origin & Interception Policy
1. **Canonical Origin Constant:** Declared as `const val CANONICAL_GAME_HOST = "game.local"`, `const val CANONICAL_GAME_ORIGIN = "https://game.local"`.
2. **Strict Origin Matching:** In `VfsResourceInterceptor.kt` and `WebBackend.kt`:
   Only requests whose URI scheme is strictly `"https"`, host is strictly `CANONICAL_GAME_HOST`, and port is -1 (default 443) are routed to local VFS assets.
   Substring or prefix checks like `url.startsWith("https://game.local")` without authority boundaries are prohibited.
3. **Subframe / IFrame Document Isolation:**
   In `shouldInterceptRequest`: Any request for subframe documents (`request.isForMainFrame == false` requesting HTML MIME types from remote origins) is blocked fail-closed.
   Game assets are self-contained; untrusted remote HTML embedding is forbidden.

#### 2.3.2 Strict Navigation Override (`shouldOverrideUrlLoading`)
On `gameWebView`:
```kotlin
override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
    val uri = request?.url ?: return true

    // 1. Internal navigation within canonical game origin: allow WebView to handle
    if (uri.scheme == "https" && uri.host == CANONICAL_GAME_HOST) {
        return false
    }

    // 2. Reject all other in-place navigations on gameWebView
    // If the navigation is a top-level user action with gesture to HTTP/HTTPS:
    if (request.isForMainFrame && (uri.scheme == "http" || uri.scheme == "https") && request.hasGesture()) {
        dispatchExternalUrl(uri.toString())
    } else {
        DiagnosticLogStore.recordLog(
            level = "WARN",
            message = "[AND-SEC-001] Blocked unauthorized in-place navigation to ${uri.scheme}://${uri.host}",
            source = "WEB_SECURITY"
        )
    }

    // Always return true to prevent privileged gameWebView from loading external content
    return true
}
```
- **Security Impact:**
  - `gameWebView` **NEVER** leaves `https://game.local/`.
  - Injected JavascriptInterfaces (`RuntimePackNative`, `RuntimePackWeb`) can **NEVER** be accessed by remote web pages.
  - Automatic popup redirects or hidden iframe navigations lacking user gesture are completely neutralized and logged.

---

### 2.4 Bounded External URL Service Architecture (`CAP-WEB-030`)

#### 2.4.1 Security Invariants
1. **Zero Privileged Navigation:** External URLs are strictly dispatched to the Android system browser via Android Intent.
2. **Intent Scheme Restriction:** Only `http` and `https` schemes are accepted. All other schemes (`javascript:`, `file:`, `content:`, `intent:`, `data:`, `blob:`, `about:`) are rejected fail-closed.
3. **Intent Category & Flags:** Intent must be constructed as:
   - Action: `Intent.ACTION_VIEW`
   - Category: `Intent.CATEGORY_BROWSABLE` (ensures only web browser activities match)
   - Flags: `Intent.FLAG_ACTIVITY_NEW_TASK`
   - No arbitrary Intent extras, no permission grants (`FLAG_GRANT_READ_URI_PERMISSION`).
4. **No Universal INTERNET Permission:** Handing off external URLs to an external browser application does **NOT** require `android.permission.INTERNET` in RuntimePack's `AndroidManifest.xml`.
5. **Sanitized Diagnostics Logging:** When logging external URL dispatches or failures:
   - **NEVER** log query parameters or fragments which could contain sensitive session tokens or keys (`?token=xyz` or `#auth=abc`).
   - Log ONLY: `[CAP-WEB-030] Dispatched external URL to host: ${uri.host} (scheme: ${uri.scheme})`.

#### 2.4.2 Pure JVM URL Validation Policy (`WebExternalUrlPolicy.kt`)
To guarantee testability under standard JVM unit tests without Robolectric and without triggering `Method parse in android.net.Uri not mocked`, all URL parsing and validation is implemented in a pure Kotlin class using `java.net.URI`:

```kotlin
package io.runtimepack.host

import java.net.URI

object WebExternalUrlPolicy {
    const val DEFAULT_MAX_URL_LENGTH = 2048

    fun validateAndNormalize(rawUrl: String?, maxLength: Int = DEFAULT_MAX_URL_LENGTH): String? {
        if (rawUrl.isNullOrBlank()) return null
        val trimmed = rawUrl.trim()
        if (trimmed.length > maxLength) return null

        // 1. Reject forbidden control characters (0..31, 127) and raw whitespace
        for (c in trimmed) {
            if (c.code < 32 || c.code == 127 || c == ' ') return null
        }

        // 2. Reject backslashes anywhere in URL (prevents path ambiguity)
        if (trimmed.contains('\\')) return null

        return try {
            val uri = URI(trimmed)
            // 3. Reject relative, opaque, or schemeless URIs
            if (!uri.isAbsolute || uri.isOpaque) return null

            // 4. Strict protocol allowlist
            val scheme = uri.scheme?.lowercase() ?: return null
            if (scheme != "http" && scheme != "https") return null

            // 5. Host and authority validation
            val host = uri.host ?: return null
            if (host.isBlank()) return null
            if (uri.rawAuthority.isNullOrBlank()) return null

            // 6. Credential / Userinfo rejection (anti-spoofing)
            if (uri.rawUserInfo != null) return null

            // 7. Port range validation
            val port = uri.port
            if (port != -1 && (port < 1 || port > 65535)) return null

            uri.toASCIIString()
        } catch (_: Exception) {
            null
        }
    }

    /**
     * Differential parser validation comparing java.net.URI with android.net.Uri
     */
    fun verifyAndroidUriParity(normalizedUrl: String, androidScheme: String?, androidHost: String?): Boolean {
        return try {
            val javaUri = URI(normalizedUrl)
            javaUri.scheme.equals(androidScheme, ignoreCase = true) &&
                javaUri.host.equals(androidHost, ignoreCase = true)
        } catch (_: Exception) {
            false
        }
    }
}
```

#### 2.4.3 Lifecycle-Aware Dispatcher Injection on `WebBridge`
Rather than relying on mutable global singletons (`WebActivity.currentInstance`), `WebBridge` receives an explicit, lifecycle-aware dispatcher lambda via its constructor:

```kotlin
class WebBridge(
    private val saveService: SaveService,
    private val saveRepository: NativeSaveRepository? = null,
    private val externalUrlDispatcher: ((String) -> Boolean)? = null
) {
    @JavascriptInterface
    fun openExternalUrl(url: String): Boolean {
        val dispatcher = externalUrlDispatcher ?: return false
        return dispatcher(url)
    }
    // ... existing bridge methods preserved ...
}
```

In `WebActivity.kt`:
```kotlin
fun dispatchExternalUrl(url: String): Boolean {
    if (isFinishing || isDestroyed) return false
    val normalizedUrl = WebExternalUrlPolicy.validateAndNormalize(url) ?: return false

    // Differential parser sanity check
    val parsedAndroid = Uri.parse(normalizedUrl)
    if (!WebExternalUrlPolicy.verifyAndroidUriParity(normalizedUrl, parsedAndroid.scheme, parsedAndroid.host)) {
        DiagnosticLogStore.recordLog(
            level = "WARN",
            message = "[AND-SEC-002] URI parser differential detected; rejecting URL",
            source = "URL_BRIDGE"
        )
        return false
    }

    val targetView = gameWebView ?: return false
    return targetView.post {
        if (isFinishing || isDestroyed) return@post
        try {
            val intent = Intent(Intent.ACTION_VIEW, parsedAndroid).apply {
                addCategory(Intent.CATEGORY_BROWSABLE)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            startActivity(intent)
            DiagnosticLogStore.recordLog(
                level = "INFO",
                message = "[CAP-WEB-030] Dispatched external URL to host: ${parsedAndroid.host}",
                source = "URL_BRIDGE"
            )
        } catch (e: ActivityNotFoundException) {
            DiagnosticLogStore.recordError(
                message = "[RT-103] No browser activity found to handle external URL host: ${parsedAndroid.host}",
                kind = "activity_not_found",
                source = "URL_BRIDGE"
            )
        } catch (e: Exception) {
            DiagnosticLogStore.recordError(
                message = "[RT-103] Failed to dispatch external URL host: ${parsedAndroid.host} (${e.message})",
                kind = "url_dispatch_error",
                source = "URL_BRIDGE"
            )
        }
    }
}
```
- **Thread Safety & Deadlock Prevention:**
  - `openExternalUrl` executes on the background WebView JS thread.
  - Validates URL synchronously and posts the dispatch to the Android UI thread via `targetView.post`.
  - Checks `isFinishing || isDestroyed` both before posting and inside the looper Runnable.
  - Returns `true` immediately to the JS caller, eliminating any possibility of UI thread deadlock.

#### 2.4.4 Documented JavaScript Namespace Bridge (`runtimepack_web_shims.js`)
In `runtimepack_web_shims.js`:
```javascript
root.RuntimePack = root.RuntimePack || {};
root.RuntimePack.openExternalUrl = function(url) {
    if (root.RuntimePackNative && typeof root.RuntimePackNative.openExternalUrl === "function") {
        return root.RuntimePackNative.openExternalUrl(url);
    }
    console.warn("[RuntimePack] RuntimePackNative.openExternalUrl bridge is unavailable");
    return false;
};
if (typeof root.window !== "undefined") {
    root.window.RuntimePack = root.RuntimePack;
}
```

---

## 3. Declarative Compatibility Profile Specification

- **File Name:** `compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json`
- **Engine ID:** `rpgmaker-mz`
- **Capability IDs:** `CAP-WEB-028`, `CAP-WEB-030`
- **Diagnostic Code:** `CMP-107` (Warning, status `DEGRADED`)
- **Immutability Principle:** The existing profile `compatibility-db/profiles/rpgmaker-mz-desktop-subproc-disable.json` remains completely unmodified and preserved.

### 3.1 Profile Content & Patches Summary
The profile combines the existing 4 desktop subprocess removal patches with the new external URL plugin adapter patch:

1. **Patch 1 (`Shiroin_SaveUI_MZ.js`)**: Identical to `rpgmaker-mz-desktop-subproc-disable.json` (offsets 24013..24275, post-patch SHA `fcc96849...`).
2. **Patch 2 (`QJ-MPMZLib-0.js`)**: Identical to `rpgmaker-mz-desktop-subproc-disable.json` (offsets 94193..95524, post-patch SHA `52aa3864...`).
3. **Patch 3 (`Drill_PictureSnapShot.js`)**: Identical to `rpgmaker-mz-desktop-subproc-disable.json` (offsets 105752..106191, post-patch SHA `5b939355...`).
4. **Patch 4 (`TSR_Save.js`)**: Identical to `rpgmaker-mz-desktop-subproc-disable.json` (offsets 190507..190788, post-patch SHA `5dbb4d03...`).
5. **Patch 5 (`QJ-MPMZLib-AnsuzEffects.js`)**:
   - `target_anchor`: `"payload"`
   - `logical_path`: `"js/plugins/QJ-MPMZLib-AnsuzEffects.js"` (Linux case-preserved)
   - `expected_file_size`: `49343`
   - `expected_file_sha256`: `"9e47514d2cbb5b17f72a7d22645057d805cc03afcd23f52b3be7c8b20cd9b943"`
   - `range`: `{"start": 2548, "end": 2590}` (42 bytes)
   - `expected_fragment_sha256`: `"a388de60450b3bacfed65328aa296d0bfeecdd11c84ca00e775514e952c087ea"`
   - `replacement`: `"(window.RuntimePack && window.RuntimePack.openExternalUrl ? window.RuntimePack.openExternalUrl(url) : (window.RuntimePackNative && window.RuntimePackNative.openExternalUrl ? window.RuntimePackNative.openExternalUrl(url) : false));"` (187 bytes)
   - `result_file_size`: `49488` bytes
   - `result_file_sha256`: `"05711ea1c8c83e74a10df55fe8a823e2b347a4ba7ff0e412586ce3cbebe4f5bb"`

---

## 4. Proposed External Evidence Entries (For Canonical REFERENCES.md)

*(To be incorporated into `Docs/REFERENCES.md` during implementation without mutating canonical docs during design phase):*

### Proposed S37 — Apache Cordova InAppBrowser Plugin API Specification
- **URL:** `https://cordova.apache.org/docs/en/latest/reference/cordova-plugin-inappbrowser/`
- **Retrieval Date:** 2026-10-10
- **Supports:**
  - `cordova.InAppBrowser.open(url, target, options)` API lifecycle and return value.
  - When `target` is specified as `'_system'`, the navigation is handed off directly to the system's native web browser.
  - The return object exposes event handlers for in-app views, but `_system` transfers external URL navigation to native OS browser handlers.

### Proposed S38 — Android Developers Intent.ACTION_VIEW & @JavascriptInterface Threading Specification
- **URL:** `https://developer.android.com/reference/android/content/Intent#ACTION_VIEW`
- **URL:** `https://developer.android.com/reference/android/webkit/JavascriptInterface`
- **Retrieval Date:** 2026-10-10
- **Supports:**
  - `Intent.ACTION_VIEW` combined with `Intent.CATEGORY_BROWSABLE` is the standard Android mechanism for delegating web URLs to external browsers.
  - Methods annotated with `@JavascriptInterface` run on a private background WebView thread, not the Android UI main thread.
  - UI interactions and Activity launches initiated from background threads must be posted to the main looper or use `FLAG_ACTIVITY_NEW_TASK`.
  - Missing browser activity results in `ActivityNotFoundException`. Bridge methods return synchronous acceptance boolean without blocking the JS worker thread.

### Proposed S39 — Android WebView Cross-Origin Interface Security Guidelines
- **URL:** `https://developer.android.com/privacy-and-security/risks/webview-attacks`
- **Retrieval Date:** 2026-10-10
- **Supports:**
  - `WebView.addJavascriptInterface` exposes Java objects to all frames in the WebView.
  - External URL bridges and WebViewClients must strictly enforce URI scheme allowlists (`http`/`https`), override URL loadings to prevent in-place navigation of external domains, and reject dangerous schemes (`javascript:`, `file:`, `intent:`) to prevent sandbox bypass and origin leak.

### Proposed S40 — RPG Maker MZ Platform Detection & Storage Routing Baseline
- **Evidence Path:** Internal repository fixtures and authentic MZ v1.10 distribution (`js/rmmz_core.js`, `js/rmmz_managers.js`)
- **SHA-256 (`rmmz_core.js`):** `4ad828ae6cdd8b526e25591eaffd1ccf5ac4eb753f865f7ff48a0b3c821ff67f`
- **SHA-256 (`rmmz_managers.js`):** `ad61be483293aa4b4056abc0b309e8871d2bd66165ac4453dafd90c5d92529cd`
- **Supports:**
  - `Utils.isNwjs()` returns `typeof require === "function" && typeof process === "object"`.
  - `StorageManager.isLocalMode()` delegates directly to `Utils.isNwjs()`.
  - When `isNwjs` returns `false`, `StorageManager.saveZip` routes to `saveToForage`, which maps to `RuntimePackNative` (`NativeSaveRepository`).

### Proposed S41 — Node.js OS Module Specifications and Unsupported Mobile Scope
- **URL:** `https://nodejs.org/api/os.html`
- **Retrieval Date:** 2026-10-10
- **Supports:**
  - Desktop-only `os.tmpdir()`, `os.platform()`, `os.cpus()` APIs.
  - Unshimmed in Android mobile web runtime; plugins guarding desktop behavior behind `Utils.isNwjs()` must bypass OS module inspection entirely when identity is properly virtualized.

---

## 5. Verification & Acceptance Criteria

1. **Synthetic Platform Identity Verification:**
   - In Node VM and WebView JS tests, evaluating global `function Utils() {}` followed by `Utils.isNwjs = ...` results in `Utils.isNwjs() === false`.
   - `StorageManager.isLocalMode()` evaluates to `false`.
   - `StorageManager.saveZip` routes to `saveToForage` and persists save data to `NativeSaveRepository`.
   - In simulated `Scene_Boot.start()`, `detectLaunchEnvironment` returns immediately without invoking `require("os")`.
2. **URL Policy & Bridge Unit Tests:**
   - Valid HTTP/HTTPS URLs (`http://example.com`, `https://example.com/path?q=1`) return normalized string and `true`.
   - URLs with userinfo (`http://admin:pass@host`), invalid schemes (`javascript:alert(1)`, `file:///etc/hosts`), control characters, or non-numeric/out-of-range ports return `null` and `false`.
   - Pure JVM Kotlin unit tests pass with 100% assertion coverage.
3. **Declarative Profile Integrity:**
   - `cargo test --test staging_profile_test` confirms that `rpgmaker-mz-desktop-subproc-external-url.json` applies cleanly to staging trees.
   - Resulting files match expected SHA-256 digests.
4. **Android Build & Deliverable Preservation:**
   - Real candidate build uses `--flavor beta` and `--compat-profile compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json`.
   - Source game directory remains 100% byte-identical.
   - Saved user states remain preserved.
