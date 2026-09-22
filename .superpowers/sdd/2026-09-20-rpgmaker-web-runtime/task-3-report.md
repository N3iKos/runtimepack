# Task 3 Report: Mobile Web Shims (Node.js, NW.js, Steamworks, Storage)

## Summary
Successfully implemented zero-dependency mobile web shims (`runtimepack_web_shims.js`) for Node.js modules (`path`, `fs`), NW.js (`nw.gui`, `nw.Window`, `nw.App`), Steamworks (`Greenworks`), and RPG Maker `StorageManager.isLocalMode` virtualization for Android WebView execution. Updated `WebActivity.kt` to load the shims asset on startup and inject it into `gameWebView` on `onPageStarted` and `onPageFinished`.

## Modified and Created Files
1. `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
   - Lightweight polyfill providing:
     - `window.require(moduleName)`:
       - `'path'`: POSIX path methods (`join`, `resolve`, `normalize`, `dirname`, `basename`, `extname`, `sep: '/'`, `delimiter: ':'`).
       - `'nw.gui'` / `'nw'`: `Window: { get: () => ({ isFullScreen: false, enterFullscreen, leaveFullscreen, on, showDevTools, show, hide, focus, blur }) }`, `App: { manifest: {} }`.
       - `'fs'`: Virtual/stubbed safe dummy methods (`existsSync: () => false`, `readFileSync: () => ""`, `mkdirSync: () => {}`, `writeFileSync: () => {}`, `statSync`, `readdirSync`, `readFile`, `writeFile`).
       - Unknown module fallback logging warning and returning `{}`.
     - Global `window.nw = window.require('nw.gui')`.
     - Global `window.Greenworks = window.greenworks = { init: () => true, isSteamRunning: () => false, getScreenLanguage: () => "en", activateAchievement: () => {}, getNumberOfPlayers: (cb) => cb && cb(1) }`.
     - Virtualized `StorageManager.isLocalMode = function() { return false; }` patched both for pre-existing and dynamically loaded `StorageManager` declarations (via getter/setter proxy and direct patch).
     - Guard against multiple evaluation via `window.__runtimepack_shims_installed`.
2. `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Added `loadWebShims()` reading `assets/web_shims/runtimepack_web_shims.js`.
   - Updated `wv.webViewClient` to inject `runtimepack_web_shims.js` on `onPageStarted(view, url, favicon)` and on `onPageFinished(view, url)`.
3. `android/shell/app/src/test/js/web_shims.test.js`:
   - Node.js unit test suite verifying:
     - `require('path').join("www", "data", "System.json") == "www/data/System.json"`.
     - `nw.Window.get()` returns valid mock without throwing.
     - `Greenworks.init()` and `greenworks.init()` return true.
     - `StorageManager.isLocalMode()` returns false (pre-existing and late-initialized).
     - Multiple evaluation guard idempotency.
     - Unhandled require module safety.
4. `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`:
   - Added unit test `testWebShimsScriptInitialState` verifying initial and mutable state for `webShimsScript`.

## Verification Results
1. TDD Failing Phase:
   - Executing `node android/shell/app/src/test/js/web_shims.test.js` prior to creating `runtimepack_web_shims.js` resulted in `AssertionError [ERR_ASSERTION]: Shims file must exist at .../runtimepack_web_shims.js`.
2. TDD Passing Phase:
   - `node android/shell/app/src/test/js/web_shims.test.js`: All assertions passed successfully.
   - `.\gradlew.bat testDebugUnitTest`: BUILD SUCCESSFUL (24 actionable tasks: 1 executed, 23 up-to-date).
   - Additional regression tests passed:
     - `node android/shell/app/src/test/js/vpad_bridge.test.js`
     - `node android/shell/app/src/test/js/cheat_primitives.test.js`
     - `node android/shell/app/src/test/js/cheat_menu.test.js`

## Status
DONE
