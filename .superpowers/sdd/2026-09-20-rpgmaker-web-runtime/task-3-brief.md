# Task 3: Mobile Web Shims (Node.js, NW.js, Steamworks, Storage)

## Goal
Implement a lightweight zero-dependency JavaScript polyfill file `runtimepack_web_shims.js` that provides Node.js `path`, NW.js `nw.Window`, Steamworks `Greenworks`, and RPG Maker `StorageManager` IndexedDB compatibility for Android WebView execution without crashing.

## Requirements
1. In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
   - Implement `window.require(moduleName)` polyfill:
     - `'path'`: `join`, `resolve`, `normalize`, `dirname`, `basename`, `extname`, `sep: '/'`, `delimiter: ':'`.
     - `'nw.gui'` / `'nw'`: `Window: { get: () => ({ isFullScreen: false, enterFullscreen: () => {}, leaveFullscreen: () => {}, on: () => {}, showDevTools: () => {}, show: () => {}, hide: () => {}, focus: () => {}, blur: () => {} }) }, App: { manifest: {} }`.
     - `'fs'`: Virtual/stubbed `fs` returning safe dummy methods (`existsSync: () => false`, `readFileSync: () => ""`, `mkdirSync: () => {}`, `writeFileSync: () => {}`).
   - Define global `window.nw = window.require('nw.gui')`.
   - Define global `window.Greenworks = window.greenworks = { init: () => true, isSteamRunning: () => false, getScreenLanguage: () => "en", activateAchievement: () => {}, getNumberOfPlayers: (cb) => cb && cb(1) }`.
   - Patch `StorageManager.isLocalMode = function() { return false; }` so RPG Maker MV / MZ uses web IndexedDB / WebStorage rather than failing on Node `fs.writeFile`.
   - Guard against multiple evaluation via `window.__runtimepack_shims_installed`.
2. In `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Read `assets/web_shims/runtimepack_web_shims.js` on startup and inject it into `gameWebView` in `onPageStarted` and `onPageFinished` using `evaluateJavascript`.
3. In `android/shell/app/src/test/js/web_shims.test.js`:
   - Write Node.js unit test suite:
     - Verify `require('path').join("www", "data", "System.json") == "www/data/System.json"`.
     - Verify `nw.Window.get()` returns valid mock without throwing.
     - Verify `Greenworks.init()` returns true.
     - Verify `StorageManager.isLocalMode()` returns false.
4. Run test:
   `node android/shell/app/src/test/js/web_shims.test.js`
   `.\gradlew.bat testDebugUnitTest`
5. Write execution report to `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-3-report.md`.
